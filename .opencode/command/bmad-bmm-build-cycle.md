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
1. Loading its own agent persona from `_bmad/{module}/agents/{agent}.md`
2. Executing the specified bmad workflow (e.g., `/bmad-bmm-sprint-planning`)
3. Making autonomous decisions during workflow execution
4. Updating `sprint-status.yaml` with new status
5. Committing changes upon completion
6. Return only: `DONE` or `FAILED: {reason}`

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
  prompt="Execute /bmad-bmm-sprint-planning. Autonomous mode. Commit when done."
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
  prompt="Execute /bmad-bmm-create-story. Autonomous mode. Commit when done."
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
  prompt="Execute /bmad-bmm-dev-story. Story: {story_path}. Autonomous mode. Commit when done."
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
  prompt="Execute /bmad-bmm-code-review. Story: {story_path}. Autonomous mode. Commit fixes if any."
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
  prompt="Execute /bmad-bmm-retrospective. Epic: {epic_num}. Autonomous mode. Commit when done."
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
  prompt="Execute /bmad-bmm-correct-course. Autonomous mode."
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
