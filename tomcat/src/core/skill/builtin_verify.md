---
name: verify
description: Run evidence-backed green-build acceptance before completing a code-changing plan.
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

## Green-build acceptance

Use this skill only after a plan's code review reports pass (or its runtime explicitly says review is skipped). Your job is to produce real evidence that the current code can run, not to describe what would be tested.

1. First identify the project and its documented acceptance commands. Use this order:
   - P0: an explicit command from the user or the approved plan;
   - P1: repository instructions such as `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, or the nearest README;
   - P2: the nearest project manifest and its scripts/tasks;
   - P3: workspace-level configuration or CI workflow;
   - P4: a narrowly scoped smoke command inferred from the changed code;
   - P5: if no runnable command can be found, explain that fact with the files inspected and run the smallest safe parse/type/build check available.

2. Select commands that directly cover the edited behavior: build/compile, focused tests, lint/typecheck when available, and existing project-specific UI smoke tests when UI code changed. Do not invent project tests or claim visual checks that do not exist.

3. Start every acceptance command with `bash(run_in_background=true)`. If the next step does not strictly depend on its result, do other independent work and wait for `<background-task-finished>`. If it does, call `task_output(block=true)` with a realistic wait slice until it finishes.

4. A command is valid evidence only when its background task is `Finished` with `exit_code=0`, and it started after the newest code edit. Failed, stopped, reused, or still-running tasks are not evidence.

5. When all selected checks pass, call `update_plan` with:

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
