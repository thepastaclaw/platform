//! Shared tokio runtime for blocking on async wallet operations.
//!
//! ## Stack size
//!
//! iOS dispatch/concurrency worker threads default to ~512 KB of stack.
//! Proof verification in `rs-drive` recurses through GroveDB deeply
//! enough to blow past that — we've seen `EXC_BAD_ACCESS` at
//! `verify_state_transition_was_executed_with_proof`'s function
//! prologue, which is the classic fingerprint of a stack-guard hit.
//!
//! Two mitigations together:
//!
//! 1. Configure the worker-thread stack to 8 MB (matches what rs-sdk
//!    uses internally for similar reasons).
//! 2. Dispatch the heavy async work onto a worker via
//!    [`block_on_worker`] instead of polling directly on the
//!    (small-stacked) calling thread. `block_on` itself still runs
//!    on the caller, but it parks almost immediately — all the
//!    compute happens on the tokio worker.
//!
//! ## Panic boundary
//!
//! Spawned work runs through `tokio::spawn`, which catches panics and
//! surfaces them via `JoinError`. We translate a panic into a
//! [`PlatformWalletFFIResult`] error rather than `.expect(...)`-ing —
//! a panic unwinding across `extern "C"` aborts the host process,
//! and FFI callers should see a structured error instead.

use crate::error::{PlatformWalletFFIResult, PlatformWalletFFIResultCode};

/// Worker thread stack size for the shared runtime. 8 MB gives proof
/// verification + GroveDB comfortable headroom without meaningfully
/// affecting memory footprint (we spin up a small number of workers).
const WORKER_STACK_BYTES: usize = 8 * 1024 * 1024;

/// Get the shared tokio runtime.
///
/// All async FFI functions use this runtime. Prefer
/// [`block_on_worker`] over `runtime().block_on(...)` so the heavy
/// work runs on a worker thread with the larger stack configured
/// here, rather than the (small) calling thread.
pub(crate) fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: once_cell::sync::Lazy<tokio::runtime::Runtime> = once_cell::sync::Lazy::new(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_stack_size(WORKER_STACK_BYTES)
            .build()
            .expect("Failed to create tokio runtime for platform-wallet-ffi")
    });
    &RT
}

/// Drive `future` to completion, moving the actual polling onto a
/// worker thread so the caller's stack size doesn't bound the
/// computation.
///
/// The calling thread still blocks (that's what FFI wants); it just
/// parks on a oneshot instead of driving the future itself.
///
/// Returns `Err(PlatformWalletFFIResult)` if the spawned worker
/// panicked — callers thread that through the existing
/// `unwrap_result_or_return!` machinery so the FFI surface returns a
/// structured error instead of unwinding across `extern "C"` and
/// aborting the host process.
pub(crate) fn block_on_worker<F>(future: F) -> Result<F::Output, PlatformWalletFFIResult>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    let rt = runtime();
    let join_result = rt.block_on(async move { rt.spawn(future).await });
    join_result.map_err(join_error_to_ffi_result)
}

/// Render a `tokio::task::JoinError` as a `PlatformWalletFFIResult`.
///
/// Panics carry a payload that may be a `&'static str` or `String` (the
/// two common shapes for `panic!(...)`); we try both before falling
/// back to a generic message. Cancellation also lands here for
/// completeness, though `block_on_worker` never cancels its own task.
fn join_error_to_ffi_result(err: tokio::task::JoinError) -> PlatformWalletFFIResult {
    if err.is_panic() {
        let payload = err.into_panic();
        let msg = if let Some(s) = payload.downcast_ref::<&'static str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic payload>".to_string()
        };
        PlatformWalletFFIResult::err(
            PlatformWalletFFIResultCode::ErrorUnknown,
            format!("tokio worker panicked: {msg}"),
        )
    } else {
        PlatformWalletFFIResult::err(
            PlatformWalletFFIResultCode::ErrorUnknown,
            format!("tokio worker task failed: {err}"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A panic inside the spawned future must be caught and returned
    /// as an `Err(PlatformWalletFFIResult)` so the panic never
    /// unwinds across the `extern "C"` boundary into the host.
    #[test]
    fn worker_panic_becomes_ffi_error_result() {
        let r: Result<(), PlatformWalletFFIResult> = block_on_worker(async {
            panic!("induced worker panic");
        });
        let err = r.expect_err("panicking future must produce Err, not propagate");
        assert_eq!(err.code, PlatformWalletFFIResultCode::ErrorUnknown);
        assert!(!err.message.is_null());
        let msg = unsafe { std::ffi::CStr::from_ptr(err.message) }
            .to_string_lossy()
            .into_owned();
        assert!(
            msg.contains("tokio worker panicked") && msg.contains("induced worker panic"),
            "panic message must surface to the caller (got: {msg})"
        );
    }

    /// The happy path keeps returning the future's output, just
    /// wrapped in `Ok`. Pins the new signature contract.
    #[test]
    fn worker_success_returns_ok_output() {
        let r = block_on_worker(async { 42u32 }).expect("non-panicking future must succeed");
        assert_eq!(r, 42);
    }
}
