use std::path::Path;

pub fn resolve_ffmpeg_binary() -> String {
    if let Some(found) = env_override("REWINDER_FFMPEG_BIN") {
        return found;
    }
    resolve_bundled_media_binary("ffmpeg")
}

pub fn resolve_ffprobe_binary(ffmpeg_bin: &str) -> String {
    if let Some(found) = env_override("REWINDER_FFPROBE_BIN") {
        return found;
    }
    if let Some(sibling) = sibling_binary(ffmpeg_bin, "ffprobe") {
        return sibling;
    }
    resolve_bundled_media_binary("ffprobe")
}

pub fn resolve_sck_helper_binary(not_built_error: &str) -> Result<String, String> {
    if let Ok(bin) = std::env::var("REWINDER_SCK_HELPER_BIN") {
        if !bin.trim().is_empty() {
            if Path::new(&bin).exists() {
                return Ok(bin);
            }
            return Err(format!("REWINDER_SCK_HELPER_BIN is set but missing: {}", bin));
        }
    }

    if let Some(compiled) = option_env!("REWINDER_SCK_HELPER_PATH") {
        if !compiled.trim().is_empty() && Path::new(compiled).exists() {
            return Ok(compiled.to_string());
        }
    }

    Err(not_built_error.to_string())
}

fn env_override(var: &str) -> Option<String> {
    match std::env::var(var) {
        Ok(bin) if !bin.trim().is_empty() => Some(bin),
        _ => None,
    }
}

fn resolve_bundled_media_binary(name: &str) -> String {
    if let Some(bundled) = bundled_resource_binary(name) {
        return bundled;
    }
    if let Some(dev) = dev_checkout_binary(name) {
        return dev;
    }
    let homebrew = format!("/opt/homebrew/bin/{name}");
    if Path::new(&homebrew).exists() {
        return homebrew;
    }
    name.to_string()
}

fn bundled_resource_binary(name: &str) -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let contents_dir = exe.parent()?.parent()?;
    let bundled = contents_dir.join("Resources").join("bin").join(name);
    bundled
        .exists()
        .then(|| bundled.to_string_lossy().to_string())
}

fn dev_checkout_binary(name: &str) -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    let dev = cwd.join("src-tauri").join("bin").join(name);
    dev.exists().then(|| dev.to_string_lossy().to_string())
}

fn sibling_binary(bin_path: &str, sibling_name: &str) -> Option<String> {
    let parent = Path::new(bin_path).parent()?;
    if parent.as_os_str().is_empty() {
        return None;
    }
    let sibling = parent.join(sibling_name);
    sibling
        .exists()
        .then(|| sibling.to_string_lossy().to_string())
}
