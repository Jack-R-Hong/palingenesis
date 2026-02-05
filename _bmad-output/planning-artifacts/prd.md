---
stepsCompleted: ['step-01-init', 'step-02-discovery', 'step-03-success', 'step-04-journeys', 'step-05-domain', 'step-06-innovation', 'step-07-project-type', 'step-08-scoping', 'step-09-functional', 'step-10-nonfunctional', 'step-11-polish', 'step-12-complete']
inputDocuments: []
workflowType: 'prd'
lastStep: 12
workflow_completed: true
project_name: 'palingenesis'
user_name: 'Jack'
date: '2026-02-05'
documentCounts:
  briefs: 0
  research: 0
  projectDocs: 0
classification:
  projectType: 'cli_tool + api_backend hybrid'
  domain: 'devtools'
  complexity: 'medium'
  projectContext: 'greenfield'
---

# Product Requirements Document - palingenesis

**Author:** Jack
**Date:** 2026-02-05

---

## Initial Vision Capture (Step 1)

### Product Name
**palingenesis** (Greek: "rebirth") — Agent resurrection system for continuous AI workflow execution

### Core Concept
A lightweight Rust daemon that monitors OpenCode process and sessions, automatically restarting OpenCode (`opencode serve`) when it crashes and resuming work when the agent stops due to rate limits, context limits, or other interruptions.

### Key Capabilities (Discovered)

1. **Monitors** OpenCode process and sessions for stop signals
2. **Restarts** OpenCode automatically via `opencode serve` when process dies
3. **Classifies** stop reason (rate limit vs context exhaustion vs completion vs crash)
4. **Waits** intelligently (respects `Retry-After`, polls quota endpoints)
5. **Resumes** work automatically via OpenCode HTTP API (same session or new session)
6. **Minimizes tokens** via step-file architecture
7. **Observes** via OpenTelemetry (traces, metrics, logs)
8. **Notifies** via external channels (webhook, Slack, Discord, Telegram, ntfy)
9. **Controlled** via external channels (pause/resume/skip/abort/status/config)
10. **MCP Server** interface — supports OpenCode MCP protocol for AI agent control

### Stop Reasons Identified

| Stop Reason | Frequency | Action |
|-------------|-----------|--------|
| Rate limits (API quotas) | HIGH | Wait for quota refresh, then resume same session |
| Context window exhausted | MEDIUM | Start new session from Next-step.md |
| **OpenCode process crash** | MEDIUM | Restart via `opencode serve`, then resume session |
| Session timeout | LOW | Resume same session |
| Network/infra issues | LOW | Retry with backoff |
| User exit | N/A | Respect, don't auto-resume |

### New Session Decision Criteria

| Criteria | New Session | Continue Same Session |
|----------|-------------|----------------------|
| Token count | Context >80% full | Context <80% full |
| Error type | `context_length_exceeded` | `rate_limit_error` |
| Task completion | `stepsCompleted` includes final step | `lastStep < total_steps` |
| State integrity | No valid frontmatter found | Valid `stepsCompleted` array exists |

### External Channel Control Commands

| Command | Action |
|---------|--------|
| `pause` | Stop monitoring, hold current state |
| `resume` | Continue monitoring & auto-resume |
| `skip` | Skip current step, move to next |
| `abort` | Kill current session entirely |
| `status` | Report current session state |
| `config` | Update runtime config |
| `new-session` | Force start fresh session from Next-step.md |

### Success Criteria

| # | Criterion | Measurable |
|---|-----------|------------|
| 1 | Detects agent stop within 5 seconds | Daemon log shows stop event |
| 2 | Correctly identifies stop reason | `rate_limit` / `context_exhausted` / `user_exit` |
| 3 | Rate limit: waits for quota refresh | Resumes automatically when 429 clears |
| 4 | Context exhausted: new session from `Next-step.md` | Continues from correct step |
| 5 | Notifies opencode to continue | `opencode resume` works |
| 6 | Lightweight: <10MB binary, <1% CPU idle | `htop` confirms |
| 7 | OTEL metrics visible in Grafana/Jaeger | Dashboard shows session lifecycle |
| 8 | External notification delivered <10s after event | Slack/Discord/webhook fires |
| 9 | Control command executes <5s after received | `/palin pause` -> daemon pauses |
| 10 | At least 2 control channels supported | CLI + one of (Slack/Discord/webhook) |

---

## Project Classification (Step 2)

| Dimension | Classification |
|-----------|----------------|
| **Project Type** | CLI tool + API backend hybrid |
| **Domain** | Developer Tools |
| **Complexity** | Medium |
| **Project Context** | Greenfield |

### Complexity Factors

- OpenTelemetry integration (OTEL SDK, exporters, semantic conventions)
- Bi-directional external channels (Slack/Discord bots, webhooks)
- Session state machine (rate limit vs context exhaustion detection)
- opencode integration (hooks, session API)

---

## Success Criteria (Step 3)

### User Success

**Emotional Win**: Trust — "I can finally trust my AI coding assistant to just work"

| Tier | Behavior | Metric |
|------|----------|--------|
| **Tier 1: Adoption** | Installs, works first try | <5 min to first auto-resume |
| **Tier 2: Integration** | Part of daily workflow | Added to dotfiles/startup |
| **Tier 3: Dependency** | Can't work without it | "Where has this been all my life?" |
| **Tier 4: Evangelism** | Recommends to colleagues | Unsolicited testimonials |

**Key Moment**: First unattended overnight run completes successfully
**Ultimate Success**: User forgets palingenesis is running

### Business Success

| Timeframe | Metric | Target |
|-----------|--------|--------|
| **3-Month** | Personal Use | Works for Jack daily |
| **3-Month** | GitHub Stars | 50-200 |
| **3-Month** | Active Users | 50-200 developers |
| **3-Month** | Testimonials | 5-10 |
| **12-Month** | Active Users | 500-2,000 |
| **12-Month** | GitHub Stars | 200-500 |
| **12-Month** | Contributors | 5-10 beyond Jack |
| **12-Month** | MRR (if monetized) | $500-$2K |

**Business Model Path**: Personal tool → Open source (MIT) → GitHub Sponsors → Optional freemium

### Technical Success

| Metric | Target | Verification |
|--------|--------|--------------|
| Stop detection latency | <5 seconds | Daemon logs |
| Binary size | <10MB | `ls -lh` |
| CPU usage (idle) | <1% | `htop` |
| Memory usage | <50MB | `htop` |
| Resume success rate | >95% | OTEL metrics |
| Notification latency | <10 seconds | End-to-end test |

### Measurable Outcomes

| Outcome | Measurement | Target |
|---------|-------------|--------|
| Interruptions prevented | Auto-resumes / week | 10+ |
| Time saved | Hours not spent babysitting | 5+ hrs/week |
| Flow state preservation | Uninterrupted work sessions | 80%+ |
| Trust level | Days since manual intervention | 7+ |

---

## Product Scope (Step 3)

### MVP - Minimum Viable Product

**Core Loop**: Monitor → Detect → Wait → Resume

| Feature | Description | Priority |
|---------|-------------|----------|
| Process monitoring | Watch opencode process for stop signals | P0 |
| Stop classification | Rate limit vs context exhaustion vs user exit | P0 |
| Intelligent waiting | Respect Retry-After headers, exponential backoff | P0 |
| Same-session resume | Continue existing session after rate limit | P0 |
| New-session resume | Start from Next-step.md after context exhaustion | P0 |
| CLI control | `palin status/pause/resume/abort` | P0 |
| Single binary | Cross-platform Rust binary <10MB | P0 |

### Growth Features (Post-MVP)

| Feature | Description | Priority |
|---------|-------------|----------|
| OpenTelemetry | Metrics, traces, logs export | P1 |
| Webhook notifications | HTTP POST on events | P1 |
| ntfy.sh integration | Lightweight push notifications | P1 |
| Config file | TOML/YAML configuration | P1 |
| Multi-assistant support | Cursor, Copilot, etc. | P1 |
| MCP Server interface | Daemon exposes MCP tools via stdio transport | P1 |
| OpenCode integration | Support `type: "local"` MCP configuration | P1 |

### Vision (Future)

| Feature | Description | Priority |
|---------|-------------|----------|
| Slack/Discord bots | Bi-directional control via chat | P2 |
| gRPC API | Programmatic control interface | P2 |
| Web dashboard | Visual monitoring UI | P2 |
| Multi-session orchestration | Manage multiple concurrent sessions | P2 |
| Team features | Shared configs, team notifications | P2 |

---

## User Journeys (Step 4)

### Journey 1: Jack's First Save (Primary User - Success Path)

**Persona**: Jack, developer using Claude Code daily

**The Story**: Jack is deep in a complex refactoring task at 11 PM—3 hours of accumulated AI context. Claude Code freezes with "rate_limit_error". His heart sinks. But he'd installed palingenesis earlier that day.

Two days later, same scenario. Claude Code hits rate limit at 2 AM during an overnight session. Jack wakes up to:

> ✅ **palingenesis**: Resumed. Session restored. 127 events processed while you slept.

**Emotional Arc**: Frustration → Skepticism → Cautious optimism → Relief → Trust → Evangelism

**Requirements Revealed**:
- Auto-detect running AI assistants
- Zero-config start
- Discord/Slack notifications
- Visible "first save" celebration
- Works overnight unattended

---

### Journey 2: The 3 AM Incident (Primary User - Edge Case)

**Scenario**: Context window exhausted (180K tokens), not just rate limit

**The Decision**: palingenesis detects `context_length_exceeded` and makes the right call:
- Backs up original session to `session-backup-20260205-0300.md`
- Starts new session from `Next-step.md`
- Continues from Step 7 of 12

**Morning Result**: Work completed. Steps 7-12 done overnight with clear audit trail.

**Requirements Revealed**:
- Distinguish rate limit vs context exhaustion
- Automatic new session creation from Next-step.md
- Session backup before new session
- Clear audit trail of decisions

---

### Journey 3: Sarah's Setup (Power User - Configuration)

**Persona**: Sarah, team lead wanting centralized monitoring

**The Setup**:
```bash
$ palingenesis config init
$ palingenesis config edit
```

```toml
[monitoring]
assistants = ["claude-code", "cursor", "copilot"]

[notifications]
discord_webhook = "https://discord.com/api/webhooks/..."

[otel]
endpoint = "http://grafana:4317"
```

**The Dashboard**: Grafana shows team-wide metrics—47 auto-resumes this week, 12.3 hours saved.

**Requirements Revealed**:
- TOML config file with documentation
- `palingenesis config validate` command
- OTEL metrics export
- Multi-assistant monitoring

---

### Journey 4: Remote Control (Slack/Discord Operator)

**Scenario**: Jack at dinner, phone buzzes with Discord alert

**The Interaction**:
```
/palin status
→ Claude Code: waiting (retry in 23s)

/palin logs --tail 5
→ [Shows recent events]
```

Jack lets auto-resume handle it. 30 minutes later: "Claude Code resumed."

**Requirements Revealed**:
- Push notifications with action buttons
- `/palin status` and `/palin logs` commands
- Mobile-friendly interface
- Remote restart capability

---

### Journey 5: Building Trust (Skeptic → Believer)

**The Trust Ladder**:

| Week | Check Frequency | Trust Level | Trigger |
|------|-----------------|-------------|---------|
| 1 | 5x/day | Skeptical | Anxiety |
| 2 | 2x/day | Hopeful | First save |
| 3 | 1x/day | Confident | Multiple saves |
| 4 | When alerted | Trusting | Overnight success |
| 8+ | Never | "Set and forget" | Consistent reliability |

**Weekly Summary Email**:
> This week: 7 auto-resumes, 4.2 hours saved, 100% success rate

**Ultimate Realization**: "I forgot palingenesis was running. That's how I know it's working."

**Requirements Revealed**:
- "Saves" counter visible
- Weekly summary email
- Success rate metrics
- Time saved calculation

---

### Journey Requirements Summary

| Capability | Journeys |
|------------|----------|
| Auto-detection | 1, 3 |
| Zero-config start | 1, 5 |
| Stop classification | 1, 2 |
| Same-session resume | 1, 4 |
| New-session resume | 2 |
| State persistence | 2 |
| Config system | 3 |
| OTEL metrics | 3, 5 |
| Discord/Slack notifications | 1, 4 |
| Remote commands | 4 |
| Trust metrics | 5 |

---

## Innovation & Novel Patterns (Step 6)

### Detected Innovation Areas

1. **Market Gap**: No tool currently monitors AI coding assistants and auto-resumes them
   - task-orchestrator: Memory persistence, but requires manual session management
   - Continuity: Synthetic memory, but no auto-resume ($15-399)
   - **palingenesis fills this gap with automatic, zero-config operation**

2. **Novel Architecture**: Session state machine with intelligent routing
   - Rate limit → Wait → Resume same session
   - Context exhausted → New session from Next-step.md
   - **No other tool makes this decision automatically**

3. **Trust-Building Pattern**: "Set and forget" for AI workflows
   - Progressive autonomy (Week 1: supervised → Week 8: forgotten)
   - **First tool designed for unattended AI workflow execution**

### Competitive Landscape

| Tool | What It Does | Gap Filled by palingenesis |
|------|--------------|---------------------------|
| task-orchestrator | MCP server for persistent memory | No auto-resume |
| Continuity | Synthetic memory for AI assistants | No auto-resume, costs $15-399 |
| LangChain/Temporal | Workflow orchestration | Developer frameworks, not end-user tools |

### Validation Approach

| Innovation | Validation Method |
|------------|-------------------|
| Auto-resume works | Jack uses it daily for 2 weeks |
| Context routing correct | 95%+ correct decisions (rate limit vs context) |
| Unattended execution | First overnight task completes successfully |
| Market demand | 50+ GitHub stars in 3 months |

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Wrong resumption decision | Always backup session before new session |
| opencode API changes | Loose coupling, file-based state detection |
| Rate limit detection fails | Fallback to exponential backoff |

---

## CLI Tool Specific Requirements (Step 7)

### Command Structure

```
palingenesis
├── daemon
│   ├── start [--debug] [--foreground]
│   ├── stop
│   ├── restart
│   ├── reload          # SIGHUP - reload config
│   └── status
├── status              # Quick status check
├── health              # Detailed health check
├── logs [--follow] [--tail N] [--since TIME]
├── config
│   ├── init
│   ├── show
│   ├── edit
│   ├── validate
│   └── set KEY VALUE
├── monitors
│   ├── list
│   ├── add NAME
│   └── remove NAME
├── events [--since TIME] [--filter TYPE]
├── inspect [--json]
├── attach              # Live event stream
├── metrics export [--format prometheus|json]
└── mcp
    └── serve           # Start MCP server mode (stdio transport)
```

### Output Formats

| Format | Use Case | Flag |
|--------|----------|------|
| Human-readable | Interactive terminal | (default) |
| JSON | Scripting, piping | `--json` |
| YAML | Config export | `--yaml` |
| Prometheus | Metrics scraping | `--format prometheus` |

### Configuration Schema

```toml
# ~/.config/palingenesis/config.toml

[daemon]
check_interval = "5s"
log_level = "info"               # debug, info, warn, error

[opencode]
auto_restart = true              # Restart OpenCode if it crashes
serve_port = 4096                # Port for `opencode serve`
serve_hostname = "127.0.0.1"     # Hostname for `opencode serve`
restart_delay_ms = 1000          # Delay before restart
health_check_interval = "5s"     # How often to check OpenCode health

[monitoring]
assistants = ["claude-code"]     # Auto-detect if empty
session_dir = "~/.opencode"      # Where to find session files

[resume]
rate_limit_strategy = "wait"     # wait, skip, new-session
context_strategy = "new-session" # new-session, abort
backup_before_new = true
max_retries = 3

[notifications]
discord_webhook = ""
slack_webhook = ""
ntfy_topic = ""
webhook_url = ""

[otel]
enabled = false
endpoint = "http://localhost:4317"
service_name = "palingenesis"
```

### Scripting Support

```bash
# Check if daemon is running
palingenesis status --json | jq -e '.running' > /dev/null

# Wait for daemon to be ready
palingenesis daemon start && palingenesis wait --timeout 30s

# Conditional restart
if palingenesis health | grep -q "degraded"; then
  palingenesis daemon restart
fi

# Export metrics for monitoring
palingenesis metrics export --format prometheus >> /var/lib/prometheus/palingenesis.prom
```

### Platform Support

| Platform | Daemon Method | Config Location |
|----------|---------------|-----------------|
| Linux | systemd user service | `~/.config/palingenesis/` |
| macOS | launchd agent | `~/Library/Application Support/palingenesis/` |
| Windows | Windows Service (future) | `%APPDATA%\palingenesis\` |

---

## Functional Requirements (Step 9)

### Session Monitoring

- FR1: Daemon can detect when opencode process starts
- FR2: Daemon can detect when opencode process stops
- FR3: Daemon can detect when opencode session hits rate limit (HTTP 429)
- FR4: Daemon can detect when opencode session exhausts context window
- FR5: Daemon can detect when user explicitly exits session
- FR6: Daemon can read session state from markdown frontmatter
- FR7: Daemon can parse `stepsCompleted` array from session files

### Session Resumption

- FR8: Daemon can resume same session after rate limit clears
- FR9: Daemon can start new session from `Next-step.md` after context exhaustion
- FR10: Daemon can backup session file before starting new session
- FR11: Daemon can respect `Retry-After` headers when waiting
- FR12: Daemon can implement exponential backoff for retries
- FR13: Daemon can track resumption history for audit trail

### CLI Control

- FR14: User can start daemon via CLI (`palingenesis daemon start`)
- FR15: User can stop daemon via CLI (`palingenesis daemon stop`)
- FR16: User can check daemon status via CLI (`palingenesis status`)
- FR17: User can view daemon logs via CLI (`palingenesis logs`)
- FR18: User can pause monitoring via CLI (`palingenesis pause`)
- FR19: User can resume monitoring via CLI (`palingenesis resume`)
- FR20: User can force new session via CLI (`palingenesis new-session`)

### Configuration

- FR21: User can initialize config file via CLI (`palingenesis config init`)
- FR22: User can validate config file via CLI (`palingenesis config validate`)
- FR23: User can edit config file via CLI (`palingenesis config edit`)
- FR24: Daemon can reload config without restart (SIGHUP)
- FR25: Daemon can auto-detect AI assistants if not configured

### Notifications (Growth)

- FR26: Daemon can send webhook notifications on events
- FR27: Daemon can send Discord notifications on events
- FR28: Daemon can send Slack notifications on events
- FR29: Daemon can send ntfy.sh notifications on events
- FR30: User can configure notification channels via config file

### External Control (Growth)

- FR31: User can check status via Discord/Slack command
- FR32: User can pause daemon via Discord/Slack command
- FR33: User can resume daemon via Discord/Slack command
- FR34: User can view logs via Discord/Slack command

### Observability (Growth)

- FR35: Daemon can export metrics in Prometheus format
- FR36: Daemon can export traces via OTLP
- FR37: Daemon can export structured logs via OTLP
- FR38: User can view metrics dashboard in Grafana
- FR39: Daemon can calculate and report "time saved" metric
- FR40: Daemon can calculate and report "saves count" metric

### MCP Server Interface (Growth)

- FR41: Daemon supports MCP stdio transport interface
- FR42: MCP interface uses JSON-RPC 2.0 protocol
- FR43: Daemon exposes control functions as MCP tools (status, pause, resume, new-session, logs)
- FR44: Supports OpenCode `type: "local"` MCP configuration format

### OpenCode Process Management (Growth)

- FR45: Daemon detects OpenCode process crash/exit
- FR46: Daemon automatically restarts OpenCode via `opencode serve`
- FR47: Daemon manages sessions via OpenCode HTTP API (`/session/*` endpoints)
- FR48: User can configure OpenCode serve port/hostname via config file

---

## Non-Functional Requirements (Step 10)

### Performance

| Requirement | Target | Rationale |
|-------------|--------|-----------|
| Stop detection latency | <5 seconds | Must catch issues quickly |
| Resume execution time | <2 seconds | Minimize interruption |
| CLI command response | <500ms | Responsive UX |
| Memory usage (idle) | <50MB | Lightweight daemon |
| CPU usage (idle) | <1% | No impact on dev work |

### Reliability

| Requirement | Target | Rationale |
|-------------|--------|-----------|
| Resume success rate | >95% | Trust requires reliability |
| Daemon uptime | >99.9% | Always-on monitoring |
| Graceful degradation | Yes | Fallback behaviors on failure |
| State persistence | Survives restart | No lost context |

### Security

| Requirement | Implementation |
|-------------|----------------|
| No credential storage | Daemon doesn't store API keys |
| Secure webhook URLs | Config file permissions (600) |
| No network by default | External features opt-in |
| Audit logging | All actions logged |

### Compatibility

| Requirement | Target |
|-------------|--------|
| Rust version | 1.75+ (stable) |
| Linux support | Ubuntu 20.04+, Fedora 38+ |
| macOS support | 12.0+ (Monterey) |
| Binary size | <10MB |

### Maintainability

| Requirement | Approach |
|-------------|----------|
| Code coverage | >80% for core logic |
| Documentation | README, man pages, `--help` |
| Release cadence | Semantic versioning |
| Backward compatibility | Config file versioning |

---

## Constraints & Assumptions

### Technical Constraints

1. **File-based integration**: palingenesis reads opencode session files, not API calls
2. **Single-user focus**: MVP targets individual developer, not team deployment
3. **opencode dependency**: Requires opencode to be running; doesn't work standalone
4. **Platform limitations**: Windows support deferred to post-MVP

### Assumptions

1. opencode session state is persisted in markdown files with YAML frontmatter
2. `stepsCompleted` array in frontmatter accurately reflects workflow progress
3. Rate limit errors include `Retry-After` headers or predictable patterns
4. Users have permission to monitor/restart opencode processes
5. Users want automatic resumption (can be disabled via config)

### Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| tokio | 1.x | Async runtime |
| clap | 4.x | CLI parsing |
| serde | 1.x | Config serialization |
| notify | 6.x | File system watching |
| reqwest | 0.11+ | HTTP for webhooks |
| tracing | 0.1+ | Logging/OTEL |
| opentelemetry | 0.21+ | Metrics/traces (optional) |

---

## Appendix: Research Summary

### Sources Consulted

- Anthropic API rate limit documentation
- OpenAI API rate limit documentation
- Rust CLI tool patterns (ripgrep, bat, fd)
- Daemon architecture patterns (systemd, launchd)
- Monitoring tool UX patterns (Datadog, Grafana, PagerDuty)
- Developer tool adoption research (Evil Martians, Stack Overflow 2025)

### Key Insights

1. **Rate limits are the #1 pain point** for AI coding assistant users
2. **Zero-config is essential** for developer tool adoption
3. **First save is the "aha moment"** that builds trust
4. **Trust takes 4-8 weeks** of consistent reliability to establish
5. **No direct competitor exists** for automatic AI session resumption

---

**PRD Complete** | Author: Jack | Date: 2026-02-05 | Version: 1.0
