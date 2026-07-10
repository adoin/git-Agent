# Create Patch Dialog Design

## Goal

Replace the current direct-save patch action with a SourceTree-style dialog that is always available for an open repository. The dialog supports patches from selected working-copy files or from exact selected commits while correcting SourceTree's known ordering and untracked-file limitations.

## Scope

The feature includes:

- A two-tab `Create Patch` dialog.
- Working-copy file selection.
- History commit selection through row clicks, Ctrl, Shift, and checkboxes.
- One combined patch file or one patch file per selected commit.
- Commit and file previews using existing history and diff components.
- Asynchronous patch generation with repository-wide busy gating.
- Complete Chinese and English localization.

Applying patch files remains handled by the existing `Apply Patch` action and is outside this change.

## Entry And Lifetime

`Actions > Create Patch...` is enabled whenever an active repository exists and no conflicting repository operation is running. A clean working tree does not disable the entry.

Opening the action creates a repository-scoped dialog model. Closing and reopening resets transient selection while retaining only normal persistent history layout preferences. Switching repositories closes the dialog so selections cannot leak between repositories.

## Dialog Layout

The dialog uses the project's borderless, gap-and-shadow visual language.

Top area:

- Localized title: `Create Patch`.
- Two connected tabs: `Working Copy Changes` and `Log / History`.

Bottom area shared by both tabs:

- Patch output path input with a localized placeholder.
- Browse button.
- History tab only: `Create a separate patch file per commit` checkbox.
- Primary `Create Patch` button.
- Secondary `Cancel` button.
- Inline validation or task status text when required.

The primary button is enabled only when the active tab has a valid selection, the output path is valid, and no patch task is running.

## Working Copy Tab

The tab lists current staged, unstaged, deleted, renamed, and untracked paths. Every row has a checkbox. A header checkbox selects or clears all currently visible paths. Search and the existing file ordering controls may filter the list without clearing selections hidden by the filter.

Behavior:

- The dialog opens even when no changes exist.
- With no changes, the list shows a localized empty state and `Create Patch` stays disabled.
- The generated patch contains only selected paths.
- Staged and unstaged changes are compared against `HEAD`, matching SourceTree's working-copy model.
- Untracked regular files are included as new-file diffs, correcting SourceTree's historical omission.
- Unsupported untracked content, such as unreadable files, produces an actionable error instead of silently omitting data.

The output is one ordinary binary-capable unified diff file. It does not preserve commit metadata because working-copy changes have no commit identity.

## History Tab

The tab reuses the complete shared history browser:

- Branch scope dropdown.
- Show remote branches checkbox.
- Date or topology sorting.
- Search field.
- Graph column and commit columns.
- Virtualized scrolling.
- Selected commit summary, changed-file list, and diff preview.

Cherry-pick, jump, and unrelated context-menu actions are hidden in this dialog.

### Selection Model

One repository-scoped ordered selection set is the source of truth for row highlighting and checkboxes.

- Plain click selects one commit and clears the previous set.
- Ctrl-click toggles one commit without clearing other selections.
- Shift-click selects the visible range from the last selection anchor to the clicked row.
- Ctrl+Shift-click adds that visible range to the existing set.
- Clicking a row checkbox toggles the same commit in the same selection set.
- Header checkbox selects or clears all currently visible commits.
- Search and view filters do not discard hidden selections.
- Disabled or unavailable rows cannot enter the selection.

Non-contiguous selection is exact. Commits between two selected commits are not included unless selected.

The bottom preview follows the focused commit, which is independent from the multi-selection set. Selecting files in the preview changes only the displayed diff and does not filter a committed patch, matching SourceTree behavior.

## Patch Generation

History patches use Git mailbox patch format so author, author date, subject, body, and binary changes are preserved.

Before invoking Git, selected commits are normalized into deterministic topological application order, oldest first. This corrects SourceTree's known newest-first numbering issue.

Combined mode:

- Exact selected commits are rendered in oldest-to-newest order.
- All mail patches are concatenated into the selected output file.
- Unselected commits are never inferred from a range.

Separate-file mode:

- The chosen output path supplies directory, basename, and extension.
- One file is written per selected commit.
- Names use `<basename>-0001<extension>`, `<basename>-0002<extension>`, and so on.
- Sequence numbers follow oldest-to-newest application order.
- Existing destination files are detected before generation and require one overwrite confirmation for the full set.

Merge commits are selectable. Their patch is generated against the first parent, matching Git's conventional single-parent patch representation. Root commits use an empty tree as parent.

## Async State Transition

Patch generation follows the repository transition contract:

1. Immediately mark the dialog as generating and show visible progress.
2. Start a named asynchronous task owning all selected hashes, paths, options, and destination data.
3. Disable create, browse, selection mutation, repository mutation actions, context menus, and related shortcuts for the same repository.
4. Keep unrelated read-only navigation available only when it cannot invalidate dialog state.
5. On success, clear the task, close the dialog, and show the generated file count and location.
6. On failure or task disconnect, clear the task, keep the dialog and selections, and show a copyable localized error.

Patch creation does not change `HEAD`, index, or working tree, so repository reload is not required after success.

## Components And Boundaries

`CreatePatchDialog` owns UI state only:

- Active tab.
- Working-copy path selection.
- Commit selection and anchor.
- Focused commit and preview state.
- Output path.
- Separate-file option.
- Validation and pending state.

The shared history browser receives a selection-mode configuration and emits explicit selection intents. It does not own patch semantics.

Git helpers own patch bytes and file generation:

- Create a selected-path working-copy patch.
- Create an exact ordered commit patch stream.
- Create an exact ordered set of numbered files.

The app task layer owns async execution, busy gating, success messages, and errors.

## Validation And Errors

Validation covers:

- Missing selection.
- Empty or invalid output path.
- Output path targeting a directory in combined mode.
- Missing parent directory.
- Existing output collision.
- Commit no longer present after repository state changes.
- Git generation failure.
- Partial write failure.

Separate-file generation writes into temporary sibling files first, then renames them after all patches succeed. Failure removes only temporary files and does not leave a partial numbered series.

## Localization

All titles, tabs, empty states, controls, validation text, overwrite confirmation, progress text, success messages, and errors use i18n keys. No user-visible English literals remain in dialog rendering or task completion paths.

## Tests

Git-level tests:

- Selected working-copy paths exclude unselected tracked files.
- Untracked text and binary files are represented as new files.
- Exact non-contiguous commit selection excludes intermediate commits.
- Combined output preserves oldest-to-newest order and metadata.
- Separate mode writes one correctly numbered file per selected commit.
- Root and merge commits produce valid patches.
- Existing-file and partial-write failures do not leave partial output.

UI/state tests:

- Menu entry remains enabled for a clean repository.
- Clean working-copy tab opens and disables create.
- Ctrl, Shift, Ctrl+Shift, checkbox, and header checkbox update one selection model.
- Filtering retains hidden selections.
- Create enablement tracks selection, path validity, and busy state.
- Opening starts no Git work; submitting enters pending state immediately.
- Pending state applies the shared repository busy gate.
- Success and failure release the gate.
- Repository switching cannot leak selection or task state.
- Every visible string resolves in Chinese and English.

## Acceptance Criteria

- User can always open the dialog for an active idle repository.
- User can create a patch from selected working-copy files, including untracked files.
- User can select exact commits with keyboard modifiers or checkboxes.
- Combined and separate history modes produce deterministic, applicable patches.
- Large histories remain responsive through the existing virtualized browser.
- Long-running generation gives immediate feedback and blocks unsafe same-repository actions.
- Dialog behavior and visible text work in both supported languages.
