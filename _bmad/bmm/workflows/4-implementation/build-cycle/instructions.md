# BMAD Build Cycle - Orchestration Instructions

## Overview

This workflow enables **orchestration-based implementation** of the BMAD Method Phase 4. The orchestrator (Sisyphus) delegates tasks to specialized agents rather than implementing directly.

## Agent Team

| Agent | Role | Specialization |
|-------|------|----------------|
| **SM** (Scrum Master) | Planning & Tracking | Sprint planning, story creation, retrospectives |
| **DEV** (Developer) | Implementation | Code writing, testing, story completion |
| **Oracle** | Consultation | Complex debugging, architecture decisions |
| **Explore** | Research | Codebase exploration, pattern discovery |
| **Librarian** | Documentation | External docs, library research |

## Build Cycle Phases

### Phase 1: Sprint Initialization

**Orchestrator Action**: Delegate to SM agent

```
delegate_task → SM → /bmad-bmm-sprint-planning
```

**Verification**:
- `sprint-status.yaml` created
- All epics/stories extracted and tracked
- Initial status set to `pending`

---

### Phase 2: Story Implementation Loop

Repeat for each story:

#### 2A: Story Creation

**Orchestrator Action**: Delegate to SM agent

```
delegate_task → SM → /bmad-bmm-create-story
```

**Verification**:
- Story file created with complete Dev Notes
- Tasks and acceptance criteria defined
- Architecture compliance notes included

#### 2B: Story Implementation

**Orchestrator Action**: Delegate to DEV agent

```
delegate_task → DEV → /bmad-bmm-dev-story
```

**Pre-delegation Exploration** (parallel):
```
delegate_task → explore → Find existing patterns for this feature
delegate_task → explore → Check related files that might be affected
delegate_task → librarian → Research any unfamiliar libraries
```

**Verification**:
- All tasks marked complete `[x]`
- Tests written and passing
- File List updated
- Story status = `review`

#### 2C: Code Review (Recommended)

**Orchestrator Action**: Delegate to DEV or Oracle

```
delegate_task → DEV → /bmad-bmm-code-review
```

**Verification**:
- 3-10 issues identified (minimum)
- Each issue has file:line reference
- Fixes applied or documented

---

### Phase 3: Epic Retrospective

**Orchestrator Action**: Delegate to SM agent

```
delegate_task → SM → /bmad-bmm-retrospective
```

**Verification**:
- All stories reviewed
- Lessons learned documented
- Patterns identified for future work

---

## Delegation Templates

### Sprint Planning Delegation

```
TASK: Initialize BMAD sprint planning
EXPECTED OUTCOME: sprint-status.yaml with all tracked items
REQUIRED TOOLS: Read, Write, Glob
MUST DO:
- Execute /bmad-bmm-sprint-planning workflow
- Extract ALL epics from _bmad-output/planning-artifacts/
- Create tracking file in _bmad-output/implementation-artifacts/
MUST NOT DO:
- Skip any stories
- Start implementation
```

### Story Creation Delegation

```
TASK: Create next story for implementation
EXPECTED OUTCOME: Complete story file ready for DEV agent
REQUIRED TOOLS: Read, Write, Glob
MUST DO:
- Execute /bmad-bmm-create-story workflow
- Generate comprehensive Dev Notes
- Include architecture patterns
- Reference previous story learnings
MUST NOT DO:
- Implement the story
- Create incomplete context
```

### Implementation Delegation

```
TASK: Implement story {story_path}
EXPECTED OUTCOME: Working code, passing tests, status='review'
REQUIRED TOOLS: Read, Write, Edit, Bash, LSP
MUST DO:
- Execute /bmad-bmm-dev-story workflow
- Complete ALL tasks in story
- Write tests for acceptance criteria
- Update story file (File List, Dev Agent Record)
- Pass Definition of Done checklist
MUST NOT DO:
- Skip tasks
- Leave failing tests
- Mark complete without DoD pass
CONTEXT: {story_path}
```

### Code Review Delegation

```
TASK: Adversarial review for {story_path}
EXPECTED OUTCOME: 3-10 documented issues
REQUIRED TOOLS: Read, Grep, LSP diagnostics
MUST DO:
- Execute /bmad-bmm-code-review workflow
- Find minimum 3 issues
- Check quality, tests, security, performance
- Provide file:line references
MUST NOT DO:
- Say 'looks good' without issues
- Skip security checks
```

### Retrospective Delegation

```
TASK: Epic {epic_num} retrospective
EXPECTED OUTCOME: Lessons learned, patterns documented
REQUIRED TOOLS: Read, Write
MUST DO:
- Execute /bmad-bmm-retrospective workflow
- Review all completed stories
- Extract what worked / what didn't
- Document for future epics
MUST NOT DO:
- Skip failed approaches
```

---

## State Management

### Tracking File: sprint-status.yaml

Location: `_bmad-output/implementation-artifacts/sprint-status.yaml`

States:
- `pending` - Not started
- `in-progress` - Currently being worked
- `review` - Implementation complete, awaiting review
- `done` - Reviewed and complete
- `blocked` - Cannot proceed

### State Transitions

```
pending → in-progress → review → done
                ↓
             blocked
```

---

## Error Handling

### Delegation Failed

```typescript
// Use session_id to continue
delegate_task(
  session_id="{failed_session_id}",
  prompt="Fix: {error_description}"
)
```

### Story Implementation Issues

1. Check DEV agent output for specific errors
2. Fire explore agents to investigate
3. Consult Oracle if architectural issue
4. Resume with session_id after fix

### Sprint Scope Change

```
delegate_task → SM → /bmad-bmm-correct-course
```

---

## Orchestration Checklist

Before starting:
- [ ] PRD.md exists
- [ ] Architecture.md exists
- [ ] Epics exist with stories

During execution:
- [ ] Sprint status initialized
- [ ] Each story: Created → Implemented → Reviewed
- [ ] Sprint status updated after each story
- [ ] Session IDs saved for continuations

After epic:
- [ ] All stories in 'done' status
- [ ] Retrospective completed
- [ ] Learnings documented

---

## Quick Commands

| Action | Command |
|--------|---------|
| Check status | `/bmad-bmm-sprint-status` |
| Handle change | `/bmad-bmm-correct-course` |
| Get help | `/bmad-help` |
