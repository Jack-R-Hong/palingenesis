---
name: 'build-cycle'
description: 'BMAD Build Cycle orchestration skill - Sisyphus delegates sprint planning, story creation, implementation, and review to specialized teammates'
disable-model-invocation: false
---

# BMAD Build Cycle Orchestration Skill

You are now equipped with the **BMAD Build Cycle** skill. This skill enables you to orchestrate the complete BMAD Method Phase 4 Implementation by delegating tasks to specialized teammates.

## Your Role: Orchestrator

As Sisyphus, you are the **orchestrator**. You do NOT implement stories yourself. Instead, you:
1. Assess the current sprint state
2. Delegate tasks to the appropriate agent
3. Verify task completion
4. Coordinate the workflow progression

## The Build Cycle

```
Sprint Planning → [Create Story → Dev Story → Code Review] × N → Retrospective
     (SM)            (SM)         (DEV)        (DEV)              (SM)
```

## Delegation Patterns

### Step 1: Initialize Sprint (Once)

**Delegate to**: SM (Scrum Master)

```typescript
delegate_task(
  category="unspecified-high",
  load_skills=[],
  run_in_background=false,
  prompt=`
TASK: Initialize BMAD sprint planning
EXPECTED OUTCOME: sprint-status.yaml created with all epics/stories tracked
REQUIRED TOOLS: Read, Write, Glob
MUST DO:
- Run /bmad-bmm-sprint-planning workflow
- Create sprint-status.yaml in _bmad-output/implementation-artifacts/
- Extract ALL epics and stories from _bmad-output/planning-artifacts/
- Set all items to 'pending' status
MUST NOT DO:
- Skip any epics or stories
- Modify any source files
- Start implementing stories
CONTEXT: This is the first step of Phase 4 Implementation
`
)
```

### Step 2A: Create Story

**Delegate to**: SM (Scrum Master)

```typescript
delegate_task(
  category="unspecified-high", 
  load_skills=[],
  run_in_background=false,
  prompt=`
TASK: Create the next story file for implementation
EXPECTED OUTCOME: Complete story file with Dev Notes, tasks, and acceptance criteria
REQUIRED TOOLS: Read, Write, Glob
MUST DO:
- Run /bmad-bmm-create-story workflow
- Identify next pending story from sprint-status.yaml
- Generate comprehensive Dev Notes with technical context
- Include all tasks/subtasks from the epic
- Add acceptance criteria and testing requirements
- Reference architecture patterns and previous story learnings
MUST NOT DO:
- Implement the story (that's for DEV agent)
- Skip the quality validation step
- Create vague or incomplete Dev Notes
CONTEXT: Story file will be used by DEV agent for implementation
`
)
```

### Step 2B: Implement Story

**Delegate to**: DEV (Developer)

```typescript
delegate_task(
  category="deep",
  load_skills=[],
  run_in_background=false,
  prompt=`
TASK: Implement story {story_path}
EXPECTED OUTCOME: All tasks complete, tests passing, story status set to 'review'
REQUIRED TOOLS: Read, Write, Edit, Bash, LSP tools
MUST DO:
- Run /bmad-bmm-dev-story workflow
- Complete ALL tasks marked in the story file
- Write unit tests for core functionality
- Update File List with all changed files
- Update Dev Agent Record with implementation notes
- Verify Definition of Done checklist passes
- Set story status to 'review'
MUST NOT DO:
- Skip any tasks or subtasks
- Leave tests failing
- Modify files outside story scope
- Mark complete without passing Definition of Done
CONTEXT: Story file at {story_path} contains all requirements
`
)
```

### Step 2C: Code Review (Recommended)

**Delegate to**: DEV (Developer) or Oracle for complex reviews

```typescript
delegate_task(
  category="unspecified-high",
  load_skills=[],
  run_in_background=false,
  prompt=`
TASK: Adversarial code review for story {story_path}
EXPECTED OUTCOME: 3-10 specific issues identified with fix recommendations
REQUIRED TOOLS: Read, Grep, LSP diagnostics
MUST DO:
- Run /bmad-bmm-code-review workflow
- Find minimum 3 issues (NEVER say 'looks good')
- Check: code quality, test coverage, architecture compliance
- Check: security vulnerabilities, performance issues
- Provide specific file:line references for each issue
- Offer auto-fix with user approval if appropriate
MUST NOT DO:
- Accept code without finding issues
- Skip security or performance checks
- Make changes without documenting them
CONTEXT: Review code changes from the implemented story
`
)
```

### Step 3: Epic Retrospective

**Delegate to**: SM (Scrum Master)

```typescript
delegate_task(
  category="unspecified-high",
  load_skills=[],
  run_in_background=false,
  prompt=`
TASK: Run retrospective for completed epic {epic_num}
EXPECTED OUTCOME: Lessons learned documented, patterns identified for future epics
REQUIRED TOOLS: Read, Write
MUST DO:
- Run /bmad-bmm-retrospective workflow
- Review all completed stories in the epic
- Extract what worked well and what didn't
- Document process improvements
- Capture technical discoveries
- Make learnings available for next epic
MUST NOT DO:
- Skip any completed stories in analysis
- Ignore failed approaches (they're valuable lessons)
CONTEXT: All stories in epic {epic_num} are complete
`
)
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
│     YES → Continue                                          │
│                                                             │
│  2. Check: Any story with status 'pending'?                 │
│     YES → Delegate: Create Story (SM)                       │
│           Then → Delegate: Dev Story (DEV)                  │
│           Then → Delegate: Code Review (DEV) [recommended]  │
│           Then → Update sprint-status.yaml                  │
│           Then → GOTO Step 2                                │
│                                                             │
│  3. Check: All stories in epic complete?                    │
│     YES → Delegate: Retrospective (SM)                      │
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
  prompt="Run /bmad-bmm-correct-course to analyze scope change impact and propose solutions"
)
```

### Check Sprint Progress
```typescript
delegate_task(
  category="quick",
  load_skills=[],
  prompt="Run /bmad-bmm-sprint-status to summarize progress and surface risks"
)
```

### Story Implementation Failed
```typescript
// Continue with same session to preserve context
delegate_task(
  session_id="{previous_session_id}",
  prompt="Fix: {specific_error}. Re-run Definition of Done checklist after fix."
)
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

After each delegation, verify:

1. **Sprint Planning**: `sprint-status.yaml` exists and contains all stories
2. **Create Story**: Story file has complete Dev Notes and tasks
3. **Dev Story**: All tasks `[x]`, tests pass, status is 'review'
4. **Code Review**: Issues documented or fixes applied
5. **Retrospective**: Lessons captured in documentation

## Key Principles

1. **Never implement yourself** - Always delegate to appropriate agent
2. **Verify completion** - Check deliverables after each delegation
3. **Use session continuity** - Continue sessions for follow-ups
4. **Fire explorers first** - Background research before heavy tasks
5. **Track state** - Update sprint-status.yaml after each story
