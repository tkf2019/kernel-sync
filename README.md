# Rust kernel-sync

Kernel synchronization primitives implemented in Rust:

- [x] Local interrupt disabling: Forbid interrupt handling on a single CPU.
- [x] Spin Lock: Lock with busy wait.
- [x] Sleep Lock: Lock with blocking wait (sleep).
- [ ] Read-Copy-Update (RCU): Lock-free access to shared data structures through pointers.

Features:
- Interrupt dependent on architure:
  - [x] riscv64

## Usage

### [SpinLock](src/spinlock.rs)

See `[spin::Mutex](https://docs.rs/spin/latest/spin/mutex/spin/struct.SpinMutex.html)`.

Remember to save CPU local variables, e.g. `cpu->intena` before switching task context.

### [SleepLock](src/sleeplock.rs)

Here is a brief implementation for **SleepLock**:

```rust
impl kernel_sync::SleepLockSched for TaskLockedInner {
    unsafe fn sched(guard: SpinLockGuard<Self>) {
        // Lock might be released after the task is pushed back to the scheduler.
        TASK_MANAGER.lock().add(curr_task().take().unwrap());
        drop(guard);

        __switch(curr_ctx(), idle_ctx());
    }

    fn set_id(task: &mut Self, id: Option<usize>) {
        task.sleeping_on = id;
    }

    fn sleep(task: &mut Self) {
        task.state = TaskState::Interruptible;
    }

    /// Wakes up tasks sleeping on this lock.
    fn wakeup(id: usize) {
        TASK_MANAGER.lock().iter().for_each(|task| {
            let mut inner = task.locked_inner();
            if inner.state == TaskState::Interruptible
                && inner.sleeping_on.is_some()
                && inner.sleeping_on.unwrap() == id
            {
                inner.state = TaskState::Runnable;
            }
        });
    }
}
```

where the **TaskLockedInner** is defined as below:

```rust
/// Mutable inner data of the task, protected by lock.
pub struct TaskLockedInner {
    /// Task state, using five-state model.
    pub state: TaskState,

    /// Sleep lock id.
    pub sleeping_on: Option<usize>,

    /// Hierarchy pointers in task management.
    /// INIT task has no parent task.
    pub parent: Option<Weak<Task>>,

    /// Pointers to child tasks.
    /// When a parent task exits before its children, they will become orphans.
    /// These tasks will be adopted by INIT task to avoid being dropped when the reference
    /// counter becomes 0.
    pub children: Vec<Arc<Task>>,
}
```

See details in the toy os [tCore](https://github.com/tkf2019/tCore/blob/rust-vfs/kernel/src/tests/sleeplock.rs).