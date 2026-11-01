//! Asynchronous execution utilities, cancellation helpers, and exponential backoff.
//!
//! Asynchronous concurrency primitives, task cancellation tokens, and timeout helpers.

use rand::RngExt;
use std::future::Future;
use std::time::Duration;
pub use tokio_util::sync::CancellationToken;

/// Stack budget for threads that poll heavy M31A async tasks or deep recursion (16 MiB).
pub const THREAD_STACK_SIZE_BYTES: usize = 16 * 1024 * 1024;

const INITIAL_DELAY_MS: u64 = 200;
const BACKOFF_FACTOR: f64 = 2.0;

/// Cancellation error returned when an async operation was aborted via a [`CancellationToken`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CancelErr {
    #[error("operation cancelled")]
    Cancelled,
}

/// Extension trait to race any future against a [`CancellationToken`].
pub trait OrCancelExt: Sized {
    type Output;

    fn or_cancel(
        self,
        token: &CancellationToken,
    ) -> impl Future<Output = Result<Self::Output, CancelErr>> + Send;
}

impl<F> OrCancelExt for F
where
    F: Future + Send,
    F::Output: Send,
{
    type Output = F::Output;

    async fn or_cancel(self, token: &CancellationToken) -> Result<Self::Output, CancelErr> {
        tokio::select! {
            _ = token.cancelled() => Err(CancelErr::Cancelled),
            res = self => Ok(res),
        }
    }
}

/// Computes an exponential retry delay starting at 200ms and doubling on each subsequent attempt,
/// with up to +/- 10% jitter (0.9..1.1). Attempt 0 and 1 use the base 200ms delay.
pub fn backoff(attempt: u64) -> Duration {
    let exp = BACKOFF_FACTOR.powi(attempt.saturating_sub(1) as i32);
    let base = (INITIAL_DELAY_MS as f64 * exp) as u64;
    let jitter = rand::rng().random_range(0.9..1.1);
    Duration::from_millis(((base as f64 * jitter) as u64).max(1))
}

/// Spawns a dedicated OS thread configured with the 16 MiB stack budget.
pub fn spawn_bounded_thread<F, T>(name: &str, f: F) -> std::io::Result<std::thread::JoinHandle<T>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    std::thread::Builder::new()
        .name(name.to_string())
        .stack_size(THREAD_STACK_SIZE_BYTES)
        .spawn(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_ok_when_future_completes_first() {
        let token = CancellationToken::new();
        let value = async { 42 };
        let result = value.or_cancel(&token).await;
        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn returns_err_when_token_cancelled_first() {
        let token = CancellationToken::new();
        let token_clone = token.clone();

        let handle = tokio::task::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            token_clone.cancel();
        });

        let result = async {
            tokio::time::sleep(Duration::from_millis(150)).await;
            7
        }
        .or_cancel(&token)
        .await;

        handle.await.expect("task join failed");
        assert_eq!(result, Err(CancelErr::Cancelled));
    }

    #[tokio::test]
    async fn returns_err_when_token_already_cancelled() {
        let token = CancellationToken::new();
        token.cancel();

        let result = async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            99
        }
        .or_cancel(&token)
        .await;

        assert_eq!(result, Err(CancelErr::Cancelled));
    }

    #[test]
    fn test_backoff_calculations() {
        let d0 = backoff(0);
        let d1 = backoff(1);
        let d2 = backoff(2);
        let d3 = backoff(3);

        // Attempt 0 and 1 around 200ms (+/- 10%)
        assert!(d0.as_millis() >= 170 && d0.as_millis() <= 230);
        assert!(d1.as_millis() >= 170 && d1.as_millis() <= 230);
        // Attempt 2 around 400ms (+/- 10%)
        assert!(d2.as_millis() >= 350 && d2.as_millis() <= 450);
        // Attempt 3 around 800ms (+/- 10%)
        assert!(d3.as_millis() >= 700 && d3.as_millis() <= 900);
    }

    #[test]
    fn test_spawn_bounded_thread() {
        let handle = spawn_bounded_thread("test-stack-thread", || {
            // Allocate a relatively large array on stack to test stack budget
            let arr = [42u8; 1024 * 128];
            arr[100]
        })
        .expect("thread spawn failed");

        let res = handle.join().expect("join failed");
        assert_eq!(res, 42);
    }
}
