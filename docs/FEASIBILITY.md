# TokenStat Feasibility

Version 0.3 | Date 2026-11-17 | Author Abi Chaudhuri | Status Current

Purpose: Record which Claude usage data sources exist, how trustworthy they are, and which one Phase 1 should use.

## 1. Verified facts (checked 2026-11-17)

| ID | Fact | Source |
|---|---|---|
| V-01 | Claude Code's status line feature sends JSON to a user-configured command. It includes `rate_limits.five_hour` and `rate_limits.seven_day`, each with `used_percentage` (0 to 100) and `resets_at` (Unix seconds). | code.claude.com/docs/en/statusline (no date on page) |
| V-02 | `rate_limits` appears only for claude.ai Pro and Max subscribers (or behind a Claude apps gateway with spend limits), and only after the session's first API response. Each window can be absent. A window is dropped once its `resets_at` passes. | same |
| V-03 | The status line runs on events (after responses etc.), debounced at 300 ms, plus when a `resets_at` time is reached. Optional `refreshInterval` re-runs on a timer. | same |
| V-04 | For individual Pro, Max and Ultra plans, limits are shared across Claude and Claude Code. Team and Enterprise members are pointed to a separate article. | support.claude.com/en/articles/11145838, "updated over 2 weeks ago" |
| V-05 | claude.ai Settings > Usage shows five-hour session and weekly limits as progress bars, plus a separate Fable weekly limit where included. No API is mentioned. | support.claude.com/en/articles/9797557, "updated over 2 weeks ago" |
| V-06 | The Usage and Cost Admin API needs an Admin key (or `org:admin` OAuth). It is not available to claude.ai Team plan organisations or individual Console accounts. Data lags about 5 minutes; poll at most once per minute. It covers API usage, not subscription allowance. | platform.claude.com/docs/en/build-with-claude/usage-cost-api (no date on page) |

## 2. Usage surfaces

| Surface | What "usage" means | Exposed to a local app? |
|---|---|---|
| Claude web / desktop / mobile | Shared subscription allowance (5-hour session, weekly, Fable weekly) | Only via Settings > Usage UI. No documented API. |
| Claude Code (subscription login) | Same shared allowance (V-04, individual plans) | Yes, as percentages and reset times via status line (V-01). |
| Claude Code (API key) | API spend and tokens, not allowance | Token and cost fields in status line; spend via Admin API if org admin. |
| Anthropic API | Monetary spend, tokens, rate limits per org | Admin API (V-06). Not relevant to subscription users. |

## 3. Candidate data sources

| ID | Source | Data | Official? | Auth | Freshness | Accuracy / coverage | Privacy / terms | Maintenance | Phase 1 |
|---|---|---|---|---|---|---|---|---|---|
| S-01 | Claude Code status line hook | 5h and 7d % used, reset times | Yes, documented | None extra; uses existing Claude Code login | As of last Claude Code response | Server-reported %, not an estimate. No Fable weekly bar. Only Pro/Max documented. | Read-only, local. Must drop prompt paths and other fields. | Low; field names may change between versions | Recommended, pending spike |
| S-02 | claude.ai internal usage endpoint (what Settings > Usage calls) | Same bars incl. Fable | No, undocumented | Session cookie | On demand | Matches UI, may break anytime | Cookie extraction; terms risk | High | Out. Needs your explicit approval. |
| S-03 | Admin Usage and Cost API | API tokens and spend | Yes | Admin key | ~5 min lag | API only, no allowance | Org-level secret | Low | Out (wrong data; not on Team plan) |
| S-04 | Claude Code local session logs | Observed tokens per message | No (format undocumented) | File access | Near real time | Tokens only. Cannot derive remaining allowance. | Contains conversation content; must not read | Medium | Out |
| S-05 | Claude Code OpenTelemetry export | Tokens, cost, sessions | Yes | Local collector | Near real time | Tokens and cost, not allowance | Needs a collector process | Medium | Out (Phase 2 candidate at most) |
| S-06 | `/status` command in Claude Code | Remaining allocation (text) | Yes, interactive | n/a | On demand | Human-readable only | n/a | n/a | Out (not machine-readable) |
| S-08 | Claude Code local cache (`cachedUsageUtilization` in ~/.claude.json) | Session, weekly, Fable %, reset times | No, undocumented | File read | Checked 2026-11-17: fetched 9:04 am, showed session 0%, weekly 15%, while live figures were 21% and 19%. Not refreshed by VS Code extension use. | Stale; format undocumented | File also holds account ID and other private settings | High | Rejected |
| S-09 | One-off `claude -p --output-format stream-json` request, `rate_limit_event` line | Session and weekly %, reset times (`unifiedWindows`); status | Event type is in the published Agent SDK types (SDKRateLimitInfo: status, resetsAt, rateLimitType, utilization). The per-window `unifiedWindows` figures are NOT in the published types: undocumented. | Existing Claude Code login, handled by Claude Code itself | On demand, ~3 s | Spike D-02 2026-11-17: session 28%, weekly 20%, resets match status line. Status line does not run in -p mode. | Read only the rate_limit_event line; ignore everything else | Medium | Candidate for VS Code refresh; needs maintainer approval (undocumented field). Each refresh uses a small amount of allowance. |
| S-07 | Browser automation of Settings > Usage | Same bars | No | Live session | On demand | Fragile | Scraping; terms risk | High | Out. Needs your explicit approval. |

## 4. Assumptions and unknowns

| ID | Item | Status |
|---|---|---|
| A-01 | The maintainer's Claude Code login is a Team plan seat, not individual Pro/Max. | Confirmed 2026-11-17 |
| A-02 | Status line `rate_limits` is populated for Team plan seats. Docs only name Pro and Max. | Confirmed 2026-11-17 on Claude Code 2.1.319, Team seat: five_hour and seven_day present. Not documented, so could change. |
| A-03 | Team plan limits are shared between claude.ai and Claude Code, so the % reflects all surfaces. | Confirmed 2026-11-17: claude.ai use moved claude.ai to 7%; Claude Code showed 6% at 9:59 am, then 7% at 10:21 am. Values are whole numbers and can trail claude.ai by about 1 point. |
| A-04 | The status line % matches claude.ai Settings > Usage within a few points. | Confirmed 2026-11-17: 6% / resets 2:10 pm and 16% / resets Thu 6:00 am match exactly. |
| A-05 | Running a writer per status update has acceptable CPU cost on Windows. | PowerShell spike measured 326 to 359 ms per run (10 runs, 2026-11-17), over the 100 ms budget. Expected from PowerShell startup; Rust hook to be measured in slice 2. |
| A-06 | Settings > Usage offers manual "Limit resets" (full and Fable). Using one changes the real % but TokenStat only sees it after the next Claude Code response. | Observed 2026-11-17. Show as Stale until refreshed. |
| A-07 | Organisation-managed Claude Code settings could override or block the status line hook. | Risk; Claude Code shows "organization may apply global configuration". |
| A-09 | VS Code extension chat panel runs the status line hook. | Not observed 2026-11-17: snapshot unchanged (captured_at 1794873724) after a message in the extension panel. Claude Code in VS Code's integrated terminal does run it. Readings refresh only on CLI use. |
| A-08 | Status line JSON also carries session_id, transcript_path, cwd and other fields. TokenStat must discard all but rate_limits. | Observed 2026-11-17 |

## 5. Honest product boundary

- TokenStat can show the last server-reported 5-hour and weekly % used, reset times, and how old that reading is.
- It cannot see usage from claude.ai web until the next Claude Code response refreshes the reading.
- It cannot show absolute tokens remaining. Anthropic exposes percentages only.
- It cannot show the Fable weekly bar via S-01 today.
- If A-02 fails, the only routes to allowance data are S-02 or S-07, both undocumented. That would be a no-go for an honest Phase 1 unless you approve them.

## 6. Go / no-go

Go (2026-11-17). A-02, A-03 and A-04 confirmed on a Team plan seat. Display whole % with an "as of" time; never imply live accuracy.
