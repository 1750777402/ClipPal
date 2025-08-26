use backon::{ExponentialBuilder, Retryable};
use log::{debug, info, warn};
use std::time::Duration;

/// 通用重试配置 - 基于 backon 的封装
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// 最大重试次数
    pub max_retries: usize,
    /// 初始延迟时间（毫秒）
    pub initial_delay_ms: u64,
    /// 退避倍数
    pub backoff_multiplier: f64,
    /// 最大延迟时间（毫秒）
    pub max_delay_ms: u64,
    /// 是否启用抖动
    pub enable_jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000,  // 1秒
            backoff_multiplier: 2.0, // 指数退避倍数
            max_delay_ms: 60000,     // 最大延迟1分钟
            enable_jitter: true,     // 启用抖动
        }
    }
}

impl RetryConfig {
    /// 创建新的重试配置
    pub fn new(max_retries: usize, initial_delay_ms: u64) -> Self {
        Self {
            max_retries,
            initial_delay_ms,
            ..Default::default()
        }
    }

    /// 设置退避倍数
    pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }

    /// 设置最大延迟时间
    pub fn with_max_delay(mut self, max_delay_ms: u64) -> Self {
        self.max_delay_ms = max_delay_ms;
        self
    }

    /// 设置是否启用抖动
    pub fn with_jitter(mut self, enable: bool) -> Self {
        self.enable_jitter = enable;
        self
    }

    /// 转换为 backon 的 ExponentialBuilder
    pub fn to_exponential_builder(&self) -> ExponentialBuilder {
        let mut builder = ExponentialBuilder::default()
            .with_max_times(self.max_retries)
            .with_min_delay(Duration::from_millis(self.initial_delay_ms))
            .with_max_delay(Duration::from_millis(self.max_delay_ms))
            .with_factor(self.backoff_multiplier as f32);

        if self.enable_jitter {
            builder = builder.with_jitter();
        }

        builder
    }
}

/// 使用 backon 执行带重试的异步操作
///
/// 这个函数封装了 backon 的使用，提供更友好的 API
pub async fn retry_with_backon<T, E, F, Fut>(
    config: RetryConfig,
    operation: F,
    should_retry: impl Fn(&E) -> bool + Send + Sync,
) -> Result<T, E>
where
    F: Fn() -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<T, E>> + Send,
    E: std::fmt::Display + Send + Sync,
{
    let start_time = std::time::Instant::now();
    let retry_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let retry_count_clone = retry_count.clone();

    // 使用 backon 的链式 API
    let result = operation
        .retry(config.to_exponential_builder())
        .when(move |e: &E| {
            let count = retry_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let should_retry_result = should_retry(e);

            if should_retry_result {
                debug!("第 {} 次尝试失败，准备重试: {}", count + 1, e);
            } else {
                warn!("遇到不可重试错误，停止重试: {}", e);
            }

            should_retry_result
        })
        .notify(|err: &E, duration: Duration| {
            warn!("操作失败，{:?} 后重试: {}", duration, err);
        })
        .await;

    let total_duration = start_time.elapsed();
    let attempts = retry_count.load(std::sync::atomic::Ordering::SeqCst) + 1;

    match &result {
        Ok(_) => {
            if attempts > 1 {
                info!(
                    "操作在第 {} 次尝试后成功，总耗时: {:?}",
                    attempts, total_duration
                );
            } else {
                debug!("操作首次成功，耗时: {:?}", total_duration);
            }
        }
        Err(e) => {
            warn!(
                "操作最终失败 (尝试 {} 次)，总耗时: {:?}: {}",
                attempts, total_duration, e
            );
        }
    }

    result
}

/// 使用默认配置的便捷函数
pub async fn retry_with_default<T, E, F, Fut>(
    operation: F,
    should_retry: impl Fn(&E) -> bool + Send + Sync,
) -> Result<T, E>
where
    F: Fn() -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<T, E>> + Send,
    E: std::fmt::Display + Send + Sync,
{
    retry_with_backon(RetryConfig::default(), operation, should_retry).await
}

/// 使用自定义配置的便捷函数
pub async fn retry_with_config<T, E, F, Fut>(
    config: RetryConfig,
    operation: F,
    should_retry: impl Fn(&E) -> bool + Send + Sync,
) -> Result<T, E>
where
    F: Fn() -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<T, E>> + Send,
    E: std::fmt::Display + Send + Sync,
{
    retry_with_backon(config, operation, should_retry).await
}

/// 便捷的重试宏，提供更简洁的使用方式
///
/// # 示例
/// ```rust
/// use crate::utils::retry_helper::{retry, RetryConfig};
///
/// let result = retry!(
///     RetryConfig::new(3, 1000),
///     || async { fetch_data().await },
///     |e| e.is_retryable()
/// ).await;
/// ```
#[macro_export]
macro_rules! retry {
    ($config:expr, $op:expr, $should_retry:expr) => {
        $crate::utils::retry_helper::retry_with_config($config, $op, $should_retry)
    };
    ($op:expr, $should_retry:expr) => {
        $crate::utils::retry_helper::retry_with_default($op, $should_retry)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[derive(Debug, PartialEq)]
    enum TestError {
        Retryable,
        NonRetryable,
    }

    impl std::fmt::Display for TestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                TestError::Retryable => write!(f, "Retryable error"),
                TestError::NonRetryable => write!(f, "Non-retryable error"),
            }
        }
    }

    impl std::error::Error for TestError {}

    #[tokio::test]
    async fn test_backon_successful_operation() {
        let config = RetryConfig::new(3, 10);

        let result =
            retry_with_config(config, || async { Ok::<i32, TestError>(42) }, |_| true).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_backon_retry_until_success() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let config = RetryConfig::new(5, 10);

        let result = retry_with_config(
            config,
            move || {
                let count = attempt_count_clone.clone();
                async move {
                    let current_attempt = count.fetch_add(1, Ordering::SeqCst);
                    if current_attempt < 2 {
                        Err(TestError::Retryable)
                    } else {
                        Ok(42)
                    }
                }
            },
            |e| matches!(e, TestError::Retryable),
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_backon_max_retries_reached() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let config = RetryConfig::new(3, 10);

        let result = retry_with_config(
            config,
            move || {
                let count = attempt_count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Err::<i32, TestError>(TestError::Retryable)
                }
            },
            |e| matches!(e, TestError::Retryable),
        )
        .await;

        assert!(result.is_err());
        // backon 的 max_times(3) 表示总共尝试 3+1=4 次（第一次 + 3次重试）
        assert_eq!(attempt_count.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn test_backon_non_retryable_error() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let config = RetryConfig::new(3, 10);

        let result = retry_with_config(
            config,
            move || {
                let count = attempt_count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Err::<i32, TestError>(TestError::NonRetryable)
                }
            },
            |e| matches!(e, TestError::Retryable), // 只重试 Retryable 错误
        )
        .await;

        assert!(result.is_err());
        assert_eq!(attempt_count.load(Ordering::SeqCst), 1); // 只尝试一次
    }

    #[tokio::test]
    async fn test_retry_macro() {
        let result = retry!(
            RetryConfig::new(2, 10),
            || async { Ok::<i32, TestError>(100) },
            |_| true
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 100);
    }
}
