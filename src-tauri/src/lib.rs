#[allow(dead_code)]
mod buffer;
mod capture;
mod commands;
mod core;
mod encoding;
mod events;
mod hotkeys;
mod settings;
mod writer;

use std::collections::HashMap;
use std::net::{TcpStream, ToSocketAddrs};
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use capture::permissions;
use core::engine::Engine;
use core::state::{EngineStateDto, LifecycleState, TriggerSourceDto};
use parking_lot::Mutex;
use settings::{SettingsDto, SettingsPatchDto};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{ActivationPolicy, AppHandle, Manager, Url, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;

const DEV_WATCHDOG_POLL_INTERVAL_MS: u64 = 2_000;
const DEV_WATCHDOG_CONNECT_TIMEOUT_MS: u64 = 600;
const DEV_WATCHDOG_FAILURE_THRESHOLD: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeLifecyclePolicy {
    hide_on_close: bool,
    tray_enabled: bool,
    use_accessory_activation: bool,
    start_dev_watchdog: bool,
}

pub struct TrayHandles {
    pub primary_action: MenuItem<tauri::Wry>,
    pub status_line: MenuItem<tauri::Wry>,
    pub res_items: Vec<(u16, CheckMenuItem<tauri::Wry>)>,
    pub dur_items: Vec<(u16, CheckMenuItem<tauri::Wry>)>,
    pub audio_system: CheckMenuItem<tauri::Wry>,
    pub audio_system_mic: CheckMenuItem<tauri::Wry>,
}

pub struct AppState {
    pub engine: Arc<Engine>,
    pub tray: Mutex<Option<TrayHandles>>,
    pub shutdown_started: Arc<AtomicBool>,
}

const fn runtime_lifecycle_policy(is_dev_runtime: bool) -> RuntimeLifecyclePolicy {
    if is_dev_runtime {
        RuntimeLifecyclePolicy {
            hide_on_close: false,
            tray_enabled: true,
            use_accessory_activation: false,
            start_dev_watchdog: true,
        }
    } else {
        RuntimeLifecyclePolicy {
            hide_on_close: true,
            tray_enabled: true,
            use_accessory_activation: true,
            start_dev_watchdog: false,
        }
    }
}

fn next_watchdog_failure_count(previous: u8, healthy: bool) -> u8 {
    if healthy {
        0
    } else {
        previous
            .saturating_add(1)
            .min(DEV_WATCHDOG_FAILURE_THRESHOLD)
    }
}

fn classify_dev_watchdog_shutdown_reason(
    parent_failures: u8,
    dev_server_failures: u8,
) -> Option<&'static str> {
    if parent_failures >= DEV_WATCHDOG_FAILURE_THRESHOLD {
        return Some("parent_host_lost");
    }
    if dev_server_failures >= DEV_WATCHDOG_FAILURE_THRESHOLD {
        return Some("dev_server_lost");
    }
    None
}

fn parse_single_pid(output: &str) -> Option<u32> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .and_then(|line| line.parse::<u32>().ok())
}

fn current_parent_pid() -> Option<u32> {
    let output = Command::new("ps")
        .args(["-o", "ppid=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_single_pid(&String::from_utf8_lossy(&output.stdout))
}

fn process_is_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn resolve_dev_server_url(app: &AppHandle) -> Option<Url> {
    app.get_webview_window("main")?.url().ok()
}

fn dev_server_is_reachable(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return true;
    };
    let Some(port) = url.port_or_known_default() else {
        return true;
    };
    let Ok(addrs) = format!("{host}:{port}").to_socket_addrs() else {
        return false;
    };
    addrs.into_iter().any(|addr| {
        TcpStream::connect_timeout(
            &addr,
            Duration::from_millis(DEV_WATCHDOG_CONNECT_TIMEOUT_MS),
        )
        .is_ok()
    })
}

fn shutdown_app(app: &AppHandle, reason: &'static str) {
    let app_state: tauri::State<'_, AppState> = app.state();
    if app_state.shutdown_started.swap(true, Ordering::Relaxed) {
        return;
    }
    app_state.engine.shutdown_for_app_exit(reason);
    app.exit(0);
}

fn start_dev_watchdog(app: &AppHandle) {
    let shutdown_started = {
        let app_state: tauri::State<'_, AppState> = app.state();
        Arc::clone(&app_state.shutdown_started)
    };
    let app_handle = app.clone();
    let dev_server_url = resolve_dev_server_url(app);
    let initial_parent_pid = current_parent_pid();

    std::thread::spawn(move || {
        let mut parent_failures = 0_u8;
        let mut dev_server_failures = 0_u8;

        loop {
            std::thread::sleep(Duration::from_millis(DEV_WATCHDOG_POLL_INTERVAL_MS));
            if shutdown_started.load(Ordering::Relaxed) {
                break;
            }

            let parent_healthy = initial_parent_pid.map(process_is_alive).unwrap_or(true);
            parent_failures = next_watchdog_failure_count(parent_failures, parent_healthy);

            let dev_server_healthy = dev_server_url
                .as_ref()
                .map(dev_server_is_reachable)
                .unwrap_or(true);
            dev_server_failures =
                next_watchdog_failure_count(dev_server_failures, dev_server_healthy);

            if let Some(reason) =
                classify_dev_watchdog_shutdown_reason(parent_failures, dev_server_failures)
            {
                shutdown_app(&app_handle, reason);
                break;
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings = SettingsDto::default();
    let permission =
        permissions::detect_permissions_for_output_dir(settings.output_dir_path().as_path());
    let engine = Engine::new(settings, permission);
    let lifecycle_policy = runtime_lifecycle_policy(tauri::is_dev());
    let pressed_hotkeys: Arc<Mutex<HashMap<u32, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler({
                    let engine = Arc::clone(&engine);
                    let pressed_hotkeys = Arc::clone(&pressed_hotkeys);
                    move |app, _shortcut, event| {
                        let now = Instant::now();
                        let hotkey_id = event.id();
                        let should_trigger = {
                            let mut pressed = pressed_hotkeys.lock();
                            pressed.retain(|_, seen_at| {
                                now.saturating_duration_since(*seen_at) < Duration::from_secs(4)
                            });
                            match event.state() {
                                ShortcutState::Pressed => {
                                    if pressed.contains_key(&hotkey_id) {
                                        false
                                    } else {
                                        pressed.insert(hotkey_id, now);
                                        true
                                    }
                                }
                                ShortcutState::Released => {
                                    pressed.remove(&hotkey_id);
                                    false
                                }
                            }
                        };
                        if should_trigger {
                            engine.trigger_save_replay_hotkey(app.clone());
                        } else if event.state() == ShortcutState::Pressed {
                            engine.note_hotkey_repeat_ignored();
                        }
                    }
                })
                .build(),
        )
        .manage(AppState {
            engine: Arc::clone(&engine),
            tray: Mutex::new(None),
            shutdown_started: Arc::new(AtomicBool::new(false)),
        })
        .on_window_event(move |window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if lifecycle_policy.hide_on_close {
                    let _ = window.hide();
                } else {
                    shutdown_app(&window.app_handle(), "dev_window_close");
                }
            }
        })
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            {
                let activation_policy = if lifecycle_policy.use_accessory_activation {
                    ActivationPolicy::Accessory
                } else {
                    ActivationPolicy::Regular
                };
                let _ = app.set_activation_policy(activation_policy);
            }

            let app_handle = app.handle().clone();
            engine.initialize(&app_handle)?;
            if lifecycle_policy.tray_enabled {
                setup_tray(&app_handle, Arc::clone(&engine))?;
                update_tray_labels(&app_handle, &engine.get_engine_state());
            }
            if lifecycle_policy.start_dev_watchdog {
                start_dev_watchdog(&app_handle);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_engine_state,
            commands::get_settings,
            commands::update_settings,
            commands::set_replay_enabled,
            commands::resume_capture,
            commands::trigger_save_replay,
            commands::list_recent_clips,
            commands::recheck_permissions,
            commands::request_microphone_permission,
            commands::list_microphones,
            commands::grant_output_dir_access,
            commands::grant_screen_recording_access,
            commands::grant_microphone_access
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_tray(app: &AppHandle, engine: Arc<Engine>) -> Result<(), String> {
    let me = |err| format!("failed to build tray menu item: {err}");
    let ms = |err| format!("failed to build separator: {err}");

    let current = engine.get_settings();

    let primary_action =
        MenuItem::with_id(app, "primary_action", "Save Replay", true, None::<&str>).map_err(me)?;
    let status_line =
        MenuItem::with_id(app, "status_line", "Starting...", false, None::<&str>).map_err(me)?;

    let res_options: Vec<(u16, &str)> =
        vec![(360, "360p"), (480, "480p"), (720, "720p"), (1080, "1080p")];
    let mut res_items: Vec<(u16, CheckMenuItem<tauri::Wry>)> = Vec::new();
    let mut res_menu_items: Vec<CheckMenuItem<tauri::Wry>> = Vec::new();
    for (height, label) in &res_options {
        let id = format!("res_{height}");
        let checked = current.video_resolution == *height;
        let item =
            CheckMenuItem::with_id(app, &id, *label, true, checked, None::<&str>).map_err(me)?;
        res_items.push((*height, item.clone()));
        res_menu_items.push(item);
    }
    let res_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = res_menu_items
        .iter()
        .map(|i| i as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
        .collect();
    let res_submenu = Submenu::with_items(app, "Resolution", true, &res_refs)
        .map_err(|err| format!("failed to build submenu: {err}"))?;

    let dur_options: Vec<(u16, &str)> = vec![
        (15, "15 seconds"),
        (30, "30 seconds"),
        (60, "60 seconds"),
        (90, "90 seconds"),
        (120, "2 minutes"),
    ];
    let mut dur_items: Vec<(u16, CheckMenuItem<tauri::Wry>)> = Vec::new();
    let mut dur_menu_items: Vec<CheckMenuItem<tauri::Wry>> = Vec::new();
    for (secs, label) in &dur_options {
        let id = format!("dur_{secs}");
        let checked = current.replay_duration_secs == *secs;
        let item =
            CheckMenuItem::with_id(app, &id, *label, true, checked, None::<&str>).map_err(me)?;
        dur_items.push((*secs, item.clone()));
        dur_menu_items.push(item);
    }
    let dur_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = dur_menu_items
        .iter()
        .map(|i| i as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
        .collect();
    let dur_submenu = Submenu::with_items(app, "Replay Duration", true, &dur_refs)
        .map_err(|err| format!("failed to build submenu: {err}"))?;

    let is_mic_mode = current.audio_mode == "system_plus_mic" && current.mic_enabled;
    let audio_system = CheckMenuItem::with_id(
        app,
        "audio_system",
        "System Audio Only",
        true,
        !is_mic_mode,
        None::<&str>,
    )
    .map_err(me)?;
    let audio_system_mic = CheckMenuItem::with_id(
        app,
        "audio_system_mic",
        "System Audio + Mic",
        true,
        is_mic_mode,
        None::<&str>,
    )
    .map_err(me)?;
    let audio_submenu = Submenu::with_items(
        app,
        "Audio",
        true,
        &[
            &audio_system as &dyn tauri::menu::IsMenuItem<tauri::Wry>,
            &audio_system_mic,
        ],
    )
    .map_err(|err| format!("failed to build submenu: {err}"))?;

    let settings_item =
        MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>).map_err(me)?;
    let quit_item =
        MenuItem::with_id(app, "quit", "Quit Rewinder", true, None::<&str>).map_err(me)?;

    let sep1 = PredefinedMenuItem::separator(app).map_err(ms)?;
    let sep2 = PredefinedMenuItem::separator(app).map_err(ms)?;
    let sep3 = PredefinedMenuItem::separator(app).map_err(ms)?;

    let menu = Menu::with_items(
        app,
        &[
            &primary_action as &dyn tauri::menu::IsMenuItem<tauri::Wry>,
            &sep1,
            &status_line,
            &sep2,
            &res_submenu,
            &dur_submenu,
            &audio_submenu,
            &sep3,
            &settings_item,
            &quit_item,
        ],
    )
    .map_err(|err| format!("failed to build tray menu: {err}"))?;

    {
        let app_state: tauri::State<'_, AppState> = app.state();
        *app_state.tray.lock() = Some(TrayHandles {
            primary_action: primary_action.clone(),
            status_line: status_line.clone(),
            res_items,
            dur_items,
            audio_system: audio_system.clone(),
            audio_system_mic: audio_system_mic.clone(),
        });
    }

    let mut builder = TrayIconBuilder::with_id("rewinder-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event({
            let engine = Arc::clone(&engine);
            move |app, event| {
                let id = event.id.as_ref();
                match id {
                    "primary_action" => handle_primary_action(app, &engine),
                    "settings" => focus_or_show_main_window(app),
                    "quit" => shutdown_app(app, "tray_quit"),
                    _ if id.starts_with("res_") => {
                        if let Some(height) =
                            id.strip_prefix("res_").and_then(|v| v.parse::<u16>().ok())
                        {
                            let _ = engine.apply_runtime_patch(
                                app,
                                SettingsPatchDto {
                                    video_resolution: Some(height),
                                    ..Default::default()
                                },
                                "tray",
                            );
                        }
                    }
                    _ if id.starts_with("dur_") => {
                        if let Some(secs) =
                            id.strip_prefix("dur_").and_then(|v| v.parse::<u16>().ok())
                        {
                            let _ = engine.apply_runtime_patch(
                                app,
                                SettingsPatchDto {
                                    replay_duration_secs: Some(secs),
                                    ..Default::default()
                                },
                                "tray",
                            );
                        }
                    }
                    "audio_system" => {
                        let _ = engine.apply_runtime_patch(
                            app,
                            SettingsPatchDto {
                                audio_mode: Some("system_only".to_string()),
                                mic_enabled: Some(false),
                                ..Default::default()
                            },
                            "tray",
                        );
                    }
                    "audio_system_mic" => {
                        let _ = engine.apply_runtime_patch(
                            app,
                            SettingsPatchDto {
                                audio_mode: Some("system_plus_mic".to_string()),
                                mic_enabled: Some(true),
                                ..Default::default()
                            },
                            "tray",
                        );
                    }
                    _ => {}
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        focus_or_show_main_window(app);
                    }
                }
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    let _tray = builder
        .build(app)
        .map_err(|err| format!("failed to create tray: {err}"))?;

    Ok(())
}

fn handle_primary_action(app: &AppHandle, engine: &Arc<Engine>) {
    let snapshot = engine.get_engine_state();
    match snapshot.lifecycle_state {
        LifecycleState::Armed | LifecycleState::SavingReplay => {
            let _ = engine.trigger_save_replay(app, TriggerSourceDto::Manual);
        }
        LifecycleState::Disabled => {
            if snapshot.arm_blocker_code.as_deref() == Some("user_stopped_sharing") {
                let _ = engine.set_replay_enabled(app, true);
            } else if snapshot.arm_blocker_code.as_deref() == Some("capture_paused") {
                let _ = engine.resume_capture(app);
            } else {
                let _ = engine.set_replay_enabled(app, true);
            }
        }
        LifecycleState::PermissionRequired => {
            let _ = engine.grant_screen_recording_access(app);
        }
        LifecycleState::Booting => {}
    }
}

pub fn update_tray_labels(app: &AppHandle, snapshot: &EngineStateDto) {
    let app_state: tauri::State<'_, AppState> = app.state();
    let tray_guard = app_state.tray.lock();
    let Some(handles) = tray_guard.as_ref() else {
        return;
    };

    let (primary_label, primary_enabled) = match snapshot.lifecycle_state {
        LifecycleState::Armed | LifecycleState::SavingReplay => ("Save Replay", true),
        LifecycleState::Disabled => {
            if snapshot.arm_blocker_code.as_deref() == Some("user_stopped_sharing") {
                ("Restart Capture", true)
            } else if snapshot.arm_blocker_code.as_deref() == Some("capture_paused") {
                ("Resume Capture", true)
            } else {
                ("Restart Capture", true)
            }
        }
        LifecycleState::PermissionRequired => ("Grant Permission", true),
        LifecycleState::Booting => ("Starting...", false),
    };

    let status_text = match snapshot.lifecycle_state {
        LifecycleState::Armed | LifecycleState::SavingReplay => {
            format!(
                "Recording \u{00B7} {}s \u{00B7} {}p {}fps",
                snapshot.settings.replay_duration_secs,
                snapshot.settings.video_resolution,
                snapshot.settings.fps,
            )
        }
        LifecycleState::Disabled => "Paused".to_string(),
        LifecycleState::PermissionRequired => "Permission needed".to_string(),
        LifecycleState::Booting => "Starting...".to_string(),
    };

    let _ = handles.primary_action.set_text(primary_label);
    let _ = handles.primary_action.set_enabled(primary_enabled);
    let _ = handles.status_line.set_text(&status_text);

    for (height, item) in &handles.res_items {
        let _ = item.set_checked(*height == snapshot.settings.video_resolution);
    }
    for (secs, item) in &handles.dur_items {
        let _ = item.set_checked(*secs == snapshot.settings.replay_duration_secs);
    }
    let is_mic_mode =
        snapshot.settings.audio_mode == "system_plus_mic" && snapshot.settings.mic_enabled;
    let _ = handles.audio_system.set_checked(!is_mic_mode);
    let _ = handles.audio_system_mic.set_checked(is_mic_mode);
}

fn focus_or_show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_runtime_policy_uses_foreground_lifecycle() {
        let policy = runtime_lifecycle_policy(true);
        assert!(!policy.hide_on_close);
        assert!(policy.tray_enabled);
        assert!(!policy.use_accessory_activation);
        assert!(policy.start_dev_watchdog);
    }

    #[test]
    fn packaged_runtime_policy_keeps_background_tray() {
        let policy = runtime_lifecycle_policy(false);
        assert!(policy.hide_on_close);
        assert!(policy.tray_enabled);
        assert!(policy.use_accessory_activation);
        assert!(!policy.start_dev_watchdog);
    }

    #[test]
    fn watchdog_failure_count_resets_after_success() {
        assert_eq!(next_watchdog_failure_count(0, false), 1);
        assert_eq!(next_watchdog_failure_count(2, true), 0);
    }

    #[test]
    fn watchdog_prioritizes_parent_loss_when_both_fail() {
        let reason = classify_dev_watchdog_shutdown_reason(
            DEV_WATCHDOG_FAILURE_THRESHOLD,
            DEV_WATCHDOG_FAILURE_THRESHOLD,
        );
        assert_eq!(reason, Some("parent_host_lost"));
    }

    #[test]
    fn watchdog_detects_dev_server_loss_at_threshold() {
        let reason = classify_dev_watchdog_shutdown_reason(0, DEV_WATCHDOG_FAILURE_THRESHOLD);
        assert_eq!(reason, Some("dev_server_lost"));
    }

    #[test]
    fn parse_single_pid_reads_first_non_empty_line() {
        assert_eq!(parse_single_pid("  \n1234\n"), Some(1234));
        assert_eq!(parse_single_pid("not-a-pid\n"), None);
    }
}
