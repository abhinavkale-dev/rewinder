use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::Serialize;
use serde_json::Value;

use crate::capture::permissions;
use crate::core::engine::Engine;
use crate::core::state::TriggerSourceDto;
use crate::events::EngineHost;
use crate::hotkeys::{HotkeyRegistration, RegistrationMode};
use crate::settings::{SettingsDto, SettingsPatchDto};

pub type RewinderEventCallback =
    extern "C" fn(event: *const c_char, json: *const c_char, ctx: *mut c_void);

struct RegisteredCallback {
    func: Option<RewinderEventCallback>,
    ctx: *mut c_void,
}

unsafe impl Send for RegisteredCallback {}
unsafe impl Sync for RegisteredCallback {}

struct FfiEngineHost {
    callback: Mutex<RegisteredCallback>,
}

impl FfiEngineHost {
    fn new() -> Self {
        Self {
            callback: Mutex::new(RegisteredCallback {
                func: None,
                ctx: std::ptr::null_mut(),
            }),
        }
    }

    fn set_callback(&self, func: Option<RewinderEventCallback>, ctx: *mut c_void) {
        let mut cb = self.callback.lock();
        cb.func = func;
        cb.ctx = ctx;
    }
}

impl EngineHost for FfiEngineHost {
    fn emit(&self, event: &str, payload: Value) {
        let cb = self.callback.lock();
        let Some(func) = cb.func else {
            return;
        };
        let Ok(event_c) = CString::new(event) else {
            return;
        };
        let json = serde_json::to_string(&payload).unwrap_or_else(|_| "null".to_string());
        let Ok(json_c) = CString::new(json) else {
            return;
        };
        func(event_c.as_ptr(), json_c.as_ptr(), cb.ctx);
    }

    fn replace_shortcuts(
        &self,
        primary: &str,
        _fallbacks: &[String],
    ) -> Result<HotkeyRegistration, String> {
        Ok(HotkeyRegistration {
            selected_hotkey: primary.to_string(),
            mode: RegistrationMode::Primary,
        })
    }
}

pub struct RewinderHandle {
    engine: Arc<Engine>,
    host: Arc<FfiEngineHost>,
    host_dyn: Arc<dyn EngineHost>,
    initialized: Mutex<bool>,
}

fn to_c_string(s: String) -> *mut c_char {
    CString::new(s)
        .unwrap_or_else(|_| CString::new("").expect("empty cstring"))
        .into_raw()
}

fn ok_json<T: Serialize>(value: T) -> *mut c_char {
    let data = serde_json::to_value(value).unwrap_or(Value::Null);
    to_c_string(serde_json::json!({ "ok": true, "data": data }).to_string())
}

fn err_json(message: impl Into<String>) -> *mut c_char {
    to_c_string(serde_json::json!({ "ok": false, "error": message.into() }).to_string())
}

fn install_panic_hook() {
    static PANIC_HOOK: std::sync::Once = std::sync::Once::new();
    PANIC_HOOK.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            eprintln!("phase: rust_panic {info}");
            prev(info);
        }));
    });
}

fn ffi_guard(f: impl FnOnce() -> *mut c_char) -> *mut c_char {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .unwrap_or_else(|_| err_json("internal error: engine panic was contained"))
}

fn ffi_guard_void(f: impl FnOnce()) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
}

unsafe fn handle_ref<'a>(handle: *mut RewinderHandle) -> Option<&'a RewinderHandle> {
    if handle.is_null() {
        None
    } else {
        Some(&*handle)
    }
}

unsafe fn input_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok()
}

#[no_mangle]
pub extern "C" fn rewinder_init() -> *mut RewinderHandle {
    install_panic_hook();
    std::panic::catch_unwind(|| {
        let settings = SettingsDto::default();
        let permission =
            permissions::detect_permissions_for_output_dir(settings.output_dir_path().as_path());
        let engine = Engine::new(settings, permission);
        let host = Arc::new(FfiEngineHost::new());
        let host_dyn: Arc<dyn EngineHost> = host.clone();
        let handle = Box::new(RewinderHandle {
            engine,
            host,
            host_dyn,
            initialized: Mutex::new(false),
        });
        Box::into_raw(handle)
    })
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn rewinder_set_event_callback(
    handle: *mut RewinderHandle,
    callback: Option<RewinderEventCallback>,
    ctx: *mut c_void,
) {
    ffi_guard_void(|| {
        let Some(h) = (unsafe { handle_ref(handle) }) else {
            return;
        };
        h.host.set_callback(callback, ctx);

        let mut initialized = h.initialized.lock();
        if !*initialized {
            *initialized = true;
            drop(initialized);
            let _ = h.engine.initialize(&h.host_dyn);
        }
    });
}

#[no_mangle]
pub extern "C" fn rewinder_shutdown(handle: *mut RewinderHandle) {
    if handle.is_null() {
        return;
    }
    ffi_guard_void(|| {
        let handle = unsafe { Box::from_raw(handle) };
        handle.engine.shutdown_for_app_exit("ffi_shutdown");
        handle.host.set_callback(None, std::ptr::null_mut());
        drop(handle);
    });
}

#[no_mangle]
pub extern "C" fn rewinder_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    ffi_guard_void(|| unsafe {
        drop(CString::from_raw(ptr));
    });
}

#[no_mangle]
pub extern "C" fn rewinder_get_engine_state(handle: *mut RewinderHandle) -> *mut c_char {
    ffi_guard(|| match unsafe { handle_ref(handle) } {
        Some(h) => ok_json(h.engine.get_engine_state()),
        None => err_json("null handle"),
    })
}

#[no_mangle]
pub extern "C" fn rewinder_get_settings(handle: *mut RewinderHandle) -> *mut c_char {
    ffi_guard(|| match unsafe { handle_ref(handle) } {
        Some(h) => ok_json(h.engine.get_settings()),
        None => err_json("null handle"),
    })
}

#[no_mangle]
pub extern "C" fn rewinder_default_settings() -> *mut c_char {
    ffi_guard(|| ok_json(SettingsDto::default()))
}

#[no_mangle]
pub extern "C" fn rewinder_update_settings(
    handle: *mut RewinderHandle,
    patch_json: *const c_char,
) -> *mut c_char {
    ffi_guard(|| {
