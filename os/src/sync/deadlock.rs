//! Per-process resource-allocation graph and deadlock detection.
use alloc::{collections::VecDeque, vec, vec::Vec};

struct Resource {
    available: usize,
    handoff: bool,
    waiters: VecDeque<usize>,
}

/// Track current allocations and outstanding requests, rather than future maxima.
pub struct DeadlockDetector {
    enabled: bool,
    resources: Vec<Resource>,
    allocation: Vec<Vec<usize>>,
    request: Vec<Vec<usize>>,
}

impl DeadlockDetector {
    /// Start disabled, with no synchronization objects.
    pub fn new() -> Self {
        Self {
            enabled: false,
            resources: Vec::new(),
            allocation: Vec::new(),
            request: Vec::new(),
        }
    }

    /// Toggle checking while retaining resource accounting.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Register a mutex or semaphore, independently of its userspace handle.
    pub fn add_resource(&mut self, available: usize, handoff: bool) -> usize {
        let id = self.resources.len();
        self.resources.push(Resource {
            available,
            handoff,
            waiters: VecDeque::new(),
        });
        for row in self.allocation.iter_mut().chain(self.request.iter_mut()) {
            row.push(0);
        }
        id
    }

    fn ensure_thread(&mut self, tid: usize) {
        while self.allocation.len() <= tid {
            self.allocation.push(vec![0; self.resources.len()]);
            self.request.push(vec![0; self.resources.len()]);
        }
    }

    /// Tentatively request one unit; reject and roll back only if deadlocked.
    pub fn request(&mut self, tid: usize, resource: usize) -> bool {
        self.ensure_thread(tid);
        self.request[tid][resource] += 1;
        if self.enabled && !self.can_finish() {
            self.request[tid][resource] -= 1;
            return false;
        }
        if self.resources[resource].available > 0 {
            self.resources[resource].available -= 1;
            self.request[tid][resource] -= 1;
            self.allocation[tid][resource] += 1;
        } else {
            self.resources[resource].waiters.push_back(tid);
        }
        true
    }

    fn can_finish(&self) -> bool {
        let mut work: Vec<usize> = self.resources.iter().map(|r| r.available).collect();
        let mut finished = vec![false; self.allocation.len()];
        loop {
            let mut progress = false;
            for tid in 0..finished.len() {
                if !finished[tid]
                    && self.request[tid]
                        .iter()
                        .zip(work.iter())
                        .all(|(need, free)| need <= free)
                {
                    for (free, held) in work.iter_mut().zip(self.allocation[tid].iter()) {
                        *free += held;
                    }
                    finished[tid] = true;
                    progress = true;
                }
            }
            if !progress {
                break;
            }
        }
        finished.iter().all(|done| *done)
    }

    /// Spin mutexes grant ownership when the waiter actually obtains the lock.
    pub fn finish_spin(&mut self, tid: usize, resource: usize) {
        if self.request[tid][resource] == 0 {
            return;
        }
        let res = &mut self.resources[resource];
        let index = res
            .waiters
            .iter()
            .position(|waiter| *waiter == tid)
            .unwrap();
        res.waiters.remove(index);
        res.available -= 1;
        self.request[tid][resource] -= 1;
        self.allocation[tid][resource] += 1;
    }

    /// Release a unit, transferring ownership in the same FIFO order as blocking primitives.
    pub fn release(&mut self, tid: usize, resource: usize) {
        self.ensure_thread(tid);
        if self.allocation[tid][resource] > 0 {
            self.allocation[tid][resource] -= 1;
        }
        let res = &mut self.resources[resource];
        if res.handoff {
            if let Some(waiter) = res.waiters.pop_front() {
                self.request[waiter][resource] -= 1;
                self.allocation[waiter][resource] += 1;
                return;
            }
        }
        res.available += 1;
    }
}

fn current_tid() -> usize {
    crate::task::current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid
}

/// Record a request without holding the process borrow across a context switch.
pub fn request(resource: usize) -> bool {
    let tid = current_tid();
    let process = crate::task::current_process();
    let result = process
        .inner_exclusive_access()
        .deadlock
        .request(tid, resource);
    result
}

/// Finish a successful acquisition of a spin mutex.
pub fn finish_spin(resource: usize) {
    let tid = current_tid();
    crate::task::current_process()
        .inner_exclusive_access()
        .deadlock
        .finish_spin(tid, resource);
}

/// Account for unlock/up, including calls made through condition variables.
pub fn release(resource: usize) {
    let tid = current_tid();
    crate::task::current_process()
        .inner_exclusive_access()
        .deadlock
        .release(tid, resource);
}
