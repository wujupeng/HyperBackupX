//! Windows Service 生命周期管理（SCM 注册 + 开机自启）
//!
//! 使用 raw FFI 调用 advapi32.dll 的 SCM API，无需外部 crate。

use std::ffi::OsStr;
use std::io;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::ptr;

#[cfg(windows)]
#[allow(
    non_camel_case_types,
    non_snake_case,
    dead_code,
    clippy::upper_case_acronyms
)]
mod ffi {
    use std::os::raw::{c_int, c_void};

    pub type DWORD = u32;
    pub type BOOL = c_int;
    pub type HANDLE = *mut c_void;
    pub type SC_HANDLE = HANDLE;
    pub type SERVICE_STATUS_HANDLE = HANDLE;
    pub type SIZE_T = usize;
    pub type LPWSTR = *mut u16;
    pub type LPCWSTR = *const u16;

    pub const SERVICE_NO_CHANGE: DWORD = 0xFFFFFFFF;

    #[repr(C)]
    pub struct SERVICE_STATUS {
        pub dw_service_type: DWORD,
        pub dw_current_state: DWORD,
        pub dw_controls_accepted: DWORD,
        pub dw_win32_exit_code: DWORD,
        pub dw_service_specific_exit_code: DWORD,
        pub dw_check_point: DWORD,
        pub dw_wait_hint: DWORD,
    }

    #[link(name = "advapi32")]
    extern "system" {
        pub fn OpenSCManagerW(
            lp_machine_name: LPCWSTR,
            lp_database_name: LPCWSTR,
            dw_desired_access: DWORD,
        ) -> SC_HANDLE;

        pub fn CreateServiceW(
            h_sc_manager: SC_HANDLE,
            lp_service_name: LPCWSTR,
            lp_display_name: LPCWSTR,
            dw_desired_access: DWORD,
            dw_service_type: DWORD,
            dw_start_type: DWORD,
            dw_error_control: DWORD,
            lp_binary_path_name: LPCWSTR,
            lp_load_order_group: LPCWSTR,
            lpdw_tag_id: *mut DWORD,
            lp_dependencies: LPCWSTR,
            lp_service_start_name: LPCWSTR,
            lp_password: LPCWSTR,
        ) -> SC_HANDLE;

        pub fn OpenServiceW(
            h_sc_manager: SC_HANDLE,
            lp_service_name: LPCWSTR,
            dw_desired_access: DWORD,
        ) -> SC_HANDLE;

        pub fn StartServiceW(
            h_service: SC_HANDLE,
            dw_num_service_args: DWORD,
            lp_service_arg_vectors: *const LPCWSTR,
        ) -> BOOL;

        pub fn DeleteService(h_service: SC_HANDLE) -> BOOL;

        pub fn CloseServiceHandle(h_sc_object: SC_HANDLE) -> BOOL;

        pub fn ChangeServiceConfigW(
            h_service: SC_HANDLE,
            dw_service_type: DWORD,
            dw_start_type: DWORD,
            dw_error_control: DWORD,
            lp_binary_path_name: LPCWSTR,
            lp_load_order_group: LPCWSTR,
            lpdw_tag_id: *mut DWORD,
            lp_dependencies: LPCWSTR,
            lp_service_start_name: LPCWSTR,
            lp_password: LPCWSTR,
            lp_display_name: LPCWSTR,
        ) -> BOOL;

        pub fn ChangeServiceConfig2W(
            h_service: SC_HANDLE,
            dw_info_level: DWORD,
            lp_info: *mut c_void,
        ) -> BOOL;
    }

    pub const SC_MANAGER_CONNECT: DWORD = 0x0001;
    pub const SC_MANAGER_CREATE_SERVICE: DWORD = 0x0002;
    pub const SC_MANAGER_ALL_ACCESS: DWORD = 0xF003F;

    pub const SERVICE_QUERY_CONFIG: DWORD = 0x0001;
    pub const SERVICE_CHANGE_CONFIG: DWORD = 0x0002;
    pub const SERVICE_START: DWORD = 0x0010;
    pub const SERVICE_DELETE: DWORD = 0x10000;
    pub const SERVICE_ALL_ACCESS: DWORD = 0xF01FF;

    pub const SERVICE_WIN32_OWN_PROCESS: DWORD = 0x00000010;
    pub const SERVICE_INTERACTIVE_PROCESS: DWORD = 0x00000100;

    pub const SERVICE_BOOT_START: DWORD = 0x00000000;
    pub const SERVICE_SYSTEM_START: DWORD = 0x00000001;
    pub const SERVICE_AUTO_START: DWORD = 0x00000002;
    pub const SERVICE_DEMAND_START: DWORD = 0x00000003;
    pub const SERVICE_DISABLED: DWORD = 0x00000004;

    pub const SERVICE_ERROR_IGNORE: DWORD = 0x00000000;
    pub const SERVICE_ERROR_NORMAL: DWORD = 0x00000001;
    pub const SERVICE_ERROR_SEVERE: DWORD = 0x00000002;
    pub const SERVICE_ERROR_CRITICAL: DWORD = 0x00000003;

    pub const SERVICE_STOPPED: DWORD = 0x00000001;
    pub const SERVICE_START_PENDING: DWORD = 0x00000002;
    pub const SERVICE_RUNNING: DWORD = 0x00000004;
    pub const SERVICE_STOP_PENDING: DWORD = 0x00000020;

    pub const SERVICE_ACCEPT_STOP: DWORD = 0x00000001;
    pub const SERVICE_ACCEPT_SHUTDOWN: DWORD = 0x00000004;

    pub const SERVICE_CONFIG_FAILURE_ACTIONS: DWORD = 2;
    pub const SERVICE_CONFIG_DELAYED_AUTO_START: DWORD = 3;

    pub const SC_ACTION_NONE: DWORD = 0;
    pub const SC_ACTION_RESTART: DWORD = 1;
    pub const SC_ACTION_REBOOT: DWORD = 2;
    pub const SC_ACTION_RUN_COMMAND: DWORD = 3;

    #[repr(C)]
    pub struct SC_ACTION {
        pub rtype: DWORD,
        pub delay: DWORD,
    }

    #[repr(C)]
    pub struct SERVICE_FAILURE_ACTIONSW {
        pub dw_reset_period: DWORD,
        pub lp_reboot_msg: LPWSTR,
        pub lp_command: LPWSTR,
        pub c_actions: DWORD,
        pub lpsa_actions: *mut SC_ACTION,
    }

    #[repr(C)]
    pub struct SERVICE_DELAYED_AUTO_START_INFO {
        pub f_delayed_auto_start: BOOL,
    }
}

/// Service 配置
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub binary_path: String,
    pub auto_start: bool,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: "HyperBackupXAgent".to_string(),
            display_name: "HyperBackup X Agent".to_string(),
            description: "HyperBackup X backup agent service".to_string(),
            binary_path: std::env::current_exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            auto_start: true,
        }
    }
}

/// Service 控制状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    Stopped,
    StartPending,
    Running,
    StopPending,
}

impl ServiceState {
    #[cfg(windows)]
    #[allow(dead_code, clippy::wrong_self_convention)]
    fn to_dw(self) -> ffi::DWORD {
        match self {
            ServiceState::Stopped => ffi::SERVICE_STOPPED,
            ServiceState::StartPending => ffi::SERVICE_START_PENDING,
            ServiceState::Running => ffi::SERVICE_RUNNING,
            ServiceState::StopPending => ffi::SERVICE_STOP_PENDING,
        }
    }
}

/// Service 错误
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Windows API error: {0}")]
    WindowsApi(u32),
    #[error("Service already exists: {0}")]
    AlreadyExists(String),
    #[error("Service not found: {0}")]
    NotFound(String),
    #[error("Not supported on this platform")]
    NotSupported,
}

#[cfg(windows)]
fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn last_error() -> ServiceError {
    ServiceError::WindowsApi(unsafe { windows_sys_last_error() as u32 })
}

#[cfg(windows)]
unsafe extern "system" fn windows_sys_last_error() -> i32 {
    extern "system" {
        fn GetLastError() -> u32;
    }
    GetLastError() as i32
}

/// 注册 Windows Service（SCM CreateService + 开机自启）
#[cfg(windows)]
pub fn register_service(config: &ServiceConfig) -> Result<(), ServiceError> {
    unsafe {
        let scm = ffi::OpenSCManagerW(
            ptr::null(),
            ptr::null(),
            ffi::SC_MANAGER_CONNECT | ffi::SC_MANAGER_CREATE_SERVICE,
        );
        if scm.is_null() {
            return Err(last_error());
        }

        let name_w = to_wide(&config.name);
        let display_w = to_wide(&config.display_name);
        let path_w = to_wide(&config.binary_path);

        let start_type = if config.auto_start {
            ffi::SERVICE_AUTO_START
        } else {
            ffi::SERVICE_DEMAND_START
        };

        let service = ffi::CreateServiceW(
            scm,
            name_w.as_ptr(),
            display_w.as_ptr(),
            ffi::SERVICE_ALL_ACCESS,
            ffi::SERVICE_WIN32_OWN_PROCESS,
            start_type,
            ffi::SERVICE_ERROR_NORMAL,
            path_w.as_ptr(),
            ptr::null(),
            ptr::null_mut(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
        );

        if service.is_null() {
            let err = last_error();
            ffi::CloseServiceHandle(scm);
            if let ServiceError::WindowsApi(183) = err {
                return Err(ServiceError::AlreadyExists(config.name.clone()));
            }
            return Err(err);
        }

        ffi::CloseServiceHandle(service);
        ffi::CloseServiceHandle(scm);
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn register_service(_config: &ServiceConfig) -> Result<(), ServiceError> {
    Err(ServiceError::NotSupported)
}

/// 注销 Windows Service
#[cfg(windows)]
pub fn unregister_service(name: &str) -> Result<(), ServiceError> {
    unsafe {
        let scm = ffi::OpenSCManagerW(ptr::null(), ptr::null(), ffi::SC_MANAGER_CONNECT);
        if scm.is_null() {
            return Err(last_error());
        }

        let name_w = to_wide(name);
        let service = ffi::OpenServiceW(
            scm,
            name_w.as_ptr(),
            ffi::SERVICE_DELETE | ffi::SERVICE_QUERY_CONFIG,
        );
        if service.is_null() {
            let err = last_error();
            ffi::CloseServiceHandle(scm);
            if let ServiceError::WindowsApi(1060) = err {
                return Err(ServiceError::NotFound(name.to_string()));
            }
            return Err(err);
        }

        let ok = ffi::DeleteService(service);
        ffi::CloseServiceHandle(service);
        ffi::CloseServiceHandle(scm);

        if ok == 0 {
            return Err(last_error());
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn unregister_service(_name: &str) -> Result<(), ServiceError> {
    Err(ServiceError::NotSupported)
}

/// 启动已注册的 Service
#[cfg(windows)]
pub fn start_service(name: &str) -> Result<(), ServiceError> {
    unsafe {
        let scm = ffi::OpenSCManagerW(ptr::null(), ptr::null(), ffi::SC_MANAGER_CONNECT);
        if scm.is_null() {
            return Err(last_error());
        }

        let name_w = to_wide(name);
        let service = ffi::OpenServiceW(scm, name_w.as_ptr(), ffi::SERVICE_START);
        if service.is_null() {
            let err = last_error();
            ffi::CloseServiceHandle(scm);
            return Err(err);
        }

        let ok = ffi::StartServiceW(service, 0, ptr::null());
        ffi::CloseServiceHandle(service);
        ffi::CloseServiceHandle(scm);

        if ok == 0 {
            let err = last_error();
            if let ServiceError::WindowsApi(1056) = err {
                return Ok(());
            }
            return Err(err);
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn start_service(_name: &str) -> Result<(), ServiceError> {
    Err(ServiceError::NotSupported)
}

/// 修改 Service 启动类型（开机自启/手动）
#[cfg(windows)]
pub fn set_start_type(name: &str, auto_start: bool) -> Result<(), ServiceError> {
    unsafe {
        let scm = ffi::OpenSCManagerW(ptr::null(), ptr::null(), ffi::SC_MANAGER_CONNECT);
        if scm.is_null() {
            return Err(last_error());
        }

        let name_w = to_wide(name);
        let service = ffi::OpenServiceW(
            scm,
            name_w.as_ptr(),
            ffi::SERVICE_CHANGE_CONFIG,
        );
        if service.is_null() {
            let err = last_error();
            ffi::CloseServiceHandle(scm);
            return Err(err);
        }

        let start_type = if auto_start {
            ffi::SERVICE_AUTO_START
        } else {
            ffi::SERVICE_DEMAND_START
        };

        let ok = ffi::ChangeServiceConfigW(
            service,
            ffi::SERVICE_NO_CHANGE,
            start_type,
            ffi::SERVICE_NO_CHANGE,
            ptr::null(),
            ptr::null(),
            ptr::null_mut(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
        );

        ffi::CloseServiceHandle(service);
        ffi::CloseServiceHandle(scm);

        if ok == 0 {
            return Err(last_error());
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set_start_type(_name: &str, _auto_start: bool) -> Result<(), ServiceError> {
    Err(ServiceError::NotSupported)
}

#[cfg(windows)]
pub fn configure_failure_recovery(name: &str) -> Result<(), ServiceError> {
    unsafe {
        let scm = ffi::OpenSCManagerW(ptr::null(), ptr::null(), ffi::SC_MANAGER_CONNECT);
        if scm.is_null() {
            return Err(last_error());
        }

        let name_w = to_wide(name);
        let service = ffi::OpenServiceW(
            scm,
            name_w.as_ptr(),
            ffi::SERVICE_CHANGE_CONFIG,
        );
        if service.is_null() {
            let err = last_error();
            ffi::CloseServiceHandle(scm);
            return Err(err);
        }

        let actions = [
            ffi::SC_ACTION { rtype: ffi::SC_ACTION_RESTART, delay: 5000 },
            ffi::SC_ACTION { rtype: ffi::SC_ACTION_RESTART, delay: 5000 },
            ffi::SC_ACTION { rtype: ffi::SC_ACTION_RESTART, delay: 5000 },
        ];

        let failure_actions = ffi::SERVICE_FAILURE_ACTIONSW {
            dw_reset_period: 60,
            lp_reboot_msg: ptr::null_mut(),
            lp_command: ptr::null_mut(),
            c_actions: actions.len() as ffi::DWORD,
            lpsa_actions: actions.as_ptr() as *mut ffi::SC_ACTION,
        };

        let ok = ffi::ChangeServiceConfig2W(
            service,
            ffi::SERVICE_CONFIG_FAILURE_ACTIONS,
            &failure_actions as *const _ as *mut std::os::raw::c_void,
        );

        if ok == 0 {
            let err = last_error();
            ffi::CloseServiceHandle(service);
            ffi::CloseServiceHandle(scm);
            return Err(err);
        }

        let delayed_info = ffi::SERVICE_DELAYED_AUTO_START_INFO {
            f_delayed_auto_start: 1,
        };

        let ok = ffi::ChangeServiceConfig2W(
            service,
            ffi::SERVICE_CONFIG_DELAYED_AUTO_START,
            &delayed_info as *const _ as *mut std::os::raw::c_void,
        );

        ffi::CloseServiceHandle(service);
        ffi::CloseServiceHandle(scm);

        if ok == 0 {
            return Err(last_error());
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn configure_failure_recovery(_name: &str) -> Result<(), ServiceError> {
    Err(ServiceError::NotSupported)
}

#[cfg(windows)]
pub fn enable_delayed_auto_start(name: &str) -> Result<(), ServiceError> {
    unsafe {
        let scm = ffi::OpenSCManagerW(ptr::null(), ptr::null(), ffi::SC_MANAGER_CONNECT);
        if scm.is_null() {
            return Err(last_error());
        }

        let name_w = to_wide(name);
        let service = ffi::OpenServiceW(
            scm,
            name_w.as_ptr(),
            ffi::SERVICE_CHANGE_CONFIG,
        );
        if service.is_null() {
            let err = last_error();
            ffi::CloseServiceHandle(scm);
            return Err(err);
        }

        let delayed_info = ffi::SERVICE_DELAYED_AUTO_START_INFO {
            f_delayed_auto_start: 1,
        };

        let ok = ffi::ChangeServiceConfig2W(
            service,
            ffi::SERVICE_CONFIG_DELAYED_AUTO_START,
            &delayed_info as *const _ as *mut std::os::raw::c_void,
        );

        ffi::CloseServiceHandle(service);
        ffi::CloseServiceHandle(scm);

        if ok == 0 {
            return Err(last_error());
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn enable_delayed_auto_start(_name: &str) -> Result<(), ServiceError> {
    Err(ServiceError::NotSupported)
}


/// 检查 Service 是否已注册
#[cfg(windows)]
pub fn is_registered(name: &str) -> bool {
    unsafe {
        let scm = ffi::OpenSCManagerW(ptr::null(), ptr::null(), ffi::SC_MANAGER_CONNECT);
        if scm.is_null() {
            return false;
        }

        let name_w = to_wide(name);
        let service = ffi::OpenServiceW(scm, name_w.as_ptr(), ffi::SERVICE_QUERY_CONFIG);
        let registered = !service.is_null();

        if !service.is_null() {
            ffi::CloseServiceHandle(service);
        }
        ffi::CloseServiceHandle(scm);
        registered
    }
}

#[cfg(not(windows))]
pub fn is_registered(_name: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_config_default() {
        let config = ServiceConfig::default();
        assert_eq!(config.name, "HyperBackupXAgent");
        assert!(config.auto_start);
        assert!(!config.binary_path.is_empty());
    }

    #[test]
    fn test_service_state_values() {
        assert_ne!(ServiceState::Stopped, ServiceState::Running);
        assert_ne!(ServiceState::StartPending, ServiceState::StopPending);
    }

    #[test]
    fn test_service_config_custom() {
        let config = ServiceConfig {
            name: "TestService".to_string(),
            display_name: "Test".to_string(),
            description: "A test service".to_string(),
            binary_path: "C:\\test\\agent.exe".to_string(),
            auto_start: false,
        };
        assert_eq!(config.name, "TestService");
        assert!(!config.auto_start);
    }

    #[cfg(not(windows))]
    #[test]
    fn test_not_supported_on_non_windows() {
        let config = ServiceConfig::default();
        let result = register_service(&config);
        assert!(matches!(result, Err(ServiceError::NotSupported)));
    }

    #[cfg(not(windows))]
    #[test]
    fn test_configure_failure_recovery_not_supported() {
        let result = configure_failure_recovery("test");
        assert!(matches!(result, Err(ServiceError::NotSupported)));
    }

    #[cfg(not(windows))]
    #[test]
    fn test_enable_delayed_auto_start_not_supported() {
        let result = enable_delayed_auto_start("test");
        assert!(matches!(result, Err(ServiceError::NotSupported)));
    }

    #[test]
    fn test_service_config_session0_isolation() {
        let config = ServiceConfig::default();
        assert_eq!(config.name, "HyperBackupXAgent");
    }

    #[test]
    fn test_failure_recovery_constants() {
        #[cfg(windows)]
        {
            assert_eq!(ffi::SC_ACTION_RESTART, 1);
            assert_eq!(ffi::SERVICE_CONFIG_FAILURE_ACTIONS, 2);
            assert_eq!(ffi::SERVICE_CONFIG_DELAYED_AUTO_START, 3);
        }
    }
}