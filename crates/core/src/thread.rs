//! Named worker threads (VIEW-056): every thread the program starts has a name, so that a stall report, a debugger or the task manager shows
//! whose it is, and a busy thread without a name is the driver's or a library's.

use std::thread::{Builder, Scope, ScopedJoinHandle};

/// `scope.spawn(f)` on a thread called `name`; a thread that cannot be started is a failure of the system, not of the call.
pub fn spawn_named<'scope, 'env, F, T>(scope: &'scope Scope<'scope, 'env>, name: &str, f: F) -> ScopedJoinHandle<'scope, T>
where
    F: FnOnce() -> T + Send + 'scope,
    T: Send + 'scope,
{
    Builder::new().name(name.to_string()).spawn_scoped(scope, f).unwrap_or_else(|e| panic!("cannot start the thread {name}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: VIEW-056
    #[test]
    fn a_scoped_worker_has_the_name_it_was_given_and_returns_its_value() {
        let got = std::thread::scope(|s| {
            let h = spawn_named(s, "test-worker", || (std::thread::current().name().map(str::to_string), 7));
            h.join().unwrap()
        });
        assert_eq!(got, (Some("test-worker".to_string()), 7));
    }
}
