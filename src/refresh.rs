//! Opt-in refresh for VS Code extension users: one tiny hidden `claude -p` request whose
//! stream-json output carries a `rate_limit_event`. Only that event and the token totals are read.
//! `unifiedWindows` is not in the published Agent SDK types (undocumented), so parsing is defensive.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::snapshot::{Snapshot, Window};

pub const MIN_INTERVAL_SECS: i64 = 5 * 60;
pub const MAX_BACKOFF_SECS: i64 = 60 * 60;
/// Activity this long after a reading means Claude was used somewhere the hook did not see.
pub const UNSEEN_GRACE_SECS: i64 = 90;
const TIMEOUT: Duration = Duration::from_secs(60);
pub const ENV_GUARD: &str = "TOKENSTAT_REFRESH";

#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub snapshot: Snapshot,
    pub tokens: u64,
}

pub fn unseen_activity(activity_at: Option<i64>, snap: Option<&Snapshot>) -> bool {
    match (activity_at, snap) {
        (Some(a), Some(s)) => a > s.captured_at + UNSEEN_GRACE_SECS,
        (Some(_), None) => true,
        _ => false,
    }
}

/// Seconds to wait after `failures` consecutive failed refreshes.
pub fn backoff(failures: u32) -> i64 {
    (MIN_INTERVAL_SECS << failures.min(4)).min(MAX_BACKOFF_SECS)
}

fn window(v: &serde_json::Value) -> Option<Window> {
    let u = v.get("utilization")?.as_f64()?;
    let pct = if u <= 1.0 { u * 100.0 } else { u };
    Some(Window { used_percentage: pct.clamp(0.0, 100.0), resets_at: v.get("resetsAt")?.as_f64()? as i64 })
}

pub fn parse(stream: &str, now: i64) -> Option<Outcome> {
    let mut snap = None;
    let mut tokens = 0;
    for line in stream.lines() {
        if !line.contains("\"rate_limit_event\"") && !line.contains("\"type\":\"result\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("rate_limit_event") => {
                let w = v.get("rate_limit_info").and_then(|i| i.get("unifiedWindows"));
                let s = Snapshot {
                    captured_at: now,
                    five_hour: w.and_then(|w| w.get("five_hour")).and_then(window),
                    seven_day: w.and_then(|w| w.get("seven_day")).and_then(window),
                };
                if s.has_limits() {
                    snap = Some(s);
                }
            }
            Some("result") => {
                if let Some(u) = v.get("usage") {
                    tokens = ["input_tokens", "output_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]
                        .iter()
                        .filter_map(|k| u.get(*k).and_then(|x| x.as_u64()))
                        .sum();
                }
            }
            _ => {}
        }
    }
    snap.map(|snapshot| Outcome { snapshot, tokens })
}

fn command(program: &str, workdir: &Path) -> Command {
    let mut c = Command::new(program);
    if program == "cmd" {
        c.args(["/C", "claude"]);
    }
    c.args([
        "-p",
        "Reply with: ok",
        "--safe-mode",
        "--tools",
        "",
        "--no-session-persistence",
        "--model",
        "haiku",
        "--output-format",
        "stream-json",
        "--verbose",
    ])
    .current_dir(workdir)
    .env(ENV_GUARD, "1")
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c
}

pub fn run(workdir: &Path, now: i64) -> Result<Outcome, String> {
    std::fs::create_dir_all(workdir).map_err(|e| e.to_string())?;
    let mut child = match command("claude", workdir).spawn() {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => command("cmd", workdir).spawn().map_err(|e| e.to_string())?,
        Err(e) => return Err(e.to_string()),
    };
    let mut stdout = child.stdout.take().ok_or("no output")?;
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stdout.read_to_string(&mut out);
        out
    });
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < TIMEOUT => std::thread::sleep(Duration::from_millis(100)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
        }
    }
    let out = reader.join().map_err(|_| "reader failed")?;
    parse(&out, now).ok_or_else(|| "no usage figures in Claude Code output".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const STREAM: &str = r#"{"type":"system","subtype":"init","session_id":"x","tools":[]}
{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","resetsAt":1794885000,"rateLimitType":"five_hour","unifiedWindows":{"five_hour":{"utilization":0.28,"resetsAt":1794885000,"status":"allowed"},"seven_day":{"utilization":0.2,"resetsAt":1795028400,"status":"allowed"}}},"session_id":"x"}
{"type":"result","result":"ok","usage":{"input_tokens":2,"cache_creation_input_tokens":31771,"cache_read_input_tokens":17477,"output_tokens":4}}"#;

    #[test]
    fn parses_spike_output() {
        let o = parse(STREAM, 100).unwrap();
        assert_eq!(o.snapshot.captured_at, 100);
        assert!((o.snapshot.five_hour.unwrap().used_percentage - 28.0).abs() < 1e-9);
        assert_eq!(o.snapshot.seven_day.unwrap().resets_at, 1795028400);
        assert_eq!(o.tokens, 49254);
    }

    #[test]
    fn missing_unified_windows_gives_none() {
        let s = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","resetsAt":1}}"#;
        assert!(parse(s, 1).is_none());
        assert!(parse("garbage\n{}", 1).is_none());
    }

    #[test]
    fn unseen_activity_rules() {
        let s = Snapshot { captured_at: 1000, five_hour: None, seven_day: None };
        assert!(!unseen_activity(None, Some(&s)));
        assert!(!unseen_activity(Some(1000 + UNSEEN_GRACE_SECS), Some(&s)));
        assert!(unseen_activity(Some(1001 + UNSEEN_GRACE_SECS), Some(&s)));
        assert!(unseen_activity(Some(5), None));
    }

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(backoff(0), MIN_INTERVAL_SECS);
        assert_eq!(backoff(1), 2 * MIN_INTERVAL_SECS);
        assert_eq!(backoff(10), MAX_BACKOFF_SECS);
    }
}
