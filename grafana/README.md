# Palingenesis Grafana Dashboard

Pre-built Grafana dashboard for visualizing palingenesis daemon metrics.

## Prerequisites

- Grafana 9.0+ (tested with 10.x)
- Prometheus data source configured in Grafana
- Palingenesis daemon running with metrics endpoint enabled

## Import Instructions

### Method 1: Import via Grafana UI

1. Open Grafana in your browser
2. Navigate to **Dashboards** > **Import** (or use the `+` icon > Import)
3. Click **Upload JSON file**
4. Select `palingenesis-dashboard.json` from this directory
5. Select your Prometheus data source when prompted
6. Click **Import**

### Method 2: Import via Grafana API

```bash
# Set your Grafana URL and API key
GRAFANA_URL="http://localhost:3000"
GRAFANA_API_KEY="your-api-key"

# Import the dashboard
curl -X POST \
  -H "Authorization: Bearer $GRAFANA_API_KEY" \
  -H "Content-Type: application/json" \
  -d @palingenesis-dashboard.json \
  "$GRAFANA_URL/api/dashboards/db"
```

### Method 3: Provision via Configuration

Add to your Grafana provisioning configuration:

```yaml
# /etc/grafana/provisioning/dashboards/palingenesis.yaml
apiVersion: 1
providers:
  - name: palingenesis
    folder: Monitoring
    type: file
    options:
      path: /path/to/palingenesis/grafana
```

## Dashboard Panels

### Overview Row
- **Total Saves**: Count of automatic session saves
- **Time Saved**: Cumulative time saved by automatic resumption (hours)
- **Daemon State**: Current state (Monitoring/Paused/Waiting/Resuming)
- **Uptime**: Daemon uptime duration
- **Resume Success Rate**: Percentage of successful resumes
- **Active Sessions**: Number of currently monitored sessions

### Resume Activity Row
- **Saves & Resumes Over Time**: Rate of saves and resumes
- **Resumes by Reason**: Breakdown by rate_limit, context_exhausted, manual

### Session Events Row
- **Rate Limits vs Context Exhaustions**: Event comparison over time
- **Retry Attempts**: Current retry attempt number (0 = not retrying)
- **Current Session Progress**: Percentage of steps completed
- **Resume Failures by Type**: Failures categorized by error type

### Performance Row
- **Resume Duration**: Histogram percentiles (p50, p95, p99)
- **Detection Latency**: Time from session stop to detection
- **Wait Duration**: Time spent waiting for rate limit backoff

## Metrics Reference

| Metric | Type | Description |
|--------|------|-------------|
| `palingenesis_saves_total` | Counter | Total automatic saves |
| `palingenesis_time_saved_seconds_total` | Counter | Total time saved |
| `palingenesis_daemon_state` | Gauge | Current state (1-4) |
| `palingenesis_uptime_seconds` | Gauge | Daemon uptime |
| `palingenesis_resumes_total{reason}` | Counter | Resumes by reason |
| `palingenesis_resumes_success_total` | Counter | Successful resumes |
| `palingenesis_resumes_failure_total{error_type}` | Counter | Failed resumes |
| `palingenesis_active_sessions` | Gauge | Active session count |
| `palingenesis_retry_attempts` | Gauge | Current retry attempt |
| `palingenesis_rate_limits_total` | Counter | Rate limit events |
| `palingenesis_context_exhaustions_total` | Counter | Context exhaustion events |
| `palingenesis_current_session_steps_completed` | Gauge | Steps completed |
| `palingenesis_current_session_steps_total` | Gauge | Total steps |
| `palingenesis_resume_duration_seconds` | Histogram | Resume operation duration |
| `palingenesis_detection_latency_seconds` | Histogram | Detection latency |
| `palingenesis_wait_duration_seconds` | Histogram | Wait/backoff duration |
| `palingenesis_time_saved_per_resume_seconds` | Histogram | Time saved per resume |

## Configuration

The dashboard uses a templated Prometheus data source. After import, ensure:

1. Your Prometheus scrapes the palingenesis metrics endpoint (default: `127.0.0.1:7654/metrics`)
2. The data source variable `${datasource}` points to your Prometheus instance

## Troubleshooting

**No data showing?**
- Verify palingenesis daemon is running: `palingenesis status`
- Check metrics endpoint: `curl http://127.0.0.1:7654/metrics`
- Verify Prometheus is scraping the endpoint

**Data source error?**
- Re-select the Prometheus data source in dashboard settings
- Check Prometheus data source configuration in Grafana
