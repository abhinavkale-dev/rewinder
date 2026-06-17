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
        let Some(h) = (unsafe { handle_ref(handle) }) else {
            return err_json("null handle");
        };
        let text = unsafe { input_str(patch_json) }.unwrap_or("");
        let patch: SettingsPatchDto = match serde_json::from_str(text) {
            Ok(patch) => patch,
            Err(err) => return err_json(format!("invalid settings patch JSON: {err}")),
        };
        match h.engine.update_settings(&h.host_dyn, patch) {
            Ok(settings) => ok_json(settings),
            Err(err) => err_json(err),
        }
    })
}

#[no_mangle]
pub extern "C" fn rewinder_set_replay_enabled(
    handle: *mut RewinderHandle,
    enabled: bool,
) -> *mut c_char {
    ffi_guard(|| {
        let Some(h) = (unsafe { handle_ref(handle) }) else {
            return err_json("null handle");
        };
        match h.engine.set_replay_enabled(&h.host_dyn, enabled) {
            Ok(state) => ok_json(state),
            Err(err) => err_json(err),
        }
    })
}

#[no_mangle]
pub extern "C" fn rewinder_resume_capture(handle: *mut RewinderHandle) -> *mut c_char {
    ffi_guard(|| {
        let Some(h) = (unsafe { handle_ref(handle) }) else {
            return err_json("null handle");
        };
        match h.engine.resume_capture(&h.host_dyn) {
            Ok(state) => ok_json(state),
            Err(err) => err_json(err),
        }
    })
}

#[no_mangle]
pub extern "C" fn rewinder_trigger_save_replay(
    handle: *mut RewinderHandle,
    source_json: *const c_char,
) -> *mut c_char {
    ffi_guard(|| {
        let Some(h) = (unsafe { handle_ref(handle) }) else {
            return err_json("null handle");
        };
        let source = match unsafe { input_str(source_json) } {
            Some(text) if !text.trim().is_empty() && text.trim() != "null" => {
                match serde_json::from_str::<TriggerSourceDto>(text) {
                    Ok(source) => source,
                    Err(err) => return err_json(format!("invalid trigger source JSON: {err}")),
                }
            }
            _ => TriggerSourceDto::Manual,
        };
        ok_json(h.engine.trigger_save_replay(&h.host_dyn, source))
    })
}

#[no_mangle]
pub extern "C" fn rewinder_list_recent_clips(
    handle: *mut RewinderHandle,
    limit: usize,
) -> *mut c_char {
    ffi_guard(|| {
        let Some(h) = (unsafe { handle_ref(handle) }) else {
            return err_json("null handle");
        };
        let limit = if limit == 0 { None } else { Some(limit) };
        ok_json(h.engine.list_recent_clips(limit))
    })
}

#[no_mangle]
pub extern "C" fn rewinder_recheck_permissions(handle: *mut RewinderHandle) -> *mut c_char {
    ffi_guard(|| {
        let Some(h) = (unsafe { handle_ref(handle) }) else {
            return err_json("null handle");
        };
        ok_json(h.engine.recheck_permissions(&h.host_dyn))
    })
}

#[no_mangle]
pub extern "C" fn rewinder_request_microphone_permission(
    handle: *mut RewinderHandle,
) -> *mut c_char {
    ffi_guard(|| {
        let Some(h) = (unsafe { handle_ref(handle) }) else {
            return err_json("null handle");
        };
        ok_json(h.engine.request_microphone_permission(&h.host_dyn))
    })
}

#[no_mangle]
pub extern "C" fn rewinder_list_microphones() -> *mut c_char {
    ffi_guard(|| match permissions::list_microphones() {
        Ok(devices) => ok_json(devices),
        Err(err) => err_json(err),
    })
}

#[no_mangle]
pub extern "C" fn rewinder_grant_output_dir_access(handle: *mut RewinderHandle) -> *mut c_char {
    ffi_guard(|| {
        let Some(h) = (unsafe { handle_ref(handle) }) else {
            return err_json("null handle");
        };
        ok_json(h.engine.grant_output_dir_access(&h.host_dyn))
    })
}

#[no_mangle]
pub extern "C" fn rewinder_grant_screen_recording_access(
    handle: *mut RewinderHandle,
) -> *mut c_char {
    ffi_guard(|| {
        let Some(h) = (unsafe { handle_ref(handle) }) else {
            return err_json("null handle");
        };
        ok_json(h.engine.grant_screen_recording_access(&h.host_dyn))
    })
}

#[no_mangle]
pub extern "C" fn rewinder_grant_microphone_access(
    handle: *mut RewinderHandle,
    open_settings_if_denied: bool,
) -> *mut c_char {
    ffi_guard(|| {
        let Some(h) = (unsafe { handle_ref(handle) }) else {
            return err_json("null handle");
        };
        ok_json(
            h.engine
                .grant_microphone_access(&h.host_dyn, open_settings_if_denied),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static EVENT_COUNT: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn test_callback(event: *const c_char, json: *const c_char, _ctx: *mut c_void) {
        assert!(!event.is_null());
        assert!(!json.is_null());
        let event = unsafe { CStr::from_ptr(event) }.to_str().unwrap();
        let json = unsafe { CStr::from_ptr(json) }.to_str().unwrap();
        assert!(event.starts_with("rewinder://"));
        let _: Value = serde_json::from_str(json).unwrap();
        EVENT_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    fn read_and_free(ptr: *mut c_char) -> Value {
        assert!(!ptr.is_null());
        let text = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_string();
        rewinder_free_string(ptr);
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn ffi_command_envelopes_and_event_marshalling() {
        let handle = rewinder_init();
        assert!(!handle.is_null());

        let state = read_and_free(rewinder_get_engine_state(handle));
        assert_eq!(state["ok"], Value::Bool(true));
        assert!(state["data"].is_object());

        let settings = read_and_free(rewinder_get_settings(handle));
        assert_eq!(settings["ok"], Value::Bool(true));
        assert!(settings["data"].is_object());

        let clips = read_and_free(rewinder_list_recent_clips(handle, 5));
        assert_eq!(clips["ok"], Value::Bool(true));
        assert!(clips["data"].is_array());

        let mics = read_and_free(rewinder_list_microphones());
        assert!(mics["ok"].as_bool().is_some());

        let bad = CString::new("{not json").unwrap();
        let bad_result = read_and_free(rewinder_update_settings(handle, bad.as_ptr()));
        assert_eq!(bad_result["ok"], Value::Bool(false));
        assert!(bad_result["error"].is_string());

        let h = unsafe { &*handle };
        h.host
            .set_callback(Some(test_callback), std::ptr::null_mut());
        let before = EVENT_COUNT.load(Ordering::Relaxed);
        h.host_dyn
            .emit("rewinder://test", serde_json::json!({ "hello": "world" }));
        assert_eq!(EVENT_COUNT.load(Ordering::Relaxed), before + 1);

        h.host.set_callback(None, std::ptr::null_mut());
        h.host_dyn.emit("rewinder://test", Value::Null);
        assert_eq!(EVENT_COUNT.load(Ordering::Relaxed), before + 1);

        rewinder_shutdown(handle);
    }

    #[test]
    fn ffi_guard_contains_panics_as_error_envelope() {
        let contained = read_and_free(ffi_guard(|| panic!("boom")));
        assert_eq!(contained["ok"], Value::Bool(false));
        assert!(contained["error"].is_string());

        let normal = read_and_free(ffi_guard(|| ok_json(serde_json::json!({ "v": 1 }))));
        assert_eq!(normal["ok"], Value::Bool(true));
        assert_eq!(normal["data"]["v"], serde_json::json!(1));
    }
}
