# 实验三：进程管理与 stride 调度

## 练习内容

实现直接加载可执行程序的 spawn、进程优先级设置及 stride 比例调度；延续前章的时间与内存映射系统调用。

## 实现

spawn 按文件名查找 ELF，通过 TaskControlBlock::new 创建独立地址空间、PID、内核栈和初始陷阱上下文，建立父进程 weak 引用和父进程 children 强引用，随后入就绪队列。它不复制父地址空间，避免 fork 后再 exec 的额外开销。未知程序返回 -1。

每个任务维护 priority 和 stride，初始优先级为 16。set_priority 拒绝小于 2 的值，成功时返回设置值。调度器从就绪队列中选择 stride 最小的任务，并增加 max(BIG_STRIDE / priority, 1)。BIG_STRIDE 取 2^32，u64 保存累计值，通过有符号 wrapping 差值比较，支持跨越计数回绕。fork 继承父优先级和 stride，spawn 从父 stride 起步，避免新进程凭空获得过多历史调度份额。

修复 exec 后堆底和 program_brk 的更新，继承第四章 mmap、munmap、跨页 get_time 的实现。

## 验证与理解

运行官方第五章检查器，覆盖 fork/exec/waitpid、退出回收、spawn、非法优先级、不同权重的 CPU 份额及前章内存操作。priority 越大，单次 stride 增量越小，因此更频繁地被选中；就绪队列只有可运行进程，不让等待或僵尸进程参与选择。

本地验证结果：官方检查器 15/15，lab1 至 lab3 报告检查通过，命令退出码为 0。
