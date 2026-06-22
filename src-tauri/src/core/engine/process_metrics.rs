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

