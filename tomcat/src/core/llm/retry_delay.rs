use std::future::Future;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio_util::sync::CancellationToken;

use crate::infra::error::AppError;

pub(crate) const PROVIDER_RETRY_BASE_DELAY_MS: u64 = 500;
pub(crate) const PROVIDER_RETRY_CAP_MS: u64 = 4_000;
const PROVIDER_RETRY_CANCELLED_MSG: &str = "provider retry backoff cancelled";

tokio::task_local! {
    static PROVIDER_RETRY_CANCEL: CancellationToken;
}

pub(crate) fn compute_provider_retry_delay_ms(
    base_delay_ms: u64,
    attempt: u32,
    jitter_seed: u64,
    cap_ms: u64,
) -> u64 {
    if base_delay_ms == 0 {
        return 0;
    }
    let base = base_delay_ms.saturating_mul(2u64.saturating_pow(attempt));
    let jitter_pct = 80 + (jitter_seed % 41);
    let jittered = base.saturating_mul(jitter_pct) / 100;
    jittered.min(cap_ms)
}

pub(crate) fn provider_retry_delay_with(base_delay_ms: u64, attempt: u32, cap_ms: u64) -> Duration {
    let jitter_seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::from(d.subsec_nanos()))
        .unwrap_or(20);
    Duration::from_millis(compute_provider_retry_delay_ms(
        base_delay_ms,
        attempt,
        jitter_seed,
        cap_ms,
    ))
}

pub(crate) fn provider_retry_delay(attempt: u32) -> Duration {
    provider_retry_delay_with(PROVIDER_RETRY_BASE_DELAY_MS, attempt, PROVIDER_RETRY_CAP_MS)
}

pub(crate) async fn with_provider_retry_cancel<T, F>(cancel: CancellationToken, fut: F) -> T
where
    F: Future<Output = T>,
{
    PROVIDER_RETRY_CANCEL.scope(cancel, fut).await
}

pub(crate) async fn sleep_provider_retry_delay(delay: Duration) -> Result<(), AppError> {
    if delay.is_zero() {
        return Ok(());
    }
    match PROVIDER_RETRY_CANCEL.try_with(Clone::clone) {
        Ok(cancel) => {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => Err(provider_retry_cancelled_error()),
                _ = tokio::time::sleep(delay) => Ok(()),
            }
        }
        Err(_) => {
            tokio::time::sleep(delay).await;
            Ok(())
        }
    }
}

pub(crate) fn is_provider_retry_cancelled(err: &AppError) -> bool {
    matches!(err, AppError::Llm(message) if message == PROVIDER_RETRY_CANCELLED_MSG)
}

fn provider_retry_cancelled_error() -> AppError {
    AppError::Llm(PROVIDER_RETRY_CANCELLED_MSG.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        compute_provider_retry_delay_ms, is_provider_retry_cancelled, sleep_provider_retry_delay,
        with_provider_retry_cancel, PROVIDER_RETRY_BASE_DELAY_MS, PROVIDER_RETRY_CAP_MS,
    };
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn provider_retry_delay_uses_jitter_window() {
        let base = PROVIDER_RETRY_BASE_DELAY_MS;
        assert_eq!(
            compute_provider_retry_delay_ms(base, 0, 0, PROVIDER_RETRY_CAP_MS),
            400
        );
        assert_eq!(
            compute_provider_retry_delay_ms(base, 0, 20, PROVIDER_RETRY_CAP_MS),
            500
        );
        assert_eq!(
            compute_provider_retry_delay_ms(base, 0, 40, PROVIDER_RETRY_CAP_MS),
            600
        );
        assert_eq!(
            compute_provider_retry_delay_ms(base, 1, 0, PROVIDER_RETRY_CAP_MS),
            800
        );
        assert_eq!(
            compute_provider_retry_delay_ms(base, 1, 40, PROVIDER_RETRY_CAP_MS),
            1200
        );
    }

    #[test]
    fn provider_retry_delay_caps_and_saturates_large_attempts() {
        assert_eq!(
            compute_provider_retry_delay_ms(
                PROVIDER_RETRY_BASE_DELAY_MS,
                63,
                40,
                PROVIDER_RETRY_CAP_MS,
            ),
            PROVIDER_RETRY_CAP_MS
        );
        assert_eq!(
            compute_provider_retry_delay_ms(0, 10, 20, PROVIDER_RETRY_CAP_MS),
            0
        );
    }

    #[tokio::test(start_paused = true)]
    async fn scoped_retry_sleep_wakes_immediately_on_cancel() {
        let cancel = CancellationToken::new();
        let wait = tokio::spawn(with_provider_retry_cancel(cancel.clone(), async move {
            sleep_provider_retry_delay(Duration::from_secs(4)).await
        }));
        tokio::task::yield_now().await;
        cancel.cancel();
        let result = wait.await.expect("join");
        assert!(matches!(result, Err(err) if is_provider_retry_cancelled(&err)));
    }

    #[tokio::test(start_paused = true)]
    async fn unscoped_retry_sleep_waits_full_delay() {
        let wait = tokio::spawn(async { sleep_provider_retry_delay(Duration::from_secs(4)).await });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(3)).await;
        assert!(
            !wait.is_finished(),
            "unscoped sleep should still be waiting"
        );
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(wait.await.expect("join").is_ok());
    }
}
