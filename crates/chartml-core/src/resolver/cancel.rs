//! Cooperative cancellation for in-flight provider work.
//!
//! `CancellationToken` is a thin `Arc<AtomicBool>` plus a waker list so async
//! tasks can yield until cancellation flips. No external runtime dependency:
//! `Arc<AtomicBool>` + `std::sync::Mutex<Vec<Waker>>` works equivalently on
//! native and WASM (`?Send`).
//!
//! Phase 3 always passes `None` from the resolver — providers that opt into
//! listening for cancellation can do so without breaking the trait signature
//! when chartml 5.x starts emitting tokens (e.g. for tab-close cleanup).

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

/// Inner state shared by every clone of a token. Cheap to construct; the
/// waker list stays empty unless somebody actually awaits `cancelled()`.
struct CancelInner {
    cancelled: AtomicBool,
    wakers: Mutex<Vec<Waker>>,
}

/// Cooperative cancellation handle. `Clone` is cheap (just an `Arc`).
///
/// `is_cancelled()` is a non-blocking check that providers can poll between
/// chunks of work. `cancelled()` returns a future that resolves the moment
/// cancellation flips, suitable for `tokio::select!` or `futures::select!`
/// against an upstream operation.
#[derive(Clone)]
pub struct CancellationToken(Arc<CancelInner>);

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken {
    /// Create a fresh, un-cancelled token.
    pub fn new() -> Self {
        Self(Arc::new(CancelInner {
            cancelled: AtomicBool::new(false),
            wakers: Mutex::new(Vec::new()),
        }))
    }

    /// Flip the cancellation flag and wake every pending `cancelled()` future.
    /// Idempotent — calling twice is harmless.
    pub fn cancel(&self) {
        // Use `swap` so we only wake wakers on the first flip; subsequent
        // calls observe the already-true flag and return immediately.
        if !self.0.cancelled.swap(true, Ordering::SeqCst) {
            let mut wakers = self.0.wakers.lock().expect("cancel waker lock poisoned");
            for waker in wakers.drain(..) {
                waker.wake();
            }
        }
    }

    /// Non-blocking check — returns `true` once `cancel()` has been called.
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::SeqCst)
    }

    /// Future that resolves when cancellation flips. Pending until then.
    /// Cheap to construct; stores at most one waker per polling task.
    pub fn cancelled(&self) -> Cancelled<'_> {
        Cancelled { token: self }
    }
}

impl std::fmt::Debug for CancellationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancellationToken")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

/// Future returned by `CancellationToken::cancelled()`.
pub struct Cancelled<'a> {
    token: &'a CancellationToken,
}

impl Future for Cancelled<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.token.is_cancelled() {
            return Poll::Ready(());
        }
        // Register the current waker so `cancel()` can wake us up later.
        let mut wakers = self
            .token
            .0
            .wakers
            .lock()
            .expect("cancel waker lock poisoned");
        // Re-check after acquiring the lock so we don't miss a cancel that
        // raced between the load above and the lock acquisition.
        if self.token.is_cancelled() {
            return Poll::Ready(());
        }
        // Replace any stale waker for this task (poll may move tasks).
        if !wakers.iter().any(|w| w.will_wake(cx.waker())) {
            wakers.push(cx.waker().clone());
        }
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn cancel_flips_flag() {
        let t = CancellationToken::new();
        assert!(!t.is_cancelled());
        t.cancel();
        assert!(t.is_cancelled());
    }

    #[test]
    fn cancel_is_idempotent() {
        let t = CancellationToken::new();
        t.cancel();
        t.cancel(); // must not panic / poison
        assert!(t.is_cancelled());
    }

    #[test]
    fn clones_share_state() {
        let t = CancellationToken::new();
        let t2 = t.clone();
        t.cancel();
        assert!(t2.is_cancelled());
    }

    #[tokio::test]
    async fn cancelled_future_resolves_after_cancel() {
        let t = CancellationToken::new();
        let t2 = t.clone();
        // Spawn a task that flips cancellation after a short delay.
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            t2.cancel();
        });
        t.cancelled().await;
        assert!(t.is_cancelled());
    }

    #[tokio::test]
    async fn cancelled_returns_immediately_if_already_cancelled() {
        let t = CancellationToken::new();
        t.cancel();
        // Should not block — if it does, the test will time out.
        t.cancelled().await;
    }
}
