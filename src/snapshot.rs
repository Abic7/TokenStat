use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MAX_FILE_BYTES: usize = 64 * 1024;
pub const FILE_NAME: &str = "snapshot.json";

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("TokenStat")
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct Window {
    pub used_percentage: f64,
    pub resets_at: i64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Snapshot {
    pub captured_at: i64,
    #[serde(default)]
    pub five_hour: Option<Window>,
    #[serde(default)]
    pub seven_day: Option<Window>,
}

impl Snapshot {
    pub fn has_limits(&self) -> bool {
        self.five_hour.is_some() || self.seven_day.is_some()
    }
}

pub fn parse(bytes: &[u8]) -> Result<Snapshot, String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err("snapshot too large".into());
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let mut s: Snapshot = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    for w in [&mut s.five_hour, &mut s.seven_day] {
        if let Some(win) = w {
            if !win.used_percentage.is_finite() || win.resets_at <= 0 {
                *w = None;
            } else {
                win.used_percentage = win.used_percentage.clamp(0.0, 100.0);
            }
        }
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPIKE: &str = r#"{
        "captured_at": 1794871281, "cc_version": "2.1.319", "model": "Opus 5.5",
        "has_rate_limits": true, "rate_limit_keys": ["five_hour","seven_day"],
        "five_hour": {"used_percentage": 7, "resets_at": 1794885000},
        "seven_day": {"used_percentage": 16, "resets_at": 1795028400},
        "top_level_keys": ["session_id"]
    }"#;

    #[test]
    fn parses_spike_file_with_bom() {
        let mut b = b"\xEF\xBB\xBF".to_vec();
        b.extend_from_slice(SPIKE.as_bytes());
        let s = parse(&b).unwrap();
        assert_eq!(s.captured_at, 1794871281);
        assert_eq!(s.five_hour.unwrap().used_percentage, 7.0);
        assert_eq!(s.seven_day.unwrap().resets_at, 1795028400);
    }

    #[test]
    fn null_windows_mean_no_limits() {
        let s = parse(br#"{"captured_at": 1, "five_hour": null, "seven_day": null}"#).unwrap();
        assert!(!s.has_limits());
    }

    #[test]
    fn missing_windows_mean_no_limits() {
        let s = parse(br#"{"captured_at": 1}"#).unwrap();
        assert!(!s.has_limits());
    }

    #[test]
    fn clamps_out_of_range_percent() {
        let s = parse(br#"{"captured_at": 1, "five_hour": {"used_percentage": 140, "resets_at": 5}}"#).unwrap();
        assert_eq!(s.five_hour.unwrap().used_percentage, 100.0);
    }

    #[test]
    fn rejects_bad_reset_time() {
        let s = parse(br#"{"captured_at": 1, "five_hour": {"used_percentage": 5, "resets_at": 0}}"#).unwrap();
        assert!(s.five_hour.is_none());
    }

    #[test]
    fn rejects_truncated_file() {
        assert!(parse(br#"{"captured_at": 1, "five_ho"#).is_err());
    }

    #[test]
    fn rejects_oversized_file() {
        assert!(parse(&vec![b' '; MAX_FILE_BYTES + 1]).is_err());
    }
}
