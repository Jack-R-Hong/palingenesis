---
name: 'task-logger'
description: 'Log subagent task execution details including model, timing, conditions, and results to logs/ folder.'
disable-model-invocation: true
---

# Task Logger Skill

This skill instructs subagents to log their task execution details for observability and debugging purposes.

---

## MANDATORY: Task Execution Logging Protocol

When this skill is loaded, you MUST follow this logging protocol for EVERY task execution.

---

## Phase 1: Task Start (IMMEDIATELY on task begin)

### 1.1 Capture Start Information

```
CAPTURE IMMEDIATELY:
- task_id: Generate unique ID (format: task_{timestamp}_{random4})
- model: Your model identifier (from system info)
- start_time: Current timestamp in ISO 8601 format
- start_condition: The prompt/instruction that triggered this task
- delegator: Who delegated this task (if applicable)
```

### 1.2 Create Log Entry

**At task START, write initial log entry to `logs/tasks/{YYYY-MM-DD}.jsonl`:**

```json
{
  "task_id": "task_20260205_a1b2",
  "model": "anthropic/claude-sonnet-4-5",
  "start_time": "2026-02-05T08:51:37+08:00",
  "start_condition": "Implement feature X with specifications Y",
  "delegator": "sisyphus",
  "status": "started"
}
```

**File path pattern:** `logs/tasks/{YYYY-MM-DD}.jsonl`
- Example: `logs/tasks/2026-02-05.jsonl`
- One line per log entry (JSON Lines format)
- Append to existing file if it exists

---

## Phase 2: Task Completion (BEFORE returning result)

### 2.1 Capture End Information

```
CAPTURE BEFORE RETURNING:
- end_time: Current timestamp in ISO 8601 format
- elapsed_time: Calculate from start_time to end_time
- end_result: Summary of what was accomplished (success/failure/partial)
- result_details: Brief description of outputs or changes made
- error_details: If failed, capture error information
```

### 2.2 Write Completion Log Entry

**At task END, append completion entry to same log file:**

```json
{
  "task_id": "task_20260205_a1b2",
  "model": "anthropic/claude-sonnet-4-5",
  "start_time": "2026-02-05T08:51:37+08:00",
  "end_time": "2026-02-05T08:55:42+08:00",
  "elapsed_time_seconds": 245,
  "elapsed_time_human": "4m 5s",
  "start_condition": "Implement feature X with specifications Y",
  "end_result": "success",
  "result_details": "Created 3 files, modified 2 files, all tests passing",
  "delegator": "sisyphus",
  "status": "completed"
}
```

---

## Log File Structure

```
logs/
  tasks/
    2026-02-05.jsonl    # Daily task logs (JSON Lines format)
    2026-02-04.jsonl
    ...
```

### JSON Lines Format (JSONL)

Each line is a complete JSON object. This allows:
- Easy appending without parsing entire file
- Line-by-line streaming reads
- Simple grep/filtering operations

---

## Field Specifications

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `task_id` | string | YES | Unique identifier for this task execution |
| `model` | string | YES | Model identifier (e.g., `anthropic/claude-sonnet-4-5`) |
| `start_time` | ISO 8601 | YES | When task began |
| `end_time` | ISO 8601 | On complete | When task finished |
| `elapsed_time_seconds` | number | On complete | Duration in seconds |
| `elapsed_time_human` | string | On complete | Human-readable duration |
| `start_condition` | string | YES | The prompt/instruction that triggered the task |
| `end_result` | enum | On complete | `success` / `failure` / `partial` / `cancelled` |
| `result_details` | string | On complete | Brief summary of what was done |
| `error_details` | string | On failure | Error message or failure reason |
| `delegator` | string | If known | Who delegated this task |
| `status` | enum | YES | `started` / `completed` / `failed` / `cancelled` |

---

## Implementation Instructions

### On Task Start

```bash
# 1. Ensure logs directory exists
mkdir -p logs/tasks

# 2. Generate task_id
TASK_ID="task_$(date +%Y%m%d_%H%M%S)_$(head /dev/urandom | tr -dc 'a-z0-9' | head -c 4)"

# 3. Capture start_time
START_TIME=$(date -Iseconds)

# 4. Write start entry
echo '{"task_id":"'$TASK_ID'","model":"YOUR_MODEL","start_time":"'$START_TIME'","start_condition":"...","status":"started"}' >> logs/tasks/$(date +%Y-%m-%d).jsonl
```

### On Task End

```bash
# 1. Capture end_time
END_TIME=$(date -Iseconds)

# 2. Calculate elapsed time (in bash or programmatically)
# 3. Write completion entry with all fields
```

---

## Example: Complete Task Log Sequence

**Task starts:**
```json
{"task_id":"task_20260205_085137_x7k2","model":"anthropic/claude-sonnet-4-5","start_time":"2026-02-05T08:51:37+08:00","start_condition":"Fix authentication bug in auth.rs","delegator":"sisyphus","status":"started"}
```

**Task completes:**
```json
{"task_id":"task_20260205_085137_x7k2","model":"anthropic/claude-sonnet-4-5","start_time":"2026-02-05T08:51:37+08:00","end_time":"2026-02-05T08:55:42+08:00","elapsed_time_seconds":245,"elapsed_time_human":"4m 5s","start_condition":"Fix authentication bug in auth.rs","end_result":"success","result_details":"Fixed token validation logic in src/auth.rs, added 2 unit tests","delegator":"sisyphus","status":"completed"}
```

---

## Error Handling

### If log write fails:
1. DO NOT let logging failure block the actual task
2. Report logging failure in your response
3. Continue with the main task

### If time capture fails:
1. Use "unknown" for time fields
2. Note the issue in result_details

---

## Querying Logs

**Find all tasks for today:**
```bash
cat logs/tasks/$(date +%Y-%m-%d).jsonl
```

**Find failed tasks:**
```bash
grep '"status":"failed"' logs/tasks/*.jsonl
```

**Find tasks by model:**
```bash
grep '"model":"anthropic/claude-opus-4-5"' logs/tasks/*.jsonl
```

**Calculate total time spent:**
```bash
jq -s '[.[].elapsed_time_seconds // 0] | add' logs/tasks/2026-02-05.jsonl
```

---

## CRITICAL REMINDERS

1. **ALWAYS** create the start log entry BEFORE doing any work
2. **ALWAYS** create the completion log entry BEFORE returning results
3. **NEVER** let logging failures block the main task
4. **USE** the exact file path pattern: `logs/tasks/{YYYY-MM-DD}.jsonl`
5. **INCLUDE** your actual model identifier in the `model` field
