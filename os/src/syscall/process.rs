//! Process management syscalls
use crate::task::{change_program_brk, exit_current_and_run_next, suspend_current_and_run_next};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    let us = crate::timer::get_time_us();
    let mut data = [0u8; 2 * core::mem::size_of::<usize>()];
    let n = core::mem::size_of::<usize>();
    data[..n].copy_from_slice(&(us / 1_000_000).to_ne_bytes());
    data[n..].copy_from_slice(&(us % 1_000_000).to_ne_bytes());
    if crate::mm::copy_to_user(crate::task::current_user_token(), ts as usize, &data) {
        0
    } else {
        -1
    }
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(request: usize, id: usize, data: usize) -> isize {
    if request == 2 {
        return crate::task::syscall_count(id)
            .map(|n| n as isize)
            .unwrap_or(-1);
    }
    let token = crate::task::current_user_token();
    if request > 1 || !crate::mm::user_buffer_valid(token, id, 1, request == 1) {
        return -1;
    }
    let va = crate::mm::VirtAddr::from(id);
    let pte = crate::mm::PageTable::from_token(token)
        .translate(va.floor())
        .unwrap();
    let byte = &mut pte.ppn().get_bytes_array()[va.page_offset()];
    if request == 0 {
        *byte as isize
    } else {
        *byte = data as u8;
        0
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {
    crate::task::with_current_memory(|memory| memory.mmap(start, len, prot))
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    crate::task::with_current_memory(|memory| memory.munmap(start, len))
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
