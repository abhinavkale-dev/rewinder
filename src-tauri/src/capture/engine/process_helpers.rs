use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct ChildTerminationOutcome {
    pub(super) forced_kill: bool,
    pub(super) status: Option<ExitStatus>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct CaptureProcessSweepOutcome {
    pub(super) candidate_count: usize,
    pub(super) term_attempted: usize,
    pub(super) kill_attempted: usize,
    pub(super) terminated_count: usize,
}

pub(super) fn generate_capture_session_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|delta| delta.as_nanos() as u64)
        .unwrap_or(0);
    let counter = CAPTURE_SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}{:x}", nanos, counter)
}

pub(super) fn replay_continuity_gap_threshold(
    entries: &[SegmentFile],
    anchor_index: usize,
    segment_duration_secs: f32,
) -> Duration {
    let adaptive_window = 12usize;
    let window_start = anchor_index.saturating_sub(adaptive_window);
    let mut deltas: Vec<f32> = Vec::new();
    for idx in (window_start + 1)..=anchor_index {
        let prev = &entries[idx - 1];
        let next = &entries[idx];
        let Ok(delta) = next.modified.duration_since(prev.modified) else {
            continue;
        };
        let delta_secs = delta.as_secs_f32();
        if delta_secs > 0.0 && delta_secs < 8.0 {
            deltas.push(delta_secs);
        }
    }
    let baseline_secs = if deltas.is_empty() {
        segment_duration_secs
    } else {
        median_f32(&mut deltas)
    };
    let threshold_secs = (baseline_secs * 3.5).clamp(1.0, 4.0);
    Duration::from_secs_f32(threshold_secs)
}

pub(super) fn system_time_sub(base: SystemTime, duration: Duration) -> SystemTime {
    base.checked_sub(duration).unwrap_or(SystemTime::UNIX_EPOCH)
}

pub(super) fn median_f32(values: &mut [f32]) -> f32 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

pub(super) fn duration_to_ms_u64(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

pub(super) fn keep_segment_count_for_duration(
    buffer_duration_secs: u16,
    segment_duration_secs: f32,
) -> usize {
    ((f32::from(buffer_duration_secs) / segment_duration_secs).ceil() as usize)
        .max(1)
        .saturating_add(RETENTION_MARGIN_SEGMENTS)
}

pub(super) fn segment_duration_secs(segment_time_ms: u16) -> f32 {
    let millis = segment_time_ms.clamp(250, 2_000);
    f32::from(millis) / 1_000.0
}

pub(super) fn segment_time_delta_for_fps(fps: u16) -> f32 {
    let fps = f32::from(fps.max(1));
    (0.5 / fps).clamp(0.001, 0.040)
}

pub(super) fn exit_status_unknown_failure() -> ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(1)
    }

    #[cfg(not(unix))]
    {
        Command::new("false")
            .status()
            .unwrap_or_else(|_| panic!("failed to build fallback ExitStatus"))
    }
}

pub(super) fn append_capture_log_line(path: &Path, line: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

pub(super) fn capture_lock_started_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|delta| delta.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

pub(super) fn parse_capture_lock_payload(content: &str) -> CaptureLockPayload {
    let mut payload = CaptureLockPayload::default();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("owner_pid=") {
            payload.owner_pid = value.trim().parse::<u32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("started_epoch_ms=") {
            payload.started_epoch_ms = value.trim().parse::<i64>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("session_id=") {
            let value = value.trim();
            if !value.is_empty() {
                payload.session_id = Some(value.to_string());
            }
        }
    }
    payload
}

pub(super) fn read_capture_lock_payload(path: &Path) -> Option<CaptureLockPayload> {
    let raw = fs::read_to_string(path).ok()?;
    let parsed = parse_capture_lock_payload(&raw);
    if parsed.owner_pid.is_none()
        && parsed.started_epoch_ms.is_none()
        && parsed.session_id.is_none()
    {
        return None;
    }
    Some(parsed)
}

pub(super) fn write_capture_lock_payload(
    lock_file: &mut File,
    owner_pid: u32,
    session_id: &str,
    started_epoch_ms: i64,
) -> Result<(), String> {
    lock_file
        .set_len(0)
        .map_err(|err| format!("failed to truncate capture lock: {err}"))?;
    lock_file
        .seek(SeekFrom::Start(0))
        .map_err(|err| format!("failed to seek capture lock: {err}"))?;
    writeln!(lock_file, "owner_pid={owner_pid}")
        .map_err(|err| format!("failed to write capture lock owner pid: {err}"))?;
    writeln!(lock_file, "started_epoch_ms={started_epoch_ms}")
        .map_err(|err| format!("failed to write capture lock start epoch: {err}"))?;
    writeln!(lock_file, "session_id={session_id}")
        .map_err(|err| format!("failed to write capture lock session id: {err}"))?;
    lock_file
        .flush()
        .map_err(|err| format!("failed to flush capture lock payload: {err}"))?;
    Ok(())
}

pub(super) fn acquire_capture_lock(
    lock_path: &Path,
    owner_pid: u32,
    session_id: &str,
) -> Result<(File, Option<CaptureLockPayload>), String> {
    let existing_payload = read_capture_lock_payload(lock_path);
    let mut lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(lock_path)
        .map_err(|err| format!("failed to open capture lock file: {err}"))?;
    if let Err(err) = lock_file.try_lock_exclusive() {
        let owner_pid_detail = existing_payload
            .as_ref()
            .and_then(|payload| payload.owner_pid)
            .map(|pid| format!(" owner_pid={pid}"))
            .unwrap_or_default();
        let owner_session_detail = existing_payload
            .as_ref()
            .and_then(|payload| payload.session_id.clone())
            .map(|session| format!(" session_id={session}"))
            .unwrap_or_default();
        return Err(format!(
            "capture_owner_exists: Another Rewinder instance is already capturing.{}{} lock_path={} lock_err={}",
            owner_pid_detail,
            owner_session_detail,
            lock_path.display(),
            err
        ));
    }
    let stale_owner = existing_payload.and_then(|payload| {
        payload
            .owner_pid
            .filter(|pid| *pid != owner_pid && !process_is_running(*pid))
            .map(|_| payload)
    });
    write_capture_lock_payload(
        &mut lock_file,
        owner_pid,
        session_id,
        capture_lock_started_epoch_ms(),
    )?;
    Ok((lock_file, stale_owner))
}

pub(super) fn process_is_running(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub(super) fn is_rewinder_capture_process_command(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    if lower.contains("rewinder-sck-capture") {
        return true;
    }
    if !lower.contains("ffmpeg") {
        return false;
    }
    if lower.contains(".rewinder-live")
        || lower.contains("video.pipe")
        || lower.contains("system_audio.pipe")
        || lower.contains("mic_audio.pipe")
    {
        return true;
    }
    lower.contains("seg_") && lower.contains(".mp4") && lower.contains("rewinder")
}

pub(super) fn parse_capture_process_candidates(ps_output: &str) -> Vec<u32> {
    let mut candidates = Vec::new();
    for line in ps_output.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.splitn(2, |c: char| c.is_whitespace());
        let Some(pid_part) = parts.next() else {
            continue;
        };
        let Some(command_part) = parts.next() else {
            continue;
        };
        let Ok(pid) = pid_part.trim().parse::<u32>() else {
            continue;
        };
        if is_rewinder_capture_process_command(command_part.trim()) {
            candidates.push(pid);
        }
    }
    candidates
}

pub(super) fn select_capture_process_sweep_candidates(
    ps_output: &str,
    exclude_pids: &HashSet<u32>,
) -> Vec<u32> {
    let self_pid = std::process::id();
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    for pid in parse_capture_process_candidates(ps_output) {
        if pid == self_pid || exclude_pids.contains(&pid) || !seen.insert(pid) {
            continue;
        }
        selected.push(pid);
    }
    selected
}

fn signal_pid(pid: u32, signal: &str) -> bool {
    Command::new("kill")
        .args([signal, &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub(super) fn sweep_orphan_capture_processes(
    capture_log_path: &Path,
    phase: &str,
    exclude_pids: &HashSet<u32>,
) -> CaptureProcessSweepOutcome {
    let output = match Command::new("ps").args(["-axo", "pid=,command="]).output() {
        Ok(value) => value,
        Err(err) => {
            append_capture_log_line(
                capture_log_path,
                &format!("phase: stale_capture_sweep_failed phase={phase} detail={err}"),
            );
            return CaptureProcessSweepOutcome::default();
        }
    };
    if !output.status.success() {
        append_capture_log_line(
            capture_log_path,
            &format!(
                "phase: stale_capture_sweep_failed phase={phase} status={}",
                output.status
            ),
        );
        return CaptureProcessSweepOutcome::default();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let candidate_pids = select_capture_process_sweep_candidates(&stdout, exclude_pids);
    if candidate_pids.is_empty() {
        append_capture_log_line(
            capture_log_path,
            &format!(
                "phase: stale_capture_sweep phase={phase} candidates=0 term_sent=0 kill_sent=0 killed=0"
            ),
        );
        return CaptureProcessSweepOutcome::default();
    }

    let mut outcome = CaptureProcessSweepOutcome {
        candidate_count: candidate_pids.len(),
        ..CaptureProcessSweepOutcome::default()
    };
    for pid in &candidate_pids {
        if signal_pid(*pid, "-TERM") {
            outcome.term_attempted = outcome.term_attempted.saturating_add(1);
        }
    }
    thread::sleep(Duration::from_millis(PROCESS_SWEEP_TERM_GRACE_MS));
    for pid in &candidate_pids {
        if process_is_running(*pid) && signal_pid(*pid, "-KILL") {
            outcome.kill_attempted = outcome.kill_attempted.saturating_add(1);
        }
    }
    for pid in &candidate_pids {
        if !process_is_running(*pid) {
            outcome.terminated_count = outcome.terminated_count.saturating_add(1);
        }
    }
    append_capture_log_line(
        capture_log_path,
        &format!(
            "phase: stale_capture_sweep phase={phase} candidates={} term_sent={} kill_sent={} killed={}",
            outcome.candidate_count,
            outcome.term_attempted,
            outcome.kill_attempted,
            outcome.terminated_count
        ),
    );
    outcome
}

pub(super) fn terminate_stale_capture_process(pid_file: &Path) {
    let Ok(raw) = fs::read_to_string(pid_file) else {
        return;
    };
    let pid = raw.trim();
    if pid.is_empty() {
        let _ = fs::remove_file(pid_file);
        return;
    }

    let Ok(pid_u32) = pid.parse::<u32>() else {
        let _ = fs::remove_file(pid_file);
        return;
    };

    if process_is_running(pid_u32) {
        let _ = Command::new("kill")
            .args(["-TERM", pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        wait_for_pid_exit(pid_u32, Duration::from_millis(PROCESS_TERM_GRACE_MS));
    }

    if process_is_running(pid_u32) {
        let _ = Command::new("kill")
            .args(["-KILL", pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        wait_for_pid_exit(pid_u32, Duration::from_millis(400));
    }

    if !process_is_running(pid_u32) {
        let _ = fs::remove_file(pid_file);
    }
}

pub(super) fn terminate_child_gracefully(child: &mut Child) -> ChildTerminationOutcome {
    let pid = child.id().to_string();

    let _ = Command::new("kill")
        .args(["-TERM", &pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if let Some(status) = wait_for_process_exit(child, Duration::from_millis(PROCESS_TERM_GRACE_MS))
    {
        return ChildTerminationOutcome {
            forced_kill: false,
            status: Some(status),
        };
    }

    let _ = child.kill();
    ChildTerminationOutcome {
        forced_kill: true,
        status: wait_for_process_exit(child, Duration::from_millis(400)),
    }
}

pub(super) fn wait_for_process_exit(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    return None;
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return None,
        }
    }
}

fn wait_for_pid_exit(pid: u32, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if !process_is_running(pid) {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    !process_is_running(pid)
}

pub(super) fn current_display_signature() -> u64 {
    fn fold_time_signature(acc: &mut u64, modified: SystemTime) {
        if let Ok(delta) = modified.duration_since(SystemTime::UNIX_EPOCH) {
            *acc ^= delta.as_secs();
            *acc ^= u64::from(delta.subsec_nanos());
        }
    }

    let mut signature = 0_u64;

    let global = Path::new("/Library/Preferences/com.apple.windowserver.displays.plist");
    if let Ok(meta) = fs::metadata(global) {
        if let Ok(modified) = meta.modified() {
            fold_time_signature(&mut signature, modified);
        }
    }

    if let Some(home) = env::var_os("HOME") {
        let by_host = PathBuf::from(home).join("Library/Preferences/ByHost");
        if let Ok(entries) = fs::read_dir(by_host) {
            for entry in entries.flatten() {
                let path = entry.path();
                let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
                    continue;
                };
                if !name.starts_with("com.apple.windowserver.") || !name.ends_with(".plist") {
                    continue;
                }
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        fold_time_signature(&mut signature, modified);
                    }
                }
            }
        }
    }

    signature
}
