/// 工具函数模块
///
/// 提供安全的并发访问辅助函数和通用工具
use std::sync::{Mutex, MutexGuard};

/// Release 模式下的警告日志宏
///
/// 在 debug 模式下输出到 stderr,
/// 在 release 模式下可以选择性地记录或忽略
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        eprintln!("[WARN] {}", format!($($arg)*));

        #[cfg(not(debug_assertions))]
        {
            // Release 模式下可选:写入日志文件或仅记录关键错误
            // 当前实现:静默忽略非关键警告
        }
    };
}

/// Release 模式下的错误日志宏(始终输出)
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        eprintln!("[ERROR] {}", format!($($arg)*));
    };
}

/// 安全地获取 Mutex 锁,如果锁中毒则恢复数据并记录警告
///
/// # 参数
/// * `mutex` - 要锁定的 Mutex
///
/// # 返回
/// * `MutexGuard<T>` - 锁守卫
///
/// # 安全性
/// 当锁中毒时(持有锁的线程 panic),此函数会:
/// 1. 记录警告日志
/// 2. 从 PoisonError 中恢复数据
/// 3. 返回有效的 MutexGuard
///
/// 这适用于 GUI 应用中的缓存访问等场景,其中数据不一致的风险较低,
/// 且程序应该继续运行而不是崩溃。
pub fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| {
        log_warn!("Mutex poisoned, recovering data");
        poisoned.into_inner()
    })
}

/// 将 usize 安全转换为 NonZeroUsize
///
/// # 参数
/// * `value` - 要转换的 usize 值
/// * `default` - 如果 value 为 0 时的默认值
///
/// # 返回
/// * `std::num::NonZeroUsize` - 非零的 usize
pub fn to_non_zero_usize(value: usize, default: usize) -> std::num::NonZeroUsize {
    std::num::NonZeroUsize::new(value).unwrap_or_else(|| {
        std::num::NonZeroUsize::new(default).expect("Default value must be non-zero")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_lock_or_recover_normal() {
        let mutex = Mutex::new(42);
        let guard = lock_or_recover(&mutex);
        assert_eq!(*guard, 42);
    }

    #[test]
    fn test_lock_or_recover_poisoned() {
        let mutex = Arc::new(Mutex::new(42));
        let mutex_clone = Arc::clone(&mutex);

        // 创建一个会 panic 的线程
        let handle = thread::spawn(move || {
            let _guard = mutex_clone.lock().unwrap();
            panic!("Intentional panic");
        });

        // 等待线程 panic
        let _ = handle.join();

        // 现在锁已中毒,但我们的函数应该能恢复
        let guard = lock_or_recover(&mutex);
        assert_eq!(*guard, 42);
    }

    #[test]
    fn test_to_non_zero_usize_valid() {
        let result = to_non_zero_usize(10, 5);
        assert_eq!(result.get(), 10);
    }

    #[test]
    fn test_to_non_zero_usize_zero() {
        let result = to_non_zero_usize(0, 5);
        assert_eq!(result.get(), 5);
    }

    #[test]
    #[should_panic(expected = "Default value must be non-zero")]
    fn test_to_non_zero_usize_invalid_default() {
        let _ = to_non_zero_usize(0, 0);
    }
}
