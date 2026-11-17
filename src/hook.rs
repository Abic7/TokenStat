//! `tokenstat --statusline`: Claude Code runs this after each response and pipes its status JSON on stdin.
//! Only rate_limits are kept. Nothing else from stdin is stored or logged.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::snapshot::{self, Snapshot, Window};
use crate::view;

const MAX_STDIN: u64 = 4 * 1024 * 1024;
pub const REWRITE_AFTER_SECS: i64 = 60;
const CHAIN_FILE: &str = "chain.txt";
const CHAIN_TIMEOUT: Duration = Duration::from_secs(5);

pub fn extract(input: &[u8], now: i64) -> Option<Snapshot> {
    let v: serde_json::Value = serde_json::from_slice(input).ok()?;
    let rl = v.get("rate_limits");
    let win = |key: &str| {
        let w = rl?.get(key)?;
        Some(Window {
            used_percentage: w.get("used_percentage")?.as_f64()?,
            resets_at: w.get("resets_at")?.as_f64()? as i64,
        })
    };
    Some(Snapshot { captured_at: now, five_hour: win("five_hour"), seven_day: win("seven_day") })
}

/// Write when values change, or at most once a minute to keep the reading fresh.
/// Never replace a good reading with an empty one.
pub fn should_write(old: Option<&Snapshot>, new: &Snapshot) -> bool {
    match old {
        None => true,
        Some(_) if !new.has_limits() => false,
        Some(o) => {
            o.five_hour != new.five_hour
                || o.seven_day != new.seven_day
                || new.captured_at - o.captured_at >= REWRITE_AFTER_SECS
        }
    }
}

pub fn write_atomic(dir: &Path, snap: &Snapshot) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join("snapshot.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(snap)?)?;
    std::fs::rename(&tmp, dir.join(snapshot::FILE_NAME))
}

fn shell(cmd: &str) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut c = Command::new("cmd");
        c.arg("/C").raw_arg(cmd).creation_flags(CREATE_NO_WINDOW);
        c
    }
    #[cfg(not(windows))]
    {
        let mut c = Command::new("sh");
        c.arg("-c").arg(cmd);
        c
    }
}

/// Run the user's previous status line command with the same stdin and return its output.
pub fn run_chain(cmd: &str, input: &[u8]) -> Option<String> {
    let mut child = shell(cmd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let mut stdin = child.stdin.take()?;
    let data = input.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&data);
    });
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stdout.read_to_string(&mut out);
        out
    });
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < CHAIN_TIMEOUT => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
        }
    }
    let _ = writer.join();
    reader.join().ok()
}

pub const ACTIVITY_FILE: &str = "activity.txt";

/// `tokenstat --activity`: run by a Claude Code Stop hook (which also fires in the VS Code extension).
/// Records only the time Claude last replied.
pub fn activity() {
    if std::env::var_os(crate::refresh::ENV_GUARD).is_some() {
        return;
    }
    let _ = std::io::copy(&mut std::io::stdin().take(MAX_STDIN), &mut std::io::sink());
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let dir = snapshot::data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let tmp = dir.join("activity.txt.tmp");
    if std::fs::write(&tmp, now.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, dir.join(ACTIVITY_FILE));
    }
}

pub fn run() {
    let mut input = Vec::new();
    let _ = std::io::stdin().take(MAX_STDIN).read_to_end(&mut input);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let dir = snapshot::data_dir();

    let new = extract(&input, now);
    if let Some(n) = &new {
        let old = std::fs::read(dir.join(snapshot::FILE_NAME)).ok().and_then(|b| snapshot::parse(&b).ok());
        if should_write(old.as_ref(), n) {
            let _ = write_atomic(&dir, n);
        }
    }

    let chained = std::fs::read_to_string(dir.join(CHAIN_FILE))
        .ok()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .and_then(|c| run_chain(&c, &input));
    let out = chained.unwrap_or_else(|| match &new {
        Some(n) if n.has_limits() => view::build(Some(n), false, now, true, &|_| String::new()).pill,
        _ => "TokenStat: waiting for usage data".into(),
    });
    let mut so = std::io::stdout();
    let _ = so.write_all(out.trim_end().as_bytes());
    let _ = so.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    const INPUT: &str = r#"{"session_id":"abc","transcript_path":"C:\\secret\\t.jsonl","cwd":"C:\\work",
        "model":{"display_name":"Opus"},"cost":{"total_cost_usd":1.2},
        "rate_limits":{"five_hour":{"used_percentage":7,"resets_at":1794885000},
                       "seven_day":{"used_percentage":16.5,"resets_at":1795028400}}}"#;

    fn snap(t: i64, five: f64) -> Snapshot {
        Snapshot { captured_at: t, five_hour: Some(Window { used_percentage: five, resets_at: 9_999_999_999 }), seven_day: None }
    }

    #[test]
    fn keeps_only_rate_limits() {
        let s = extract(INPUT.as_bytes(), 100).unwrap();
        assert_eq!(s.five_hour.unwrap().used_percentage, 7.0);
        assert_eq!(s.seven_day.unwrap().used_percentage, 16.5);
        let stored = String::from_utf8(serde_json::to_vec(&s).unwrap()).unwrap();
        for leak in ["session", "transcript", "secret", "cwd", "work", "cost", "Opus"] {
            assert!(!stored.contains(leak), "{leak} leaked into snapshot");
        }
    }

    #[test]
    fn written_snapshot_reads_back() {
        let s = extract(INPUT.as_bytes(), 100).unwrap();
        assert_eq!(snapshot::parse(&serde_json::to_vec(&s).unwrap()).unwrap(), s);
    }

    #[test]
    fn missing_rate_limits_gives_empty_snapshot() {
        assert!(!extract(br#"{"session_id":"x"}"#, 1).unwrap().has_limits());
    }

    #[test]
    fn garbage_input_gives_none() {
        assert!(extract(b"not json", 1).is_none());
        assert!(extract(b"", 1).is_none());
    }

    #[test]
    fn write_rules() {
        assert!(should_write(None, &snap(100, 7.0)));
        assert!(!should_write(Some(&snap(100, 7.0)), &snap(130, 7.0)));
        assert!(should_write(Some(&snap(100, 7.0)), &snap(130, 8.0)));
        assert!(should_write(Some(&snap(100, 7.0)), &snap(160, 7.0)));
        let empty = Snapshot { captured_at: 500, five_hour: None, seven_day: None };
        assert!(!should_write(Some(&snap(100, 7.0)), &empty));
    }

    #[cfg(unix)]
    #[test]
    fn chain_passes_stdin_and_returns_output() {
        assert_eq!(run_chain("tr a-z A-Z", b"hello").unwrap(), "HELLO");
    }
}
