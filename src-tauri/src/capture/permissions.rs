use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::state::{MicrophoneDeviceDto, PermissionStateDto};
use crate::sys::binaries;
use serde::Deserialize;

const SCK_HELPER_MIC_PROBE_MISSING: &str =
    "ScreenCaptureKit helper binary is unavailable for microphone probe.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicrophonePermissionStatus {
    Granted,
    Denied,
    Restricted,
    NotDetermined,
    Unknown,
}

impl MicrophonePermissionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::Denied => "denied",
            Self::Restricted => "restricted",
            Self::NotDetermined => "not_determined",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MicrophonePermissionProbe {
    pub status: MicrophonePermissionStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OutputDirAccessProbe {
    pub writable: bool,
    pub error: Option<String>,
}

impl MicrophonePermissionProbe {
    fn granted() -> Self {
        Self {
            status: MicrophonePermissionStatus::Granted,
            error: None,
        }
    }

    fn with_error(status: MicrophonePermissionStatus, error: impl Into<String>) -> Self {
        Self {
            status,
            error: Some(error.into()),
        }
    }
}

pub fn detect_permissions_for_output_dir(output_dir: &Path) -> PermissionStateDto {
    let output_probe = probe_output_dir_access(output_dir);
    #[cfg(target_os = "macos")]
    {
        if !screen_capture_preflight() {
            let _ = request_screen_capture_access();
            if !screen_capture_preflight() {
                return PermissionStateDto {
                    screen_recording_granted: false,
                    system_audio_granted: false,
                    output_dir_writable: output_probe.writable,
                    output_dir_permission_error: output_probe.error.clone(),
                    reason: Some(
                        "Screen Recording permission is not granted. Enable Rewinder in System Settings > Privacy & Security > Screen Recording, then restart Rewinder."
                            .to_string(),
                    ),
                };
            }
        }
    }

    let ffmpeg_bin = std::env::var("REWINDER_FFMPEG_BIN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            if std::path::Path::new("/opt/homebrew/bin/ffmpeg").exists() {
                "/opt/homebrew/bin/ffmpeg".to_string()
            } else {
                "ffmpeg".to_string()
            }
        });

    let output = Command::new(ffmpeg_bin)
        .args([
            "-hide_banner",
            "-f",
            "avfoundation",
            "-list_devices",
            "true",
            "-i",
            "",
        ])
        .output();

    match output {
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let has_screen_device = has_screen_capture_device(&stderr);

            if has_screen_device {
                if !output_probe.writable {
                    return PermissionStateDto {
                        screen_recording_granted: true,
                        system_audio_granted: true,
                        output_dir_writable: false,
                        output_dir_permission_error: output_probe.error.clone(),
                        reason: Some(
                            output_probe.error.unwrap_or_else(|| {
                                output_dir_permission_denied_message(output_dir)
                            }),
                        ),
                    };
                }
                PermissionStateDto {
                    screen_recording_granted: true,
                    system_audio_granted: true,
                    output_dir_writable: true,
                    output_dir_permission_error: None,
                    reason: None,
                }
            } else {
                PermissionStateDto {
                    screen_recording_granted: false,
                    system_audio_granted: false,
                    output_dir_writable: output_probe.writable,
                    output_dir_permission_error: output_probe.error.clone(),
                    reason: Some(
                        "No AVFoundation screen-capture device available. Ensure Screen Recording permission is granted and at least one display source is active."
                            .to_string(),
                    ),
                }
            }
        }
        Err(err) => PermissionStateDto {
            screen_recording_granted: false,
            system_audio_granted: false,
            output_dir_writable: output_probe.writable,
            output_dir_permission_error: output_probe.error.clone(),
            reason: Some(format!("Failed to run ffmpeg for permission probe: {err}")),
        },
    }
}

pub fn probe_output_dir_access(output_dir: &Path) -> OutputDirAccessProbe {
    if let Err(err) = std::fs::create_dir_all(output_dir) {
        if is_permission_io_error(&err) {
            return OutputDirAccessProbe {
                writable: false,
                error: Some(output_dir_permission_denied_message(output_dir)),
            };
        }
        return OutputDirAccessProbe {
            writable: false,
            error: Some(format!(
                "Failed to prepare output directory {}: {err}",
                output_dir.display()
            )),
        };
    }

    if let Err(err) = std::fs::read_dir(output_dir) {
        if is_permission_io_error(&err) {
            return OutputDirAccessProbe {
                writable: false,
                error: Some(output_dir_permission_denied_message(output_dir)),
            };
        }
        return OutputDirAccessProbe {
            writable: false,
            error: Some(format!(
                "Failed to access output directory {}: {err}",
                output_dir.display()
            )),
        };
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let probe_path = output_dir.join(format!(".rewinder-permission-probe-{nonce}.tmp"));
    match std::fs::File::create(&probe_path) {
        Ok(mut file) => {
            let write_result = file.write_all(b"rewinder-probe");
            let _ = std::fs::remove_file(&probe_path);
            if let Err(err) = write_result {
                if is_permission_io_error(&err) {
                    return OutputDirAccessProbe {
                        writable: false,
                        error: Some(output_dir_permission_denied_message(output_dir)),
                    };
                }
                return OutputDirAccessProbe {
                    writable: false,
                    error: Some(format!(
                        "Failed to write output directory {}: {err}",
                        output_dir.display()
                    )),
                };
            }
            OutputDirAccessProbe {
                writable: true,
                error: None,
            }
        }
        Err(err) => {
            if is_permission_io_error(&err) {
                return OutputDirAccessProbe {
                    writable: false,
                    error: Some(output_dir_permission_denied_message(output_dir)),
                };
            }
            OutputDirAccessProbe {
                writable: false,
                error: Some(format!(
                    "Failed to open output directory {} for writing: {err}",
                    output_dir.display()
                )),
            }
        }
    }
}

pub fn ensure_output_dir_access(output_dir: &Path) -> Result<(), String> {
    let probe = probe_output_dir_access(output_dir);
    if probe.writable {
        Ok(())
    } else {
        Err(probe
            .error
            .unwrap_or_else(|| output_dir_permission_denied_message(output_dir)))
    }
}

pub fn open_downloads_permission_settings() -> bool {
    #[cfg(target_os = "macos")]
    {
        let primary =
            "x-apple.systempreferences:com.apple.preference.security?Privacy_FilesAndFolders";
        if Command::new("open")
            .arg(primary)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
        {
            return true;
        }

        let fallback = "x-apple.systempreferences:com.apple.preference.security";
        return Command::new("open")
            .arg(fallback)
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn open_microphone_permission_settings() -> bool {
    #[cfg(target_os = "macos")]
    {
        let primary = "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";
        if Command::new("open")
            .arg(primary)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
        {
            return true;
        }

        let fallback = "x-apple.systempreferences:com.apple.preference.security";
        return Command::new("open")
            .arg(fallback)
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn open_screen_recording_permission_settings() -> bool {
    #[cfg(target_os = "macos")]
    {
        let primary =
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";
        if Command::new("open")
            .arg(primary)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
        {
            return true;
        }

        let fallback = "x-apple.systempreferences:com.apple.preference.security";
        return Command::new("open")
            .arg(fallback)
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn probe_screen_recording_permission(request_if_needed: bool) -> bool {
    #[cfg(target_os = "macos")]
    {
        if screen_capture_preflight() {
            return true;
        }
        if request_if_needed {
            let _ = request_screen_capture_access();
        }
        screen_capture_preflight()
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = request_if_needed;
        true
    }
}

pub fn probe_microphone_permission(request_if_needed: bool) -> MicrophonePermissionProbe {
    #[cfg(target_os = "macos")]
    {
        let helper_bin = match binaries::resolve_sck_helper_binary(SCK_HELPER_MIC_PROBE_MISSING) {
            Ok(path) => path,
            Err(err) => {
                return MicrophonePermissionProbe::with_error(
                    MicrophonePermissionStatus::Unknown,
                    err,
                );
            }
        };
        let mode_flag = if request_if_needed {
            "--request-mic-permission"
        } else {
            "--probe-mic-permission"
        };

        let output = match Command::new(helper_bin).arg(mode_flag).output() {
            Ok(output) => output,
            Err(err) => {
                return MicrophonePermissionProbe::with_error(
                    MicrophonePermissionStatus::Unknown,
                    format!("failed to run microphone permission probe: {err}"),
                );
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}").to_ascii_lowercase();

        if output.status.success() && combined.contains("mic_permission=granted") {
            return MicrophonePermissionProbe::granted();
        }
        if combined.contains("mic_permission=denied") {
            return MicrophonePermissionProbe::with_error(
                MicrophonePermissionStatus::Denied,
                "Microphone permission denied. Enable Rewinder in System Settings > Privacy & Security > Microphone.",
            );
        }
        if combined.contains("mic_permission=restricted") {
            return MicrophonePermissionProbe::with_error(
                MicrophonePermissionStatus::Restricted,
                "Microphone permission is restricted by system policy.",
            );
        }
        if combined.contains("mic_permission=not_determined") {
            return MicrophonePermissionProbe::with_error(
                MicrophonePermissionStatus::NotDetermined,
                "Microphone permission has not been granted yet. Allow access when prompted.",
            );
        }

        return MicrophonePermissionProbe::with_error(
            MicrophonePermissionStatus::Unknown,
            format!(
                "Microphone permission probe failed (status {}): {}",
                output.status,
                stderr.trim()
            ),
        );
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = request_if_needed;
        MicrophonePermissionProbe::granted()
    }
}

pub fn ensure_microphone_permission(request: bool) -> Result<(), String> {
    let probe = probe_microphone_permission(request);
    if probe.status == MicrophonePermissionStatus::Granted {
        return Ok(());
    }
    Err(probe
        .error
        .unwrap_or_else(|| "Microphone permission probe failed.".to_string()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HelperMicrophoneDeviceDto {
    id: String,
    name: String,
    is_default: bool,
    is_available: bool,
}

pub fn list_microphones() -> Result<Vec<MicrophoneDeviceDto>, String> {
    #[cfg(target_os = "macos")]
    {
        let helper_bin = binaries::resolve_sck_helper_binary(SCK_HELPER_MIC_PROBE_MISSING)?;
        let output = Command::new(helper_bin)
            .arg("--list-microphones")
            .output()
            .map_err(|err| format!("failed to launch microphone list helper: {err}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "microphone list helper failed (status {}): {}",
                output.status,
                stderr.trim()
            ));
        }

        let devices: Vec<HelperMicrophoneDeviceDto> = serde_json::from_slice(&output.stdout)
            .map_err(|err| format!("failed to parse microphone list: {err}"))?;
        return Ok(devices
            .into_iter()
            .map(|device| MicrophoneDeviceDto {
                id: device.id,
                name: device.name,
                is_default: device.is_default,
                is_available: device.is_available,
            })
            .collect());
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(Vec::new())
    }
}

#[cfg(target_os = "macos")]
fn screen_capture_preflight() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() }
}

#[cfg(target_os = "macos")]
fn request_screen_capture_access() -> bool {
    unsafe { CGRequestScreenCaptureAccess() }
}

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

fn has_screen_capture_device(stderr: &str) -> bool {
    let mut in_video_devices = false;
    let screen_keywords = ["capture screen", "screen", "display", "monitor"];
    let camera_keywords = ["camera", "facetime", "continuity", "webcam", "iphone"];

    for line in stderr.lines() {
        if line.contains("AVFoundation video devices") {
            in_video_devices = true;
            continue;
        }
        if line.contains("AVFoundation audio devices") {
            in_video_devices = false;
            continue;
        }
        if !in_video_devices {
            continue;
        }

        let Some((_, device_name)) = parse_avfoundation_device_line(line) else {
            continue;
        };

        let lower = device_name.to_ascii_lowercase();
        let has_screen_keyword = screen_keywords
            .iter()
            .any(|keyword| lower.contains(keyword));
        let has_camera_keyword = camera_keywords
            .iter()
            .any(|keyword| lower.contains(keyword));
        if has_screen_keyword && !has_camera_keyword {
            return true;
        }
    }

    false
}

fn parse_avfoundation_device_line(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim();

    let candidate = if let Some(marker) = trimmed.rfind("] [") {
        &trimmed[marker + 2..]
    } else if trimmed.starts_with('[') {
        trimmed
    } else {
        return None;
    };

    if !candidate.starts_with('[') {
        return None;
    }

    let end = candidate.find(']')?;
    let index = candidate[1..end].parse::<usize>().ok()?;
    let name = candidate[end + 1..].trim().to_string();
    if name.is_empty() {
        return None;
    }

    Some((index, name))
}

fn output_dir_permission_denied_message(output_dir: &Path) -> String {
    format!(
        "Downloads folder access is denied for Rewinder ({}). Enable Rewinder in System Settings > Privacy & Security > Files and Folders (Downloads).",
        output_dir.display()
    )
}

fn is_permission_io_error(err: &std::io::Error) -> bool {
    matches!(err.kind(), std::io::ErrorKind::PermissionDenied)
}

#[cfg(test)]
mod tests {
    use super::{has_screen_capture_device, parse_avfoundation_device_line};

    #[test]
    fn parse_ignores_objc_warning() {
        let warning =
            "objc[63059]: class `NSKVONotifying_AVCaptureScreenInput' not linked into application";
        assert!(parse_avfoundation_device_line(warning).is_none());
    }

    #[test]
    fn detects_screen_device_only_from_device_lines() {
        let listing = r#"
[AVFoundation indev @ 0x111] AVFoundation video devices:
objc[63059]: class `NSKVONotifying_AVCaptureScreenInput' not linked into application
[AVFoundation indev @ 0x111] [0] FaceTime HD Camera
[AVFoundation indev @ 0x111] [1] Capture screen 0
[AVFoundation indev @ 0x111] AVFoundation audio devices:
"#;
        assert!(has_screen_capture_device(listing));
    }
}
