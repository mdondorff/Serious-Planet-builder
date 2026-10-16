//! Scheduling priority of the calling thread (STRM-020, ADR 0046): the tile generation workers run below normal priority so that
//! the OS gives the frame thread its core back when both want it. A priority changes when a thread runs, never what it computes
//! (CON-14): no result depends on it. The only platform code is here; elsewhere a request reports that it was not applied.

use std::fmt;

/// A thread's scheduling priority relative to the process's other threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    /// Windows `THREAD_PRIORITY_BELOW_NORMAL` (-1); Linux nice +5 for the thread.
    BelowNormal,
    /// The default of a thread: Windows `THREAD_PRIORITY_NORMAL` (0); Linux nice 0.
    Normal,
}

impl Priority {
    pub fn name(self) -> &'static str {
        match self {
            Priority::BelowNormal => "below-normal",
            Priority::Normal => "normal",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Priority::BelowNormal, Priority::Normal].into_iter().find(|p| p.name() == s)
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Set the calling thread's priority; `false` when the platform has no such call or refused it (the thread then keeps its
/// priority, which changes timing only).
pub fn set_current(p: Priority) -> bool {
    imp::set_current(p)
}

/// The calling thread's priority as the OS reports it, `None` where it cannot be read or is neither level.
pub fn current() -> Option<Priority> {
    imp::current()
}

#[cfg(windows)]
mod imp {
    use super::Priority;
    use windows::Win32::System::Threading::{
        GetCurrentThread, GetThreadPriority, SetThreadPriority, THREAD_PRIORITY, THREAD_PRIORITY_BELOW_NORMAL, THREAD_PRIORITY_NORMAL,
    };

    fn level(p: Priority) -> THREAD_PRIORITY {
        match p {
            Priority::BelowNormal => THREAD_PRIORITY_BELOW_NORMAL,
            Priority::Normal => THREAD_PRIORITY_NORMAL,
        }
    }

    pub fn set_current(p: Priority) -> bool {
        // SAFETY: the pseudo handle of the calling thread is always valid and needs no closing.
        unsafe { SetThreadPriority(GetCurrentThread(), level(p)).is_ok() }
    }

    pub fn current() -> Option<Priority> {
        // SAFETY: as above; the call only reads.
        let v = unsafe { GetThreadPriority(GetCurrentThread()) };
        [Priority::BelowNormal, Priority::Normal].into_iter().find(|p| level(*p).0 == v)
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::Priority;

    fn nice(p: Priority) -> libc::c_int {
        match p {
            Priority::BelowNormal => 5,
            Priority::Normal => 0,
        }
    }

    fn tid() -> libc::id_t {
        // SAFETY: `gettid` takes no arguments and cannot fail.
        unsafe { libc::syscall(libc::SYS_gettid) as libc::id_t }
    }

    pub fn set_current(p: Priority) -> bool {
        // SAFETY: a plain system call on the calling thread's own id (Linux nice values are per thread).
        unsafe { libc::setpriority(libc::PRIO_PROCESS, tid(), nice(p)) == 0 }
    }

    pub fn current() -> Option<Priority> {
        // SAFETY: as above; -1 is also a valid nice value, so errno is cleared first and read after.
        let v = unsafe {
            *libc::__errno_location() = 0;
            let v = libc::getpriority(libc::PRIO_PROCESS, tid());
            if v == -1 && *libc::__errno_location() != 0 {
                return None;
            }
            v
        };
        [Priority::BelowNormal, Priority::Normal].into_iter().find(|p| nice(*p) == v)
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod imp {
    use super::Priority;

    pub fn set_current(_: Priority) -> bool {
        false
    }

    pub fn current() -> Option<Priority> {
        None
    }
}
