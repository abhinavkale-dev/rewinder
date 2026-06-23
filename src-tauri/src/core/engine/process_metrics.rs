use super::*;
pub(super) fn sample_process_ps_metrics(pid: u32) -> (Option<u32>, Option<f32>) {
    let output = Command::new("ps")
        .arg("-o")
        .arg("rss=,%cpu=")
        .arg("-p")
        .arg(pid.to_string())
        .output();
    let Ok(output) = output else {
        return (None, None);
    };
    if !output.status.success() {
        return (None, None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .map(str::trim)
        .find(|value| !value.is_empty());
    let Some(line) = line else {
        return (None, None);
    };
    let mut parts = line.split_whitespace();
    let rss_mb = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .map(|kb| kb / 1024);
    let cpu_percent = parts.next().and_then(|value| value.parse::<f32>().ok());
    (rss_mb, cpu_percent)
}

pub(super) fn sample_process_ps_metrics_for_pids(pids: &[u32]) -> (Option<u32>, Option<f32>) {
    if pids.is_empty() {
        return (None, None);
    }

    let pid_list = pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let output = Command::new("ps")
        .arg("-o")
        .arg("rss=,%cpu=")
        .arg("-p")
        .arg(pid_list)
        .output();
    let Ok(output) = output else {
        return (None, None);
    };
    if !output.status.success() {
        return (None, None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_ps_rss_cpu_totals(&stdout)
}

pub(super) fn parse_ps_rss_cpu_totals(stdout: &str) -> (Option<u32>, Option<f32>) {
    let mut total_rss_kb: u64 = 0;
    let mut total_cpu: f32 = 0.0;
    let mut rss_seen = false;
    let mut cpu_seen = false;

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        if let Some(rss_part) = parts.next() {
            if let Ok(rss_kb) = rss_part.parse::<u64>() {
                total_rss_kb = total_rss_kb.saturating_add(rss_kb);
                rss_seen = true;
            }
        }
        if let Some(cpu_part) = parts.next() {
            if let Ok(cpu_pct) = cpu_part.parse::<f32>() {
                total_cpu += cpu_pct;
                cpu_seen = true;
            }
        }
    }

    (
        if rss_seen {
            Some((total_rss_kb / 1024).min(u64::from(u32::MAX)) as u32)
        } else {
            None
        },
        if cpu_seen { Some(total_cpu) } else { None },
    )
}

pub(super) fn capture_stack_rss_delta_soft_budget_mb(
    effective_video_resolution: u16,
    effective_fps: u16,
) -> u32 {
    if effective_video_resolution >= 1080 && effective_fps >= 60 {
        300
    } else if effective_video_resolution >= 1080 && effective_fps >= 30 {
        200
    } else if effective_fps >= 60 {
        240
    } else {
        180
    }
}

pub(super) fn capture_stack_rss_delta_hard_budget_mb(
    effective_video_resolution: u16,
    effective_fps: u16,
) -> u32 {
    let soft = capture_stack_rss_delta_soft_budget_mb(effective_video_resolution, effective_fps);
    soft.saturating_add(if effective_fps >= 60 { 140 } else { 100 })
}

pub(super) fn sample_thermal_state() -> Option<String> {
    #[cfg(not(target_os = "macos"))]
    {
        return None;
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("pmset").arg("-g").arg("therm").output().ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let trimmed = line.trim();
            if !trimmed.to_ascii_lowercase().contains("thermallevel") {
                continue;
            }
            let level = trimmed
                .split('=')
                .nth(1)
                .map(str::trim)
                .and_then(|value| value.parse::<i32>().ok())?;
            let label = match level {
                i32::MIN..=0 => "nominal",
                1 => "fair",
                2 => "serious",
                _ => "critical",
            };
            return Some(label.to_string());
        }
        if stdout
            .to_ascii_lowercase()
            .contains("no thermal warning level")
        {
            return Some("nominal".to_string());
        }
        None
    }
}

pub(super) fn parse_power_source(stdout: &str) -> Option<&'static str> {
    for line in stdout.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("now drawing from") {
            if lower.contains("battery power") {
                return Some("battery");
            }
            if lower.contains("ac power") {
                return Some("ac");
            }
        }
    }
    None
}

pub(super) fn sample_power_source() -> Option<String> {
    #[cfg(not(target_os = "macos"))]
    {
        return None;
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("pmset").arg("-g").arg("batt").output().ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_power_source(&stdout).map(|label| label.to_string())
    }
}
