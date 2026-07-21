//! Run a closure on a transient thread with a large stack.
//!
//! AV1 decoding (rav1d, run single-threaded so it decodes on the calling
//! thread) needs far more contiguous stack than the game provides: `ce.exe` /
//! `game.exe` is a 32-bit process whose main thread has only a ~1 MiB stack
//! reserve, and dav1d/rav1d frame decode overflowed it (the engine died with
//! `EXCEPTION_STACK_OVERFLOW` the first time a webm cutscene decoded). dav1d is
//! designed to run decode on worker threads it spawns with a generous stack; we
//! keep it single-threaded (for the simple one-send-one-get model and a trivial
//! `Send`) and instead hop each decode call onto a roomy stack here.
//!
//! `thread::scope` makes this synchronous and borrow-friendly: the closure may
//! borrow caller locals (e.g. `&mut decoder`), runs to completion on the big
//! stack, and its result is returned to the caller. A panic on the worker is
//! re-raised on the caller so the FFI layer's `catch_unwind` still contains it.

/// Stack size for the decode worker. 16 MiB is ample headroom over dav1d's
/// needs and costs only address space (committed lazily), which is fine for the
/// one-at-a-time, short-lived worker even in a 32-bit process.
const DECODE_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Run `f` on a transient thread with a 16 MiB stack and return its result.
pub(crate) fn run_on_big_stack<T, F>(f: F) -> T
where
    F: FnOnce() -> T + Send,
    T: Send,
{
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(DECODE_STACK_BYTES)
            .spawn_scoped(scope, f)
            .expect("spawn decode worker thread")
            .join()
            // Propagate a worker panic to the caller, where the FFI boundary's
            // `catch_unwind` turns it into a safe default rather than UB.
            .unwrap_or_else(|payload| std::panic::resume_unwind(payload))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_the_closure_result() {
        assert_eq!(run_on_big_stack(|| 2 + 2), 4);
    }

    #[test]
    fn closure_can_mutably_borrow_caller_locals() {
        let mut v = vec![1, 2, 3];
        run_on_big_stack(|| v.push(4));
        assert_eq!(v, [1, 2, 3, 4]);
    }

    #[test]
    fn provides_a_stack_far_larger_than_the_default() {
        // A ~4 MiB stack local would overflow the default test-thread stack
        // (2 MiB) if this ran inline; on the 16 MiB worker it is fine. This is
        // the property the decode path relies on.
        let sum = run_on_big_stack(|| {
            let buf = [7u8; 4 * 1024 * 1024];
            // `black_box` + a real read keep the array from being optimised away.
            std::hint::black_box(&buf);
            buf.iter().map(|&b| b as u64).sum::<u64>()
        });
        assert_eq!(sum, 7 * 4 * 1024 * 1024);
    }
}
