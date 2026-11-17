# TokenStat Testing

Version 0.1 | Date 2026-11-17 | Author Abi Chaudhuri | Status Current

Purpose: What has been tested, how, and the results. Targets are in README (resource budgets); this file holds measured results only.

## 1. Automated

| ID | Test | Where run | Result | Date |
|---|---|---|---|---|
| AT-01 | Unit tests (hook: keeps only rate_limits, no session/path leakage, write rules, chaining; labels on/off; plus): snapshot parsing (BOM, nulls, missing, clamping, bad reset, truncated, oversized) and view (live, stale, reset passed, amber/red, missing window, no file, no limits, data error, tooltip length) | Linux cloud workspace, Rust 1.97 | 16 passed at that date; not re-run on Linux since (see AT-03) | 2026-11-17 |
| AT-02 | Windows type check and clippy (`cargo clippy --target x86_64-pc-windows-msvc`) | Linux cloud workspace | No errors, no warnings at that date; not re-run since | 2026-11-17 |
| AT-03 | Unit tests on Windows (`cargo test`) | maintainer's laptop, Rust 1.100.0 | 37 passed, 0 failed, 1 ignored (`export_logo`, asset build only). The Unix-only chain test is not compiled on Windows | 2026-11-17 |

AT-02 checks the code compiles for Windows; it does not link or run it.

## 2. Manual checks (maintainer's laptop)

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
| MT-21 | Reply in VS Code extension, auto-refresh off | Pill turns grey; tooltip says "Used since" | Pass (observed during MT-22 setup, 2026-11-17) |
| MT-22 | Turn on auto-refresh, reply in VS Code extension | Impact dialog shown first; pill updates within ~10 s; menu shows 1 today and tokens | Pass 2026-11-17: pill 34% / 21% matched extension panel, green dot. Token count TBC |
| MT-23 | Start with Windows on, sign out and in | TokenStat starts | TBC |
| MT-24 | uninstall.ps1 | Settings restored; exe, data and Run entry gone | TBC |
| MT-25 | Turn on each system stat; drag pill edges narrower and wider | Each stat appears after the last; items wrap into rows when narrow, one row when wide; width kept after restart; CPU, RAM, GPU within a few points of Task Manager; network matches Task Manager throughput roughly | TBC |
| MT-26 | Hide pill with stats on | Sampling stops (CPU of tokenstat.exe 0 in Task Manager) | TBC |
| MT-27 | All stats on, 10 min | tokenstat.exe under 1% CPU average, under 20 MB memory | TBC |
| MT-19 | Live tray icon and tooltip | Rings readable on taskbar; tooltip shows both %, resets, as-of | Pass, maintainer screenshot 2026-11-17 |
| MT-17 | Hook timing, 10 runs | Under 100 ms each | TBC |
| MT-18 | Hide labels, restart | Pill shows "7% \| 16%", narrower; setting kept | Pass (owner, 2026-11-17) |
| MT-28 | Hide labels with system stats on | CPU, RAM, GPU words hidden too; network arrows stay | Pass (owner, 2026-11-17) |
| MT-29 | install.ps1 Start menu shortcut and Apps entry | TokenStat in Start search with logo; listed in Settings > Apps | Pass (owner, 2026-11-17) |
| MT-30 | Untick auto-refresh, use VS Code extension | No new refreshes; pill greys only | Code reviewed; owner confirmed behaviour expectation, not measured |

## 3. Known gaps

Slice 1 gaps now closed: live tray icon (MT-19), real hook instead of the spike file (MT-16).

- Atomic snapshot write (ADR-08) is built but not tested.
- Hook timing (MT-17) and auto-refresh tokens per refresh (MT-22) not yet measured.
- Chaining a previous status line is unit tested on Linux only; not yet tested on Windows.
- Rounded corners may not apply to a frameless window; to be checked visually.
