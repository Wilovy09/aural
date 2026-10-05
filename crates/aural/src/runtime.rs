//! The one tokio runtime every network call runs on. The UI awaits its join handles from
//! Freya's own executor, which works because a tokio `JoinHandle` needs no tokio context.

use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("aural-io")
            .build()
            .expect("cannot start the io runtime")
    })
}

/// Runs `future` on the io runtime.
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    runtime().spawn(future)
}

/// Runs a blocking job (a sign-in window, a decoder) on the runtime's blocking pool.
pub fn blocking<F, R>(job: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    runtime().spawn_blocking(job)
}

/// Blocks the calling thread (never a runtime thread) on `future`.
pub fn block_on<F: Future>(future: F) -> F::Output {
    runtime().block_on(future)
}
