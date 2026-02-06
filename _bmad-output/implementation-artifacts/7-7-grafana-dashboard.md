# Story 7.7: Grafana Dashboard Template

## Story Overview

**Epic:** 7 - Observability & Metrics
**Story ID:** 7-7
**Title:** Grafana Dashboard Template
**Status:** done
**Priority:** Growth
**Story Points:** 1

## User Story

As an operator,
I want a pre-built Grafana dashboard,
So that I can visualize palingenesis metrics immediately.

## Background

This story provides a ready-to-import Grafana dashboard JSON file that visualizes all palingenesis metrics. The dashboard integrates with Prometheus as the data source and displays key operational metrics like saves count, resume success rate, time saved, and daemon state.

## Acceptance Criteria

### AC1: Dashboard JSON exists
**Given** the project repository
**When** an operator looks for the dashboard
**Then** `grafana/palingenesis-dashboard.json` exists

### AC2: Dashboard imports successfully
**Given** the dashboard JSON file
**When** imported into Grafana
**Then** it imports without errors
**And** displays all palingenesis metrics

### AC3: Dashboard panels cover key metrics
**Given** the imported dashboard
**When** viewed
**Then** panels include:
- Saves over time (counter graph)
- Resume success rate (percentage gauge)
- Time saved (counter/stat)
- Daemon state (state timeline)
- Active sessions
- Retry attempts
- Resume duration histogram

### AC4: Works with Prometheus data source
**Given** the dashboard
**When** configured with a Prometheus data source
**Then** all queries execute successfully
**And** metrics display correctly

## Technical Notes

### Metrics Available
From `src/telemetry/metrics.rs`:
- `palingenesis_info` - daemon version info
- `palingenesis_build_info` - build information
- `palingenesis_daemon_state` - current state (1=monitoring, 2=paused, 3=waiting, 4=resuming)
- `palingenesis_uptime_seconds` - daemon uptime
- `palingenesis_resumes_total{reason}` - resume count by reason
- `palingenesis_resumes_success_total` - successful resumes
- `palingenesis_resumes_failure_total{error_type}` - failed resumes
- `palingenesis_saves_total` - total saves count
- `palingenesis_sessions_started_total` - sessions started
- `palingenesis_rate_limits_total` - rate limit events
- `palingenesis_context_exhaustions_total` - context exhaustion events
- `palingenesis_current_session_steps_completed` - current session progress
- `palingenesis_current_session_steps_total` - total steps in session
- `palingenesis_active_sessions` - active session count
- `palingenesis_retry_attempts` - current retry attempts
- `palingenesis_resume_duration_seconds` - histogram of resume durations
- `palingenesis_detection_latency_seconds` - histogram of detection latency
- `palingenesis_wait_duration_seconds` - histogram of wait durations
- `palingenesis_time_saved_seconds_total` - total time saved
- `palingenesis_time_saved_per_resume_seconds` - histogram of time saved per resume

### Dashboard Structure
- Row 1: Overview (saves count, time saved, daemon state, uptime)
- Row 2: Resumes (success rate, resumes over time, failures by type)
- Row 3: Sessions (active sessions, steps progress, rate limits vs context exhaustions)
- Row 4: Performance (resume duration, detection latency, wait duration histograms)

## Definition of Done

- [x] `grafana/palingenesis-dashboard.json` created
- [x] Dashboard imports into Grafana without errors
- [x] All metrics panels configured
- [x] Prometheus queries are valid
- [x] README documents import process

---
*Generated: 2026-02-06*
*Epic Reference: _bmad-output/planning-artifacts/epics.md#story-77-grafana-dashboard-template*
