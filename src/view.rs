use crate::snapshot::{Snapshot, Window};
use crate::sysstats::{format_rate, Enabled, Stats};

pub const STALE_AFTER_SECS: i64 = 15 * 60;
pub const TOOLTIP_MAX_CHARS: usize = 127;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    Live,
    Stale,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Normal,
    Amber,
    Red,
}

/// A run of pill text. `None` is plain label text; `Some(level)` is a usage value coloured by level.
pub type Segment = (String, Option<Level>);

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    /// (session %, weekly %) for the tray rings; None when missing or reset.
    pub rings: (Option<f64>, Option<f64>),
    pub pill: String,
    pub segments: Vec<Segment>,
    /// Pill items that wrap as a unit when the pill is narrow.
    pub items: Vec<Vec<Segment>>,
    pub tooltip: String,
    pub accessible: String,
    pub freshness: Freshness,
    pub level: Level,
    pub next_change: Option<i64>,
}

struct Part {
    label: String,
    value: String,
    tip: String,
    pct: Option<f64>,
    next: Option<i64>,
}

fn part(label: &str, w: Option<Window>, now: i64, fmt: &dyn Fn(i64) -> String) -> Part {
    match w {
        None => Part {
            label: label.into(),
            value: "--".into(),
            tip: format!("{label}: not provided"),
            pct: None,
            next: None,
        },
        Some(w) if w.resets_at <= now => Part {
            label: label.into(),
            value: "reset".into(),
            tip: format!("{label}: reset, waiting for Claude Code"),
            pct: None,
            next: None,
        },
        Some(w) => Part {
            label: label.into(),
            value: format!("{:.0}%", w.used_percentage),
            tip: format!("{label} {:.0}%, resets {}", w.used_percentage, fmt(w.resets_at)),
            pct: Some(w.used_percentage),
            next: Some(w.resets_at),
        },
    }
}

pub const AMBER_AT: f64 = 75.0;
pub const RED_AT: f64 = 90.0;

pub fn level_of(pct: f64) -> Level {
    if pct >= RED_AT {
        Level::Red
    } else if pct >= AMBER_AT {
        Level::Amber
    } else {
        Level::Normal
    }
}

fn level(pcts: &[Option<f64>]) -> Level {
    let max = pcts.iter().flatten().cloned().fold(0.0_f64, f64::max);
    if max >= RED_AT {
        Level::Red
    } else if max >= AMBER_AT {
        Level::Amber
    } else {
        Level::Normal
    }
}

fn truncate(s: String) -> String {
    if s.chars().count() <= TOOLTIP_MAX_CHARS {
        s
    } else {
        s.chars().take(TOOLTIP_MAX_CHARS).collect()
    }
}

fn simple(pill: &str, short: &str, labels: bool, tip: &str, freshness: Freshness, next_change: Option<i64>) -> View {
    let pill = if labels { pill } else { short };
    View {
        rings: (None, None),
        pill: pill.into(),
        segments: vec![(pill.into(), None)],
        items: vec![vec![(pill.into(), None)]],
        tooltip: truncate(format!("TokenStat\n{tip}")),
        accessible: format!("Claude usage. {tip}"),
        freshness,
        level: Level::Normal,
        next_change,
    }
}

/// `fmt` turns a Unix time into a short local time such as "2:10 pm" or "Thu 6:00 am".
/// `labels` false drops the "Session"/"Weekly" words from the pill only; tooltip and screen reader text keep them.
pub fn build(snap: Option<&Snapshot>, load_error: bool, now: i64, labels: bool, fmt: &dyn Fn(i64) -> String) -> View {
    build_with_activity(snap, load_error, now, labels, false, fmt)
}

/// `unseen` true means Claude replied somewhere the reading did not capture (for example the VS Code extension).
pub fn build_with_activity(
    snap: Option<&Snapshot>,
    load_error: bool,
    now: i64,
    labels: bool,
    unseen: bool,
    fmt: &dyn Fn(i64) -> String,
) -> View {
    let Some(s) = snap else {
        return if load_error {
            simple("Data error", "Error", labels, "Usage file could not be read", Freshness::Unknown, None)
        } else {
            simple("Waiting for Claude Code", "Waiting", labels, "Waiting for first Claude Code response", Freshness::Unknown, None)
        };
    };

    let stale_at = s.captured_at + STALE_AFTER_SECS;
    let freshness = if load_error || unseen || now >= stale_at { Freshness::Stale } else { Freshness::Live };
    let stale_next = (now < stale_at).then_some(stale_at);

    if !s.has_limits() {
        let tip = format!("Claude Code gave no usage limits for this account. As of {}", fmt(s.captured_at));
        return simple("No usage data", "No data", labels, &tip, freshness, stale_next);
    }

    let five = part("Session", s.five_hour, now, fmt);
    let week = part("Weekly", s.seven_day, now, fmt);
    let next_change = [stale_next, five.next, week.next].into_iter().flatten().filter(|t| *t > now).min();

    let prefix = if load_error {
        "Data error. "
    } else if unseen {
        "Used since. "
    } else if freshness == Freshness::Stale {
        "Stale. "
    } else {
        ""
    };
    let body = format!("{}\n{}\n{prefix}As of {}, from Claude Code", five.tip, week.tip, fmt(s.captured_at));

    let value = |p: &Part| (if labels { format!("{} ", p.label) } else { String::new() }, p.value.clone(), p.pct.map(level_of).unwrap_or(Level::Normal));
    let (l1, v1, c1) = value(&five);
    let (l2, v2, c2) = value(&week);
    View {
        rings: (five.pct, week.pct),
        pill: format!("{l1}{v1} | {l2}{v2}"),
        items: vec![vec![(l1.clone(), None), (v1.clone(), Some(c1))], vec![(l2.clone(), None), (v2.clone(), Some(c2))]],
        segments: vec![(l1, None), (v1, Some(c1)), (" | ".into(), None), (l2, None), (v2, Some(c2))],
        tooltip: truncate(body.clone()),
        accessible: format!("Claude usage. {}", body.replace('\n', ". ")),
        freshness,
        level: level(&[five.pct, week.pct]),
        next_change,
    }
}

/// System stats as pill items, one per enabled stat. `labels` false drops the CPU, RAM and GPU words,
/// the same as for the Claude figures; the network arrows stay because they carry the meaning.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn system_items(stats: &Stats, on: Enabled, labels: bool) -> Vec<Vec<Segment>> {
    let mut out = Vec::new();
    for (enabled, label, value) in [(on.cpu, "CPU ", stats.cpu), (on.ram, "RAM ", stats.ram), (on.gpu, "GPU ", stats.gpu)] {
        if enabled {
            let value = match value {
                Some(v) => (format!("{v:.0}%"), Some(level_of(v))),
                None => ("--".into(), None),
            };
            out.push(if labels { vec![(label.to_string(), None), value] } else { vec![value] });
        }
    }
    if on.net {
        let (d, u) = stats.net.map(|(d, u)| (format_rate(d), format_rate(u))).unwrap_or(("--".into(), "--".into()));
        out.push(vec![(format!("\u{2193}{d} \u{2191}{u}"), None)]);
    }
    out
}

/// Greedy row wrap: which items go on each row, given item widths, the row width and the gap between items.
/// An item wider than the row gets a row of its own.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn wrap(widths: &[i32], row_width: i32, gap: i32) -> Vec<Vec<usize>> {
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut used = 0;
    for (i, w) in widths.iter().enumerate() {
        match rows.last_mut() {
            Some(row) if used + gap + w <= row_width => {
                row.push(i);
                used += gap + w;
            }
            _ => {
                rows.push(vec![i]);
                used = *w;
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_794_871_300;

    fn fmt(t: i64) -> String {
        format!("T{t}")
    }

    fn snap(captured: i64, five: Option<(f64, i64)>, week: Option<(f64, i64)>) -> Snapshot {
        let w = |o: Option<(f64, i64)>| o.map(|(p, r)| Window { used_percentage: p, resets_at: r });
        Snapshot { captured_at: captured, five_hour: w(five), seven_day: w(week) }
    }

    #[test]
    fn live_reading() {
        let s = snap(NOW - 60, Some((7.0, NOW + 3600)), Some((16.0, NOW + 90_000)));
        let v = build(Some(&s), false, NOW, true, &fmt);
        assert_eq!(v.pill, "Session 7% | Weekly 16%");
        assert_eq!(v.freshness, Freshness::Live);
        assert_eq!(v.level, Level::Normal);
        assert!(v.tooltip.contains("Session 7%, resets T"));
        assert!(!v.tooltip.contains("Stale"));
        assert_eq!(v.next_change, Some(NOW - 60 + STALE_AFTER_SECS));
        assert_eq!(v.rings, (Some(7.0), Some(16.0)));
    }

    #[test]
    fn stale_after_15_minutes() {
        let s = snap(NOW - STALE_AFTER_SECS, Some((7.0, NOW + 3600)), Some((16.0, NOW + 90_000)));
        let v = build(Some(&s), false, NOW, true, &fmt);
        assert_eq!(v.freshness, Freshness::Stale);
        assert!(v.tooltip.contains("Stale. As of"));
        assert_eq!(v.next_change, Some(NOW + 3600));
    }

    #[test]
    fn passed_reset_shows_reset_not_old_value() {
        let s = snap(NOW - 60, Some((88.0, NOW - 1)), Some((16.0, NOW + 90_000)));
        let v = build(Some(&s), false, NOW, true, &fmt);
        assert_eq!(v.pill, "Session reset | Weekly 16%");
        assert_eq!(v.rings, (None, Some(16.0)));
        assert_eq!(v.level, Level::Normal);
    }

    #[test]
    fn amber_and_red_thresholds() {
        let a = build(Some(&snap(NOW, Some((75.0, NOW + 10)), None)), false, NOW, true, &fmt);
        let r = build(Some(&snap(NOW, Some((10.0, NOW + 10)), Some((90.0, NOW + 10)))), false, NOW, true, &fmt);
        let n = build(Some(&snap(NOW, Some((74.0, NOW + 10)), None)), false, NOW, true, &fmt);
        assert_eq!(a.level, Level::Amber);
        assert_eq!(r.level, Level::Red);
        assert_eq!(n.level, Level::Normal);
    }

    #[test]
    fn each_value_coloured_separately() {
        let v = build(Some(&snap(NOW, Some((99.0, NOW + 10)), Some((16.0, NOW + 10)))), false, NOW, true, &fmt);
        assert_eq!(v.segments[1], ("99%".to_string(), Some(Level::Red)));
        assert_eq!(v.segments[4], ("16%".to_string(), Some(Level::Normal)));
        assert_eq!(v.segments.iter().map(|s| s.0.as_str()).collect::<String>(), v.pill);
    }

    #[test]
    fn unseen_use_marks_reading_old() {
        let s = snap(NOW - 60, Some((7.0, NOW + 3600)), Some((16.0, NOW + 90_000)));
        let v = build_with_activity(Some(&s), false, NOW, true, true, &fmt);
        assert_eq!(v.freshness, Freshness::Stale);
        assert!(v.tooltip.contains("Used since. As of"));
    }

    #[test]
    fn system_items_per_stat() {
        let stats = Stats { cpu: Some(12.4), ram: Some(91.0), gpu: None, net: Some((2048.0, 0.0)) };
        let on = Enabled { cpu: true, ram: true, gpu: true, net: true };
        let text = |items: Vec<Vec<Segment>>| -> Vec<String> { items.iter().map(|i| i.iter().map(|s| s.0.as_str()).collect()).collect() };
        let items = system_items(&stats, on, true);
        assert_eq!(items[1][1], ("91%".to_string(), Some(Level::Red)));
        assert_eq!(text(items), ["CPU 12%", "RAM 91%", "GPU --", "\u{2193}2 KB/s \u{2191}0 KB/s"]);
        assert_eq!(text(system_items(&stats, on, false)), ["12%", "91%", "--", "\u{2193}2 KB/s \u{2191}0 KB/s"]);
        assert!(system_items(&stats, Enabled::default(), true).is_empty());
    }

    #[test]
    fn wrap_fills_rows_in_order() {
        assert_eq!(wrap(&[50, 50, 50], 200, 10), vec![vec![0, 1, 2]]);
        assert_eq!(wrap(&[50, 50, 50], 120, 10), vec![vec![0, 1], vec![2]]);
        assert_eq!(wrap(&[50, 50, 50], 60, 10), vec![vec![0], vec![1], vec![2]]);
        assert_eq!(wrap(&[300, 50], 100, 10), vec![vec![0], vec![1]]);
        assert!(wrap(&[], 100, 10).is_empty());
    }

    #[test]
    fn labels_hidden() {
        let s = snap(NOW, Some((7.0, NOW + 10)), Some((16.0, NOW + 10)));
        let v = build(Some(&s), false, NOW, false, &fmt);
        assert_eq!(v.pill, "7% | 16%");
        assert!(v.tooltip.contains("Session 7%"));
        assert_eq!(build(None, false, NOW, false, &fmt).pill, "Waiting");
    }

    #[test]
    fn missing_window_shows_dashes() {
        let v = build(Some(&snap(NOW, Some((5.0, NOW + 10)), None)), false, NOW, true, &fmt);
        assert_eq!(v.pill, "Session 5% | Weekly --");
    }

    #[test]
    fn no_file_waits() {
        let v = build(None, false, NOW, true, &fmt);
        assert_eq!(v.pill, "Waiting for Claude Code");
        assert_eq!(v.freshness, Freshness::Unknown);
    }

    #[test]
    fn no_limits_says_so() {
        let v = build(Some(&snap(NOW, None, None)), false, NOW, true, &fmt);
        assert_eq!(v.pill, "No usage data");
    }

    #[test]
    fn load_error_keeps_values_but_marks_stale() {
        let s = snap(NOW - 60, Some((7.0, NOW + 3600)), None);
        let v = build(Some(&s), true, NOW, true, &fmt);
        assert_eq!(v.pill, "Session 7% | Weekly --");
        assert_eq!(v.freshness, Freshness::Stale);
        assert!(v.tooltip.contains("Data error."));
    }

    #[test]
    fn tooltip_fits_tray_limit() {
        let long = |_: i64| "x".repeat(80);
        let s = snap(NOW, Some((7.0, NOW + 10)), Some((16.0, NOW + 10)));
        assert!(build(Some(&s), false, NOW, true, &long).tooltip.chars().count() <= TOOLTIP_MAX_CHARS);
    }
}
