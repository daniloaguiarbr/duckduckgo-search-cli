// SPDX-License-Identifier: MIT OR Apache-2.0
//! Loom interleaving tests for `AtomicBool` rate-limit flag.
//!
//! `tokio::sync::Semaphore` is NOT supported by loom — only std-style
//! primitives (`loom::sync::Arc`, `loom::sync::atomic::`*, `loom::thread`).
//! This test validates the `AtomicBool` flag used in search.rs for
//! rate-limit signaling across tasks.
//!
//! Run with: RUSTFLAGS="--cfg loom" cargo test --test `loom_atomics` --release

#[cfg(loom)]
mod loom_tests {
    use loom::sync::atomic::{AtomicBool, Ordering};
    use loom::sync::Arc;
    use loom::thread;

    #[test]
    fn rate_limit_flag_visibility_across_threads() {
        loom::model(|| {
            let flag = Arc::new(AtomicBool::new(false));
            let flag2 = flag.clone();

            let writer = thread::spawn(move || {
                flag2.store(true, Ordering::Relaxed);
            });

            let saw_flag = flag.load(Ordering::Relaxed);
            writer.join().unwrap();

            assert!(flag.load(Ordering::Relaxed));
            let _ = saw_flag;
        });
    }

    #[test]
    fn multiple_writers_converge_to_true() {
        loom::model(|| {
            let flag = Arc::new(AtomicBool::new(false));

            let handles: Vec<_> = (0..3)
                .map(|_| {
                    let f = flag.clone();
                    thread::spawn(move || {
                        f.store(true, Ordering::Relaxed);
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }

            assert!(flag.load(Ordering::Relaxed));
        });
    }
}
