# TokenStat Testing

Version 0.1 | Date 2026-11-17 | Author Abi Chaudhuri | Status Current

Purpose: What has been tested, how, and the results. Targets are in README (resource budgets); this file holds measured results only.

## 1. Automated

| ID | Test | Where run | Result | Date |
|---|---|---|---|---|
| AT-01 | 24 unit tests (hook: keeps only rate_limits, no session/path leakage, write rules, chaining; labels on/off; plus): snapshot parsing (BOM, nulls, missing, clamping, bad reset, truncated, oversized) and view (live, stale, reset passed, amber/red, missing window, no file, no limits, data error, tooltip length) | Linux cloud workspace, Rust 1.97 | 16 passed | 2026-11-17 |
| AT-02 | Windows type check and clippy (`cargo clippy --target x86_64-pc-windows-msvc`) | Linux cloud workspace | No errors, no warnings | 2026-11-17 |
| AT-03 | Unit tests on Windows (`cargo test`) | maintainer's laptop | TBC | |

AT-02 checks the code compiles for Windows; it does not link or run it.

## 2. Slice 1 manual checks (maintainer's laptop)

| ID | Check | Expected | Result |
|---|---|---|---|
| MT-01 | Release build | Builds; exe size noted | TBC |
| MT-02 | Launch | Pill appears bottom-right with green dot and "Session x% \| Weekly y%" matching Claude Code | maintainer: slice 1 works as expected, 2026-11-17 (labels changed after) |
| MT-03 | Drag pill, exit, relaunch | Reopens where left | TBC |
| MT-04 | Double-click pill, or Esc | Pill hides; tray icon stays | TBC |
| MT-05 | Hover tray icon | Tooltip: both %, reset times, "As of" time | TBC |
| MT-06 | Left-click tray | Pill toggles | TBC |
| MT-07 | Right-click tray and pill | Menu: Show/Hide pill, Exit | TBC |
| MT-08 | Send a Claude Code message | Pill updates within 2 s | TBC |
| MT-09 | Wait 15 min without Claude Code | Dot turns grey; tooltip starts "Stale." | TBC |
| MT-10 | Launch a second copy | Nothing happens (single instance) | TBC |
| MT-11 | Restart Explorer | Tray icon comes back | TBC |
| MT-12 | Move pill to a second monitor with different scaling | Text stays crisp and sized | TBC |
| MT-13 | Memory and CPU after 10 min idle | Working set under 15 MB; CPU 0 | TBC |
| MT-14 | Per-value colour (99% session, 80% weekly test reading), dark mode | 99% red, 80% amber, labels plain | Pass, maintainer screenshot 2026-11-17 |
| MT-15 | Light mode | Light pill, readable colours | Done by maintainer 2026-11-17 |
| MT-16 | Real hook in Claude Code | Status bar shows Session/Weekly; pill updates after each reply | Pass: hook wrote snapshot.json at 11:02 am, 2026-11-17 |
| MT-20 | install.ps1 on maintainer's laptop | Status line and Stop hook in settings.json; backup made; app starts | Pass 2026-11-17: both entries present, Stop stored as array |
| MT-21 | Reply in VS Code extension, auto-refresh off | Pill turns grey; tooltip says "Used since" | TBC |
| MT-22 | Turn on auto-refresh, reply in VS Code extension | Impact dialog shown first; pill updates within ~10 s; menu shows 1 today and tokens | Pass 2026-11-17: pill 34% / 21% matched extension panel, green dot. Token count TBC |
| MT-23 | Start with Windows on, sign out and in | TokenStat starts | TBC |
| MT-24 | uninstall.ps1 | Settings restored; exe, data and Run entry gone | TBC |
| MT-25 | Turn on each system stat; drag pill edges narrower and wider | Each stat appears after the last; items wrap into rows when narrow, one row when wide; width kept after restart; CPU, RAM, GPU within a few points of Task Manager; network matches Task Manager throughput roughly | TBC |
| MT-26 | Hide pill with stats on | Sampling stops (CPU of tokenstat.exe 0 in Task Manager) | TBC |
| MT-27 | All stats on, 10 min | tokenstat.exe under 1% CPU average, under 20 MB memory | TBC |
| MT-19 | Live tray icon and tooltip | Rings readable on taskbar; tooltip shows both %, resets, as-of | Pass, maintainer screenshot 2026-11-17 |
| MT-17 | Hook timing, 10 runs | Under 100 ms each | TBC |
| MT-18 | Hide labels, restart | Pill shows "7% \| 16%", narrower; setting kept | TBC |

## 3. Known gaps in slice 1

- Tray icon is the generic Windows application icon (T-06).
- Reads the spike's `spike-snapshot.json`; the real hook comes in slice 2.
- A partly written spike file can briefly show "Data error"; the atomic write in slice 2 removes this.
- Rounded corners may not apply to a frameless window; to be checked visually.
