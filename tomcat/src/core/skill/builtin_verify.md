---
name: verify
description: Discover and run this project's build/test/lint verification commands (P0–P5 discovery), scaled to the change, and report real green-build evidence.
allowed-tools:
  - read
  - search_files
  - list_dir
  - bash
  - task_output
  - task_list
  - task_stop
  - update_plan
---

## Green-build verification

Produce real evidence that the current code builds and passes its checks — do not
describe what would be tested.

1. First identify the project and its documented acceptance commands. Use this order:
   - P0: an explicit command from the user or the approved plan;
   - P1: repository instructions such as `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, or the nearest README;
   - P2: the nearest project manifest and its scripts/tasks;
   - P3: workspace-level configuration or CI workflow;
   - P4: a narrowly scoped smoke command inferred from the changed code;
   - P5: if no runnable command can be found, explain that fact with the files inspected and run the smallest safe parse/type/build check available.

2. Scale the checks to the change:
   - Default: the project's full check set (format / lint / full tests / build,
     each discovered per project).
   - May narrow: a small, isolated change (for example a single-module bugfix) →
     that package's / module's tests + lint are sufficient evidence.
   - Must NOT narrow: a cross-module refactor, a core path, or a dependency change →
     run the full check set.

3. Select commands that directly cover the edited behavior: build/compile, focused tests, lint/typecheck when available, and existing project-specific UI smoke tests when UI code changed. Do not invent project tests or claim visual checks that do not exist.

4. Start every acceptance command with `bash(run_in_background=true)`. If the next step does not strictly depend on its result, do other independent work and wait for `<background-task-finished>`. If it does, call `task_output(block=true)` with a realistic wait slice until it finishes.

5. A command is valid evidence only when its background task is `Finished` with `exit_code=0`, and it started after the newest code edit. Failed, stopped, reused, or still-running tasks are not evidence.

6. When all selected checks pass, call `update_plan` with:

```json
{
  "green_build_pass": true,
  "green_build_evidence": [{
    "command": "<the exact background bash command>",
    "task_id": "<finished-background-task-id>"
  }],
  "ops": []
}
```

The runtime validates the task ID, actual command, exit code, and freshness itself. Never set `green_build_pass` to true without this evidence. If discovery found no meaningful command, leave the plan open and report the concrete missing acceptance path rather than fabricating success.
