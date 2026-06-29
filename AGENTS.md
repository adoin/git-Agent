# Project Rules

## Long-Running State Transitions

Any action that may take noticeable time, touch the working tree/index, change HEAD, use network,
spawn an external tool, or reload repository state must follow the same transition pattern:

1. Move the UI into the target action space immediately.
   - Show the selected target as pending/current when the target is known.
   - Keep the previous snapshot visible only as a temporary scaffold.
   - Do not wait for the expensive operation before giving visible feedback.

2. Run the expensive work asynchronously.
   - Track the operation with an explicit task receiver and a named pending/busy state.
   - Request repaint while the task is pending.
   - Reload repository state without stale cache when the operation changes HEAD or worktree state.

3. Block operations that would be unsafe in the half-complete state.
   - Disable actions that depend on the same repository, branch, index, worktree, remote, or target.
   - Context menus, toolbar buttons, dialogs, double-click actions, and keyboard shortcuts must share
     the same busy gate.
   - Keep unrelated navigation allowed only when it cannot mutate or depend on the transitioning state.

4. Release the gate only after the async operation and required reload finish.
   - On success, apply the fresh snapshot, clear the pending state, then re-enable actions.
   - On failure or task disconnect, clear the pending state, show the error, and leave a consistent UI.

New long-running actions need regression tests that prove: immediate visible feedback, async task
ownership, shared busy gating, stale-cache avoidance when required, and gate release on completion.

## Web UI Skill Gate

For the `ui-ux-pro-max` skill, use this decision flow instead of matching every UI-related task:

1. First decide whether the task is a Web UI scenario.
   - Web UI means browser-rendered interface work: HTML/CSS, DOM, canvas/SVG in a browser,
     web app components, web layout/style/animation, or screenshots from a browser UI.

2. If it is Web UI development and the work changes style, layout, animation, accessibility,
   or interaction polish, invoke `ui-ux-pro-max` before implementation.

3. If it is not Web UI, do not invoke `ui-ux-pro-max`.
   - Non-Web UI includes Rust/egui desktop UI, native desktop dialogs, CLI/TUI, native mobile,
     backend logic, Git behavior, tests, docs, and build scripts unless they directly change a
     browser-rendered UI.

4. If a task mixes Web UI and non-Web UI work, invoke `ui-ux-pro-max` only for the Web UI portion.
