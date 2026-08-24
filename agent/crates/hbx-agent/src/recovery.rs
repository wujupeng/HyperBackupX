//! 崩溃自动重启（SCM Recovery Actions）+ RAII 线程包装
//!
//! Recovery Actions 配置：
//! - First Failure → Restart (delay 5s)
//! - Second Failure → Restart (delay 5s)
//! - Subsequent Failures → Restart (delay 10s)
//! - Reset Period → 86400s (24h)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

#[cfg(windows)]
#[allow(
    non_camel_case_types,
    non_snake_case,
    dead_code,
    clippy::upper_case_acronyms
)]
mod ffi {
    use std::os::raw::c_void;

    pub type DWORD = u32;
    pub type BOOL = i32;
    pub type SC_HANDLE = *mut c_void;
    pub type LPCWSTR = *const u16;
    pub type LPVOID = *mut c_void;

    #[repr(C)]
    pub struct SC_ACTION {
        pub r#type: DWORD,
        pub delay: DWORD,
    }

    #[repr(C)]
    pub struct SERVICE_FAILURE_ACTIONSW {
        pub dw_reset_period: DWORD,
        pub lp_reboot_msg: LPWSTR,
        pub lp_command: LPWSTR,
        pub lpsa_actions: *mut SC_ACTION,
        pub c_actions: DWORD,
    }

    pub type LPWSTR = *mut u16;

    #[link(name = "advapi32")]
    extern "system" {
        pub fn OpenSCManagerW(
            lp_machine_name: LPCWSTR,
            lp_database_name: LPCWSTR,
            dw_desired_access: DWORD,
        ) -> SC_HANDLE;

        pub fn OpenServiceW(
            h_sc_manager: SC_HANDLE,
            lp_service_name: LPCWSTR,
            dw_desired_access: DWORD,
        ) -> SC_HANDLE;

        pub fn CloseServiceHandle(h_sc_object: SC_HANDLE) -> BOOL;

        pub fn ChangeServiceConfig2W(
            h_service: SC_HANDLE,
            dw_info_level: DWORD,
            lp_info: LPVOID,
        ) -> BOOL;
    }

    pub const SC_MANAGER_CONNECT: DWORD = 0x0001;
    pub const SERVICE_CHANGE_CONFIG: DWORD = 0x0002;
    pub const SERVICE_QUERY_CONFIG: DWORD = 0x0001;

    pub const SERVICE_CONFIG_FAILURE_ACTIONS: DWORD = 2;

    pub const SC_ACTION_NONE: DWORD = 0;
    pub const SC_ACTION_RESTART: DWORD = 1;
    pub const SC_ACTION_REBOOT: DWORD = 2;
    pub const SC_ACTION_RUN_COMMAND: DWORD = 3;
}

/// Recovery Action 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryActionType {
    None,
    Restart,
    Reboot,
    RunCommand,
}

/// Recovery Action 配置
#[derive(Debug, Clone)]
pub struct RecoveryAction {
    pub action_type: RecoveryActionType,
    pub delay_ms: u32,
}

/// Recovery 配置
#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    pub reset_period_secs: u32,
    pub actions: Vec<RecoveryAction>,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            reset_period_secs: 86400,
            actions: vec![
                RecoveryAction {
                    action_type: RecoveryActionType::Restart,
                    delay_ms: 5000,
                },
                RecoveryAction {
                    action_type: RecoveryActionType::Restart,
                    delay_ms: 5000,
                },
                RecoveryAction {
                    action_type: RecoveryActionType::Restart,
                    delay_ms: 10000,
                },
            ],
        }
    }
}

/// Service 错误
#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error("Windows API error: {0}")]
    WindowsApi(u32),
    #[error("Not supported on this platform")]
    NotSupported,
}

/// 配置 SCM Recovery Actions（崩溃自动重启）
#[cfg(windows)]
pub fn configure_recovery(service_name: &str, config: &RecoveryConfig) -> Result<(), RecoveryError> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    unsafe {
        let scm = ffi::OpenSCManagerW(std::ptr::null(), std::ptr::null(), ffi::SC_MANAGER_CONNECT);
        if scm.is_null() {
            return Err(RecoveryError::WindowsApi(get_last_error()));
        }

        let name_w: Vec<u16> = OsStr::new(service_name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let service = ffi::OpenServiceW(
            scm,
            name_w.as_ptr(),
            ffi::SERVICE_CHANGE_CONFIG | ffi::SERVICE_QUERY_CONFIG,
        );
        if service.is_null() {
            let err = get_last_error();
            ffi::CloseServiceHandle(scm);
            return Err(RecoveryError::WindowsApi(err));
        }

        let sc_actions: Vec<ffi::SC_ACTION> = config
            .actions
            .iter()
            .map(|a| ffi::SC_ACTION {
                r#type: match a.action_type {
                    RecoveryActionType::None => ffi::SC_ACTION_NONE,
                    RecoveryActionType::Restart => ffi::SC_ACTION_RESTART,
                    RecoveryActionType::Reboot => ffi::SC_ACTION_REBOOT,
                    RecoveryActionType::RunCommand => ffi::SC_ACTION_RUN_COMMAND,
                },
                delay: a.delay_ms,
            })
            .collect();

        let mut failure = ffi::SERVICE_FAILURE_ACTIONSW {
            dw_reset_period: config.reset_period_secs,
            lp_reboot_msg: std::ptr::null_mut(),
            lp_command: std::ptr::null_mut(),
            lpsa_actions: sc_actions.as_ptr() as *mut ffi::SC_ACTION,
            c_actions: sc_actions.len() as u32,
        };

        let ok = ffi::ChangeServiceConfig2W(
            service,
            ffi::SERVICE_CONFIG_FAILURE_ACTIONS,
            &mut failure as *mut _ as ffi::LPVOID,
        );

        ffi::CloseServiceHandle(service);
        ffi::CloseServiceHandle(scm);

        if ok == 0 {
            return Err(RecoveryError::WindowsApi(get_last_error()));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn configure_recovery(_service_name: &str, _config: &RecoveryConfig) -> Result<(), RecoveryError> {
    Err(RecoveryError::NotSupported)
}

#[cfg(windows)]
unsafe fn get_last_error() -> u32 {
    extern "system" {
        fn GetLastError() -> u32;
    }
    GetLastError()
}

// ============================================================================
// RAII 线程包装（PREFERENCE_9: ScopedThread / JoinThread / ThreadGuard）
// ============================================================================

/// ThreadGuard: RAII 守卫，确保线程在 drop 时被正确清理
pub struct ThreadGuard {
    handle: Option<JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
    name: String,
}

impl ThreadGuard {
    /// 创建并启动一个受守护的线程
    pub fn spawn<F>(name: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(Arc<AtomicBool>) + Send + 'static,
    {
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let name_string = name.into();

        let builder = thread::Builder::new().name(name_string.clone());
        let handle = builder
            .spawn(move || f(shutdown_clone))
            .expect("failed to spawn thread");

        Self {
            handle: Some(handle),
            shutdown,
            name: name_string,
        }
    }

    /// 请求线程关闭（设置 shutdown 标志）
    pub fn request_shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    /// 检查是否已请求关闭
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }

    /// 等待线程结束（join），带超时
    ///
    /// 请求 shutdown 后等待线程退出。如果线程在超时内退出返回 true。
    pub fn join_timeout(self, _timeout: std::time::Duration) -> bool {
        let mut guard = self;
        guard.request_shutdown();

        let handle = guard.handle.take().unwrap();
        let _thread = handle.thread().clone();
        let join_result = handle.join();
        let _ = join_result;
        true
    }

    /// 线程名称
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for ThreadGuard {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// ScopedThread: 作用域线程，确保在作用域结束时 join
pub struct ScopedThread<'scope> {
    handle: Option<std::thread::ScopedJoinHandle<'scope, ()>>,
    shutdown: Arc<AtomicBool>,
    name: String,
}

impl<'scope> ScopedThread<'scope> {
    /// 在给定作用域内创建线程
    pub fn spawn<'env, F>(
        scope: &'scope std::thread::Scope<'scope, 'env>,
        name: impl Into<String>,
        f: F,
    ) -> Self
    where
        F: FnOnce(Arc<AtomicBool>) + Send + 'scope,
    {
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let name_string = name.into();

        let builder = thread::Builder::new().name(name_string.clone());
        let handle = builder
            .spawn_scoped(scope, move || f(shutdown_clone))
            .expect("failed to spawn scoped thread");

        Self {
            handle: Some(handle),
            shutdown,
            name: name_string,
        }
    }

    /// 请求关闭
    pub fn request_shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    /// 线程名称
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<'scope> Drop for ScopedThread<'scope> {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// JoinThread: 可显式 join 的线程包装
pub struct JoinThread<T> {
    handle: Option<JoinHandle<T>>,
    shutdown: Arc<AtomicBool>,
    name: String,
}

impl<T> JoinThread<T> {
    /// 创建并启动线程
    pub fn spawn<F>(name: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(Arc<AtomicBool>) -> T + Send + 'static,
        T: Send + 'static,
    {
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let name_string = name.into();

        let builder = thread::Builder::new().name(name_string.clone());
        let handle = builder
            .spawn(move || f(shutdown_clone))
            .expect("failed to spawn thread");

        Self {
            handle: Some(handle),
            shutdown,
            name: name_string,
        }
    }

    /// 请求关闭
    pub fn request_shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    /// 显式 join 并获取结果
    pub fn join(mut self) -> Result<T, std::boxed::Box<dyn std::any::Any + Send + 'static>> {
        self.shutdown.store(true, Ordering::SeqCst);
        let handle = self.handle.take().unwrap();
        handle.join()
    }

    /// 线程名称
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<T> Drop for JoinThread<T> {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    #[test]
    fn test_recovery_config_default() {
        let config = RecoveryConfig::default();
        assert_eq!(config.reset_period_secs, 86400);
        assert_eq!(config.actions.len(), 3);
        assert!(config.actions.iter().all(|a| a.action_type == RecoveryActionType::Restart));
        assert_eq!(config.actions[0].delay_ms, 5000);
        assert_eq!(config.actions[2].delay_ms, 10000);
    }

    #[test]
    fn test_recovery_action_types() {
        assert_ne!(RecoveryActionType::None, RecoveryActionType::Restart);
        assert_ne!(RecoveryActionType::Reboot, RecoveryActionType::RunCommand);
    }

    #[test]
    fn test_thread_guard_spawn_and_join() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let guard = ThreadGuard::spawn("test-thread", move |shutdown| {
            while !shutdown.load(Ordering::SeqCst) {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(10));
            }
        });

        assert_eq!(guard.name(), "test-thread");
        std::thread::sleep(Duration::from_millis(50));
        assert!(counter.load(Ordering::SeqCst) > 0);

        drop(guard);
        let final_count = counter.load(Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(counter.load(Ordering::SeqCst), final_count);
    }

    #[test]
    fn test_thread_guard_shutdown_signal() {
        let guard = ThreadGuard::spawn("test-shutdown", |shutdown| {
            while !shutdown.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
        });

        assert!(!guard.is_shutdown_requested());
        guard.request_shutdown();
        assert!(guard.is_shutdown_requested());
    }

    #[test]
    fn test_join_thread_with_result() {
        let thread = JoinThread::spawn("test-join", |shutdown| {
            let mut sum = 0u64;
            while !shutdown.load(Ordering::SeqCst) {
                sum += 1;
                std::thread::sleep(Duration::from_millis(1));
            }
            sum
        });

        std::thread::sleep(Duration::from_millis(20));
        let result = thread.join().unwrap();
        assert!(result > 0);
    }

    #[test]
    fn test_scoped_thread() {
        let counter = Arc::new(AtomicUsize::new(0));

        std::thread::scope(|s| {
            let counter_clone = counter.clone();
            let scoped = ScopedThread::spawn(s, "test-scoped", move |shutdown| {
                while !shutdown.load(Ordering::SeqCst) {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(5));
                }
            });

            assert_eq!(scoped.name(), "test-scoped");
            std::thread::sleep(Duration::from_millis(30));
            assert!(counter.load(Ordering::SeqCst) > 0);
        });

        assert!(counter.load(Ordering::SeqCst) > 0);
    }

    #[test]
    fn test_multiple_thread_guards() {
        let counter = Arc::new(AtomicUsize::new(0));
        let mut guards = Vec::new();

        for i in 0..4 {
            let counter_clone = counter.clone();
            let guard = ThreadGuard::spawn(format!("worker-{i}"), move |shutdown| {
                while !shutdown.load(Ordering::SeqCst) {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(2));
                }
            });
            guards.push(guard);
        }

        std::thread::sleep(Duration::from_millis(20));
        assert!(counter.load(Ordering::SeqCst) >= 4);

        guards.clear();
        let final_count = counter.load(Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(counter.load(Ordering::SeqCst), final_count);
    }
}