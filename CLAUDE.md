# TokenStat: notes for Claude Code

Windows 11 tray app and floating pill showing Claude usage (session and weekly %) plus optional CPU, RAM, GPU and network stats. Rust, raw Win32 via `windows-sys`, no UI framework. MIT.

## Repo

- GitHub: https://github.com/Abic7/TokenStat (owner's personal account, public). Local clone: `C:\Users\Abi.Chaudhuri\Local Projects\TokenStat`.
- Commits use the owner's GitHub no-reply email (set in this repo's git config). Never push with or to the owner's employer GitHub account, and do not move the project back into a work OneDrive folder.
- Push only when the owner asks.

## Status (2026-11-17)

Confirmed working on the owner's laptop: status line hook and live pill, Session/Weekly labels, per-figure amber/red colours, light/dark, live tray rings and tooltip, Hide labels for all items, VS Code "Used since" detection and opt-in auto-refresh (figures matched the extension panel), disabling auto-refresh stops background requests, install script with Start menu shortcut and Settings > Apps entry.
Built but not yet confirmed by the owner: free-shape resize, system stats accuracy, Start with Windows, uninstall. See `docs/TESTING.md`.

## Build and test

```powershell
$env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\TokenStat-build"   # keep build output out of the repo
cargo test
cargo clippy --target x86_64-pc-windows-msvc
cargo build --release
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1   # exits running TokenStat, reinstalls, restarts it
```

Show the install command to the user and let them run it: it edits `~/.claude/settings.json` (status line and Stop hook).

## Layout

| File | Role |
|---|---|
| `src/main.rs` | Modes: no args = app; `--statusline` = Claude Code status line hook; `--activity` = Stop hook |
| `src/app.rs` | Win32 window, tray, menu, layout, paint, timers, settings, auto-refresh orchestration |
| `src/view.rs` | Pure view model: pill items, tooltip, freshness, colour levels, row wrap. Unit tested |
| `src/snapshot.rs` | Snapshot file format and parsing |
| `src/hook.rs` | Status line and activity hooks; chains a user's previous status line (`chain.txt`) |
| `src/refresh.rs` | Opt-in hidden `claude -p` refresh; parses `rate_limit_event` |
| `src/sysstats.rs` | CPU, RAM, network, GPU (PDH) sampling |
| `src/icon.rs` | Twin-ring icon renderer (tray icon and logo) |
| `scripts/` | Per-user install and uninstall |
| `promo/` | Remotion source for the README demo video |
| `docs/` | Architecture and security review, decisions (ADR-01 to ADR-20), feasibility of data sources, testing |

Runtime data: `%LOCALAPPDATA%\TokenStat` (`snapshot.json`, `activity.txt`, `settings.txt`, `pill.pos`, `refresh-stats.txt`). Installed exe: `%LOCALAPPDATA%\Programs\TokenStat`.

## Rules

- Keep it light: no new dependencies, no polling while idle, no network use by TokenStat itself. Check new crates' licences are MIT-compatible.
- Store only percentages, reset times and timestamps. Never store or log prompts, session IDs, paths or transcripts from hook input.
- Never present an estimate or stale reading as current. Missing data shows as `--` or "unavailable".
- Auto-refresh stays opt-in with the impact dialog. It uses `unifiedWindows`, which is undocumented: parse defensively.
- Do not add telemetry, scraping, cookie or credential access, or undocumented endpoints.
- Pure logic goes in `view.rs`, `snapshot.rs`, `refresh.rs` or `sysstats.rs` with unit tests; `app.rs` stays thin Win32 glue.
- Record new design decisions in `docs/DECISIONS.md` and test results in `docs/TESTING.md`.

## Open items

- Measure auto-refresh tokens per refresh (menu shows count and tokens) and put the figure in the impact dialog.
- Run TESTING checks not yet done: Start with Windows, uninstall, system stats against Task Manager, 10-minute and 24-hour resource checks.
- Add `cargo-deny` (licences and advisories) before a binary release.
- Code signing: deferred; costs money, needs the owner's approval.
- Possible: GitHub release with a prebuilt exe so users need not install Rust.
