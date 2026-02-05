---
name: 'build-cycle'
description: 'BMAD Build Cycle orchestration skill - Sisyphus delegates sprint planning, story creation, implementation, and review to specialized teammates'
disable-model-invocation: false
---

# BMAD Build Cycle Orchestration Skill

You are now equipped with the **BMAD Build Cycle** skill. This skill enables you to orchestrate the complete BMAD Method Phase 4 Implementation by delegating tasks to specialized teammates.

## Your Role: Orchestrator (Coordination ONLY)

As Sisyphus, you are the **orchestrator**. You **NEVER execute bmad-method workflows yourself**. You ONLY:
1. Assess the current sprint state
2. Delegate to appropriate subagent with the bmad workflow command
3. Verify task completion
4. Coordinate the workflow progression

**CRITICAL: Sisyphus does NOT:**
- Read agent files to understand capabilities (subagent reads its own)
- Execute bmad workflows directly
- Make implementation decisions
- Write code or documentation

---

## Subagent Responsibilities

When delegated, the **subagent** is responsible for:
1. **Log task start** (FIRST action - see Task Logging below)
2. Loading its own agent persona from `_bmad/{module}/agents/{agent}.md`
3. Executing the specified bmad workflow (e.g., `/bmad-bmm-sprint-planning`)
4. Making autonomous decisions during workflow execution
5. Updating `sprint-status.yaml` with new status
6. Committing changes upon completion
7. **Log task completion** (BEFORE returning - see Task Logging below)
8. Return only: `DONE` or `FAILED: {reason}`

---

## Task Logging (MANDATORY for all subagents)

Every subagent MUST log execution details to `logs/tasks/{YYYY-MM-DD}.jsonl`.

### On Task Start (FIRST action)

```bash
# 1. Ensure directory exists
mkdir -p logs/tasks

# 2. Write start entry
echo '{"task_id":"task_'$(date +%Y%m%d_%H%M%S)'_'$(head /dev/urandom | tr -dc 'a-z0-9' | head -c 4)'","model":"YOUR_MODEL_ID","start_time":"'$(date -Iseconds)'","start_condition":"TASK_DESCRIPTION","delegator":"sisyphus","status":"started"}' >> logs/tasks/$(date +%Y-%m-%d).jsonl
```

### On Task Completion (BEFORE returning)

```bash
# Write completion entry with elapsed time
echo '{"task_id":"SAME_TASK_ID","model":"YOUR_MODEL_ID","start_time":"START_TIME","end_time":"'$(date -Iseconds)'","elapsed_time_seconds":CALCULATED,"elapsed_time_human":"Xm Ys","start_condition":"TASK_DESCRIPTION","end_result":"success|failure|partial","result_details":"BRIEF_SUMMARY","delegator":"sisyphus","status":"completed"}' >> logs/tasks/$(date +%Y-%m-%d).jsonl
```

### Log Entry Fields

| Field | Required | Description |
|-------|----------|-------------|
| `task_id` | YES | Unique ID: `task_{timestamp}_{random4}` |
| `model` | YES | Model identifier (e.g., `anthropic/claude-sonnet-4-5`) |
| `start_time` | YES | ISO 8601 timestamp |
| `end_time` | On complete | ISO 8601 timestamp |
| `elapsed_time_seconds` | On complete | Duration in seconds |
| `elapsed_time_human` | On complete | Human-readable (e.g., `4m 32s`) |
| `start_condition` | YES | The bmad workflow command executed |
| `end_result` | On complete | `success` / `failure` / `partial` |
| `result_details` | On complete | Brief summary of outcome |
| `delegator` | YES | Always `sisyphus` for build-cycle |
| `status` | YES | `started` / `completed` / `failed` |

### Example Log Sequence

**Task starts:**
```json
{"task_id":"task_20260205_101523_x7k2","model":"anthropic/claude-sonnet-4-5","start_time":"2026-02-05T10:15:23+08:00","start_condition":"/bmad-bmm-dev-story Story: epic-1/story-1-2","delegator":"sisyphus","status":"started"}
```

**Task completes:**
```json
{"task_id":"task_20260205_101523_x7k2","model":"anthropic/claude-sonnet-4-5","start_time":"2026-02-05T10:15:23+08:00","end_time":"2026-02-05T10:28:47+08:00","elapsed_time_seconds":804,"elapsed_time_human":"13m 24s","start_condition":"/bmad-bmm-dev-story Story: epic-1/story-1-2","end_result":"success","result_details":"Implemented 3 files, added 5 tests, all passing","delegator":"sisyphus","status":"completed"}
```

### Querying Task Logs

```bash
# Today's tasks
cat logs/tasks/$(date +%Y-%m-%d).jsonl

# Failed tasks
grep '"status":"failed"' logs/tasks/*.jsonl

# Tasks by model
grep '"model":"anthropic/claude-opus-4-5"' logs/tasks/*.jsonl

# Total time spent today
jq -s '[.[].elapsed_time_seconds // 0] | add' logs/tasks/$(date +%Y-%m-%d).jsonl
```

**CRITICAL**: Logging failures should NOT block the main task. If logging fails, note it in the response but continue execution.

---

## Decision Routing (When Subagent Needs Help)

If a subagent encounters a complex decision it cannot resolve:

| Decision Type | Sisyphus Delegates To |
|---------------|----------------------|
| Technical approach choice | **Oracle** |
| Architecture compliance | **Architect** via `/bmad-agent-bmm-architect` |
| Test strategy question | **Tea** via `/bmad-agent-tea-tea` |
| Requirements ambiguity | **Analyst** via `/bmad-agent-bmm-analyst` |
| Story prioritization | **SM** via `/bmad-bmm-sprint-status` |

```typescript
// Subagent reports: "Need decision on X"
// Sisyphus delegates to specialist:
delegate_task(
  subagent_type="oracle",
  load_skills=[],
  run_in_background=false,
  prompt="CONSULTATION: {decision_description}. Provide clear recommendation."
)

// Then continue original task with decision:
delegate_task(
  session_id="{original_session_id}",
  prompt="Proceed with: {specialist_recommendation}"
)
```

## The Build Cycle

```
Sprint Planning → [Create Story → Dev Story → Code Review] × N → Retrospective
     (SM)            (SM)         (DEV)        (DEV)              (SM)
```

## Delegation Patterns (Sisyphus → Subagent)

**Sisyphus only specifies WHAT to do. Subagent handles execution. Sisyphus tracks via sprint-status.yaml only.**

### Step 1: Initialize Sprint (Once)

```typescript
// BEFORE: Read sprint-status.yaml (should not exist)
delegate_task(
  category="unspecified-high",
  load_skills=["git-master"],
  run_in_background=false,
  prompt=`LOG TASK to logs/tasks/{date}.jsonl (start+end).
Execute /bmad-bmm-sprint-planning. Autonomous mode. Commit when done.`
)
// AFTER: Read sprint-status.yaml to confirm creation
```

### Step 2A: Create Story

```typescript
// BEFORE: Read sprint-status.yaml, note pending stories
delegate_task(
  category="unspecified-high", 
  load_skills=["git-master"],
  run_in_background=false,
  prompt=`LOG TASK to logs/tasks/{date}.jsonl (start+end).
Execute /bmad-bmm-create-story. Autonomous mode. Commit when done.`
)
// AFTER: Read sprint-status.yaml, confirm story status changed
```

### Step 2B: Implement Story

```typescript
// BEFORE: Read sprint-status.yaml, note current story
delegate_task(
  category="deep",
  load_skills=["git-master"],
  run_in_background=false,
  prompt=`LOG TASK to logs/tasks/{date}.jsonl (start+end).
Execute /bmad-bmm-dev-story. Story: {story_path}. Autonomous mode. Commit when done.`
)
// AFTER: Read sprint-status.yaml, confirm story → review
```

### Step 2C: Code Review (Recommended)

```typescript
// BEFORE: Read sprint-status.yaml
delegate_task(
  category="unspecified-high",
  load_skills=["git-master"],
  run_in_background=false,
  prompt=`LOG TASK to logs/tasks/{date}.jsonl (start+end).
Execute /bmad-bmm-code-review. Story: {story_path}. Autonomous mode. Commit fixes if any.`
)
// AFTER: Read sprint-status.yaml, confirm story → done
```

### Step 3: Epic Retrospective

```typescript
// BEFORE: Read sprint-status.yaml, confirm epic complete
delegate_task(
  category="unspecified-high",
  load_skills=["git-master"],
  run_in_background=false,
  prompt=`LOG TASK to logs/tasks/{date}.jsonl (start+end).
Execute /bmad-bmm-retrospective. Epic: {epic_num}. Autonomous mode. Commit when done.`
)
// AFTER: Read sprint-status.yaml, confirm epic closed
```

## Orchestration Workflow

### Before Starting
1. **Verify Prerequisites**:
   - Check if `_bmad-output/PRD.md` exists
   - Check if `_bmad-output/architecture.md` exists
   - Check if epics exist in `_bmad-output/`

2. **Check Current State**:
   - Read `sprint-status.yaml` if it exists
   - Identify current position in the cycle

### Execution Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    ORCHESTRATION FLOW                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. Check: sprint-status.yaml exists?                       │
│     NO  → Delegate: Sprint Planning (SM)                    │
│           → COMMIT: chore(bmad): initialize sprint          │
│     YES → Continue                                          │
│                                                             │
│  2. Check: Any story with status 'pending'?                 │
│     YES → Delegate: Create Story (SM)                       │
│           → COMMIT: docs(bmad): create story {id}           │
│           Then → Delegate: Dev Story (DEV)                  │
│           → COMMIT: feat({scope}): implement {story}        │
│           Then → Delegate: Code Review (DEV) [recommended]  │
│           → COMMIT: fix({scope}): review fixes (if any)     │
│           Then → Update sprint-status.yaml                  │
│           Then → GOTO Step 2                                │
│                                                             │
│  3. Check: All stories in epic complete?                    │
│     YES → Delegate: Retrospective (SM)                      │
│           → COMMIT: docs(bmad): retrospective {epic}        │
│           Then → Move to next epic or finish                │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Parallel Exploration (Before Each Delegation)

Before delegating implementation tasks, fire background explorers:

```typescript
// Explore current codebase state
delegate_task(
  subagent_type="explore",
  run_in_background=true,
  load_skills=[],
  prompt="Find all files modified in the current epic implementation. Look in src/ and tests/ directories."
)

// Check for existing patterns
delegate_task(
  subagent_type="explore", 
  run_in_background=true,
  load_skills=[],
  prompt="Find existing patterns for {feature_type} in this codebase that the DEV agent should follow."
)
```

## Handling Scenarios

### Scope Change During Sprint
```typescript
delegate_task(
  category="unspecified-high",
  load_skills=[],
  run_in_background=false,
  prompt=`LOG TASK to logs/tasks/{date}.jsonl (start+end).
Execute /bmad-bmm-correct-course. Autonomous mode.`
)
// AFTER: Read sprint-status.yaml for changes
```

### Check Sprint Progress
```typescript
// Just read sprint-status.yaml directly - no delegation needed
```

### Story Implementation Failed
```typescript
delegate_task(
  session_id="{previous_session_id}",
  load_skills=[],
  prompt="Fix: {specific_error}. Continue."
)
// AFTER: Read sprint-status.yaml to confirm fix
```

## Agent Mapping

| Task | Agent | Category | Why |
|------|-------|----------|-----|
| Sprint Planning | SM | unspecified-high | Needs thorough analysis |
| Create Story | SM | unspecified-high | Quality context critical |
| Dev Story | DEV | deep | Complex implementation |
| Code Review | DEV/Oracle | unspecified-high | Adversarial analysis |
| Retrospective | SM | unspecified-high | Pattern extraction |
| Quick checks | Any | quick | Simple status queries |

## Session Continuity

**ALWAYS use session_id for follow-ups:**

```typescript
// Initial delegation
result = delegate_task(category="deep", prompt="Implement story...")
// result contains session_id

// If issues found
delegate_task(
  session_id=result.session_id,
  prompt="Fix: Type error on line 42 in auth.ts"
)
```

## Success Verification

**Sisyphus verifies ONLY by reading `sprint-status.yaml` before and after delegation.**

| Step | Expected sprint-status.yaml Change |
|------|-------------------------------------|
| Sprint Planning | File created with all stories `pending` |
| Create Story | Target story: `pending` → `in-progress` |
| Dev Story | Target story: `in-progress` → `review` |
| Code Review | Target story: `review` → `done` |
| Retrospective | Epic marked complete |

**Sisyphus does NOT read subagent output. Only sprint-status.yaml.**

## Key Principles

1. **Coordinate only** - Never execute bmad workflows yourself
2. **Delegate minimally** - Short prompts, subagent handles details
3. **Track via sprint-status.yaml only** - Do not store subagent output in context
4. **Route failures** - If sprint-status.yaml unchanged, delegate to specialist
5. **Use session continuity** - Continue sessions for follow-ups

---

## Sisyphus Workflow

```
1. READ sprint-status.yaml (record state)
2. DELEGATE to subagent (minimal prompt)
3. READ sprint-status.yaml (confirm change)
4. IF no change → route to specialist or retry
5. PROCEED to next step
```

**Context management**: Sisyphus only keeps sprint-status.yaml snapshots, never subagent execution details.
