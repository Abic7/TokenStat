# TokenStat Architecture

Version 0.1 | Date 2026-11-17 | Author Abi Chaudhuri | Status Current

Purpose: How TokenStat is built, the security review, and the task breakdown. Decisions and their reasons live in DECISIONS.md.

## 1. Components

| ID | Component | File | Runs on | Job |
|---|---|---|---|---|
| C-01 | Snapshot model | src/snapshot.rs | any (pure) | Parse and validate the snapshot JSON. Only `captured_at`, `five_hour`, `seven_day` are read. |
| C-02 | View model | src/view.rs | any (pure) | Turn a snapshot plus the current time into pill text, tooltip, freshness and colour level. Unit tested. |
| C-03 | App shell | src/app.rs | Windows | Pill window, tray icon, menu, message loop, folder watch, timer, position memory. |
| C-04 | Status line hook | src/hook.rs (slice 2) | Windows | `tokenstat.exe --statusline`: read stdin, keep rate_limits only, atomic write, chain the user's previous command. |
| C-05 | Installer | slice 5 | Windows | Copy exe, add or restore status line setting, Run key, uninstall. |

## 2. Data flow

1. Claude Code finishes a response and runs the status line command, passing JSON on stdin.
2. Hook (C-04) writes `%LOCALAPPDATA%\TokenStat\snapshot.json` (temp file then rename) only when values change.
3. App (C-03) is asleep in one wait: window messages, a folder-change handle, one waitable timer.
4. Folder change wakes it. It re-reads the file (max 64 KB), parses (C-01), rebuilds the view (C-02), repaints the pill and updates the tray tooltip.
5. The timer is set to the next moment the view changes on its own: reading turns stale (15 min) or a window passes its reset time. Resume from sleep and clock changes also rebuild the view.

Until the hook exists (slice 2), the app reads the spike's `spike-snapshot.json` so it shows real data from day one.

## 3. Display states

| State | Pill | Tooltip |
|---|---|---|
| No file yet | grey dot, "Waiting for Claude Code" | Waiting for first Claude Code response |
| No rate limits in file | grey dot, "No usage data" | Not provided for this account |
| Live (under 15 min) | green dot, "Session 7% \| Weekly 16%" | per-window % and reset time, "As of 10:21 am" |
| Stale (15 min or more) | grey dot, same numbers | "Stale:" prefix |
| Window reset passed | that window shows "reset" | "reset, waiting for Claude Code" |
| Unreadable file | last good values, stale | "Data error" prefix |
| 75% or more / 90% or more | that % turns amber / red (per value) | same text |
| Light or dark Windows setting | light or dark pill | n/a |

## 4. Security and privacy review (G-02, inline, 2026-11-17)

| ID | Risk | Mitigation | Status |
|---|---|---|---|
| SEC-01 | Status line stdin carries session ID, transcript path, working folder, cost and model data. | Hook deserialises only `rate_limits`; never logs or stores stdin. Snapshot holds %, reset times, capture time only. | Design |
| SEC-02 | Editing `~/.claude/settings.json` could break the user's setup or a managed config. | Back up first; write without BOM; chain existing `statusLine`; restore exactly on uninstall; never touch managed settings files. | Design |
| SEC-03 | Chaining runs the user's previous status line command. | Store it verbatim from their own settings; run it with the same stdin; time-limit it; TokenStat failure never blocks its output. Same-user trust boundary. | Design |
| SEC-04 | Hook command path has spaces (OneDrive). | Install exe to `%LOCALAPPDATA%\Programs\TokenStat` and quote the path. | Design |
| SEC-05 | Another local process could write a fake snapshot. | Same user, low impact. Parse defensively: size cap, % clamped 0 to 100, bad file keeps last good reading. | Slice 1 |
| SEC-06 | Supply chain for an open-source binary. | Few dependencies (windows-sys, serde, serde_json); commit Cargo.lock; run cargo-deny (licences, advisories) before release. | Slice 6 |
| SEC-07 | Accidental network use. | No networking crates. Verify zero connections with Resource Monitor in testing. | Test |
| SEC-08 | Logs leaking data. | No logging in Phase 1. | Design |
| SEC-09 | Org-managed Claude Code settings may override the hook. | Detect "no snapshot" and show "Waiting"; document. | Known limit |
| SEC-10 | Prompt injection via agent instruction files in the repo. | Repo has none. Any future CLAUDE.md must not tell an AI to run scripts unseen. | Check at each review |

## 5. Task breakdown

| ID | Slice | Task | Done when | Status |
|---|---|---|---|---|
| T-01 | 1 | Snapshot parse + view model with unit tests | `cargo test` passes | Done (see TESTING) |
| T-02 | 1 | Pill window: draggable, topmost, no taskbar button, DPI aware, remembers position, Esc or double-click hides | maintainer drags, restarts, position kept | Built, untested on Windows |
| T-03 | 1 | Tray icon: tooltip, left-click toggles pill, right-click menu (Show/Hide, Exit), survives Explorer restart | maintainer checks each | Built, untested on Windows |
| T-04 | 1 | Folder watch + single timer + resume/clock-change refresh | Pill updates within 2 s of a Claude Code reply | Built, untested on Windows |
| T-05 | 2 | `--statusline` hook, atomic write, chaining (previous command in `%LOCALAPPDATA%\TokenStat\chain.txt`) | Under 100 ms; output matches previous status line | Built; unit tests pass; Linux smoke run OK; Windows timing TBC |
| T-10 | 1 | Show/Hide labels menu item; pill narrows; setting kept in settings.txt | maintainer toggles and restarts | Built, untested on Windows |
| T-06 | 3 | Twin-ring logo, exe icon, live tray icon | Visible at 100% and 200% scaling | Built; renderer unit tested; untested on Windows |
| T-11 | 4 | Stop hook `--activity` (runs in terminal and VS Code extension); "Used since" marking; opt-in auto-refresh with impact dialog, 5-min cap, backoff, daily refresh/token count in menu | maintainer enables, uses VS Code extension, pill updates within ~10 s | Built; unit tests pass; untested on Windows |
| T-07 | 4 | Error states, stale handling soak, sleep/resume tests | TESTING.md matrix rows pass | Open |
| T-08 | 5 | Start with Windows menu toggle; scripts/install.ps1 and uninstall.ps1 (backup, chain existing status line, add/remove hooks, restore) | Clean install and full removal verified | Built; scripts not run (no PowerShell here) |
| T-09 | 6 | Release build, size check, cargo-deny, README, MIT LICENSE | Budgets measured; maintainer approves publish | Open |

Critical path: T-02 to T-04 (Windows behaviour) then T-05 (hook timing).
