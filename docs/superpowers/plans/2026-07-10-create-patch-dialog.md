# Create Patch Dialog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an always-available SourceTree-style Create Patch dialog for selected working-copy paths or exact selected commits.

**Architecture:** Add a small pure state module for patch selection and output naming, keep Git patch production in `src/git.rs`, and integrate a repository-scoped dialog plus dedicated async receiver in `src/app.rs`. Extend the existing virtualized history browser through configuration and explicit selection intents instead of creating another commit table.

**Tech Stack:** Rust 2024, eframe/egui, standard-library threads and channels, Git CLI, existing project i18n and test modules.

## Global Constraints

- `Actions > Create Patch...` stays enabled for every active idle repository, including a clean repository.
- Working-copy export includes only selected paths and includes untracked regular files.
- History selection is exact; unselected commits between selected commits never enter output.
- Combined and separate history output is oldest-to-newest.
- Existing virtualized history rendering must remain responsive.
- All long-running work uses an explicit receiver, immediate pending UI, shared same-repository busy gating, repaint polling, and gate release on success, failure, or disconnect.
- Patch generation never reloads the repository because it does not mutate `HEAD`, index, or worktree.
- Every visible string resolves through Chinese and English i18n.
- Existing dirty changes in `src/app.rs`, `src/git.rs`, and `src/i18n.rs` must be preserved.

---

### Task 1: Patch Selection And Output Model

**Files:**
- Create: `src/patch.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces: `CreatePatchTab`, `PatchSelectionGesture`, `CommitPatchSelection`, `numbered_patch_paths`, and `validate_patch_output_path`.
- Consumes: visible commit hashes supplied by the history browser and ordinary `Path` values.

- [ ] **Step 1: Write failing pure-state tests**

Add `src/patch.rs` with tests first. Define wished-for public API in tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn hashes() -> Vec<String> {
        ["a", "b", "c", "d"].into_iter().map(str::to_owned).collect()
    }

    #[test]
    fn plain_toggle_and_range_gestures_share_one_selection() {
        let visible = hashes();
        let mut selection = CommitPatchSelection::default();

        selection.apply(&visible, "b", PatchSelectionGesture::Plain);
        assert_eq!(selection.ordered(), vec!["b"]);

        selection.apply(&visible, "d", PatchSelectionGesture::Toggle);
        assert_eq!(selection.ordered(), vec!["b", "d"]);

        selection.apply(&visible, "c", PatchSelectionGesture::ReplaceRange);
        assert_eq!(selection.ordered(), vec!["c", "d"]);

        selection.apply(&visible, "a", PatchSelectionGesture::AddRange);
        assert_eq!(selection.ordered(), vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn filtering_retains_hidden_selection() {
        let mut selection = CommitPatchSelection::default();
        selection.apply(&hashes(), "b", PatchSelectionGesture::Plain);
        selection.apply(&hashes(), "d", PatchSelectionGesture::Toggle);
        selection.toggle_visible(&["a".into(), "c".into()]);
        assert_eq!(selection.ordered(), vec!["b", "d", "a", "c"]);
    }

    #[test]
    fn numbered_paths_keep_basename_extension_and_order() {
        assert_eq!(
            numbered_patch_paths(Path::new("D:/out/review.diff"), 2),
            vec![
                Path::new("D:/out/review-0001.diff").to_path_buf(),
                Path::new("D:/out/review-0002.diff").to_path_buf(),
            ]
        );
    }

    #[test]
    fn output_validation_rejects_empty_and_directory_paths() {
        assert_eq!(validate_patch_output_path(Path::new("")), Err(PatchPathError::Empty));
        let temp = std::env::temp_dir();
        assert_eq!(validate_patch_output_path(&temp), Err(PatchPathError::Directory));
    }
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```powershell
cargo test --lib patch::tests
```

Expected: compilation fails because the module and types are not implemented/exported.

- [ ] **Step 3: Implement minimal selection model**

Create these concrete types and methods in `src/patch.rs`:

```rust
use std::{collections::HashSet, path::{Path, PathBuf}};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum CreatePatchTab {
    #[default]
    Worktree,
    History,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PatchSelectionGesture {
    Plain,
    Toggle,
    ReplaceRange,
    AddRange,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CommitPatchSelection {
    ordered: Vec<String>,
    selected: HashSet<String>,
    anchor: Option<String>,
}

impl CommitPatchSelection {
    pub(crate) fn apply(
        &mut self,
        visible: &[String],
        hash: &str,
        gesture: PatchSelectionGesture,
    );
    pub(crate) fn toggle_visible(&mut self, visible: &[String]);
    pub(crate) fn contains(&self, hash: &str) -> bool;
    pub(crate) fn ordered(&self) -> Vec<String>;
    pub(crate) fn retain_available(&mut self, available: &HashSet<String>);
    pub(crate) fn is_empty(&self) -> bool;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PatchPathError {
    Empty,
    Directory,
    MissingParent,
}

pub(crate) fn validate_patch_output_path(path: &Path) -> Result<(), PatchPathError>;
pub(crate) fn numbered_patch_paths(base: &Path, count: usize) -> Vec<PathBuf>;
```

Range order follows the current visible list. Hidden selections remain in `ordered`. Header toggling adds all missing visible hashes or removes all visible hashes when every visible hash is already selected.

Add `mod patch;` to `src/lib.rs`.

- [ ] **Step 4: Run tests and verify GREEN**

Run:

```powershell
cargo test --lib patch::tests
```

Expected: all patch model tests pass.

- [ ] **Step 5: Commit model**

```powershell
git add src/patch.rs src/lib.rs
git commit -m "feat: add patch selection model"
```

---

### Task 2: Selected Working-Copy Patch Engine

**Files:**
- Modify: `src/git.rs`

**Interfaces:**
- Produces: `create_worktree_patch_for_paths(root, output_path, paths) -> Result<Vec<PathBuf>>`.
- Consumes: repository root, one output file, exact selected repository-relative paths.

- [ ] **Step 1: Write failing Git integration tests**

Add tests beside `create_and_apply_worktree_patch_round_trip`:

```rust
#[test]
fn selected_worktree_patch_excludes_unselected_paths() -> Result<()> {
    let root = init_patch_test_repo("selected-paths")?;
    fs::write(root.join("selected.txt"), "changed\n")?;
    fs::write(root.join("ignored.txt"), "changed\n")?;
    let output = root.join("selected.diff");

    create_worktree_patch_for_paths(
        &root,
        &output,
        &["selected.txt".to_owned()],
    )?;

    let patch = fs::read_to_string(output)?;
    assert!(patch.contains("selected.txt"));
    assert!(!patch.contains("ignored.txt"));
    Ok(())
}

#[test]
fn selected_worktree_patch_includes_untracked_text_and_binary() -> Result<()> {
    let root = init_patch_test_repo("untracked-paths")?;
    fs::write(root.join("new.txt"), "new text\n")?;
    fs::write(root.join("new.bin"), [0_u8, 1, 2, 255])?;
    let output = root.join("untracked.diff");

    create_worktree_patch_for_paths(
        &root,
        &output,
        &["new.txt".to_owned(), "new.bin".to_owned()],
    )?;

    let patch = fs::read(output)?;
    let patch = String::from_utf8_lossy(&patch);
    assert!(patch.contains("new file mode"));
    assert!(patch.contains("new.txt"));
    assert!(patch.contains("new.bin"));
    assert!(patch.contains("GIT binary patch"));
    Ok(())
}
```

`init_patch_test_repo` creates, configures, commits two tracked files, and returns an isolated temp repository.

- [ ] **Step 2: Run tests and verify RED**

```powershell
cargo test --lib git::tests::selected_worktree_patch
```

Expected: compilation fails because `create_worktree_patch_for_paths` is missing.

- [ ] **Step 3: Implement exact selected-path export**

Add:

```rust
pub fn create_worktree_patch_for_paths(
    root: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    paths: &[String],
) -> Result<Vec<PathBuf>>;
```

Implementation rules:

- Reject an empty selection.
- Split selected paths using `git ls-files --error-unmatch -- <path>`.
- Generate tracked/staged/unstaged content with `git diff --binary --full-index HEAD -- <tracked paths>`.
- Generate each selected untracked path with `git diff --binary --no-index -- /dev/null <path>`; accept exit status `1` as successful diff output.
- Concatenate outputs in caller selection order.
- Write through a sibling temporary file named `.<filename>.git-agent-tmp-<pid>`, flush, then rename to the destination.
- Return `vec![output_path]`.
- Keep `create_worktree_patch` as a compatibility wrapper that collects all changed paths and calls the new function.

- [ ] **Step 4: Run tests and verify GREEN**

```powershell
cargo test --lib git::tests::selected_worktree_patch
cargo test --lib git::tests::create_and_apply_worktree_patch_round_trip
```

Expected: selected-path tests and existing round-trip pass.

- [ ] **Step 5: Commit working-copy engine**

```powershell
git add src/git.rs
git commit -m "feat: export selected worktree paths"
```

---

### Task 3: Exact Commit Patch Engine

**Files:**
- Modify: `src/git.rs`
- Modify: `src/patch.rs`

**Interfaces:**
- Produces: `CreatePatchRequest`, `create_patch`, and `create_commit_patches(root, output_path, hashes, separate) -> Result<Vec<PathBuf>>`.
- Consumes: exact commit hashes in any UI order; uses `numbered_patch_paths` for separate output.

- [ ] **Step 1: Write failing exact-selection and ordering tests**

Create a linear repository with commits `A`, `B`, `C`, then pass `[C, A]`:

```rust
#[test]
fn combined_commit_patch_is_exact_and_oldest_first() -> Result<()> {
    let fixture = init_commit_patch_repo()?;
    let output = fixture.root.join("series.patch");

    create_commit_patches(
        &fixture.root,
        &output,
        &[fixture.c.clone(), fixture.a.clone()],
        false,
    )?;

    let patch = fs::read_to_string(output)?;
    assert!(patch.contains("Subject: [PATCH 1/2] commit A"));
    assert!(patch.contains("Subject: [PATCH 2/2] commit C"));
    assert!(!patch.contains("commit B"));
    assert!(patch.find("commit A").unwrap() < patch.find("commit C").unwrap());
    Ok(())
}

#[test]
fn separate_commit_patches_are_numbered_oldest_first() -> Result<()> {
    let fixture = init_commit_patch_repo()?;
    let output = fixture.root.join("series.diff");
    let files = create_commit_patches(
        &fixture.root,
        &output,
        &[fixture.c.clone(), fixture.a.clone()],
        true,
    )?;

    assert_eq!(files[0].file_name().unwrap(), "series-0001.diff");
    assert_eq!(files[1].file_name().unwrap(), "series-0002.diff");
    assert!(fs::read_to_string(&files[0])?.contains("commit A"));
    assert!(fs::read_to_string(&files[1])?.contains("commit C"));
    Ok(())
}
```

Add root-commit and first-parent merge-commit fixtures. Assert non-empty output and correct changed path.

- [ ] **Step 2: Run tests and verify RED**

```powershell
cargo test --lib git::tests::combined_commit_patch_is_exact_and_oldest_first
cargo test --lib git::tests::separate_commit_patches_are_numbered_oldest_first
```

Expected: compilation fails because `create_commit_patches` is missing.

- [ ] **Step 3: Implement exact commit rendering**

Add:

```rust
#[derive(Clone, Debug)]
pub enum CreatePatchRequest {
    Worktree {
        output_path: PathBuf,
        paths: Vec<String>,
    },
    Commits {
        output_path: PathBuf,
        hashes: Vec<String>,
        separate: bool,
    },
}

pub fn create_commit_patches(
    root: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    hashes: &[String],
    separate: bool,
) -> Result<Vec<PathBuf>>;

pub fn create_patch(root: &Path, request: &CreatePatchRequest) -> Result<Vec<PathBuf>> {
    match request {
        CreatePatchRequest::Worktree { output_path, paths } => {
            create_worktree_patch_for_paths(root, output_path, paths)
        }
        CreatePatchRequest::Commits {
            output_path,
            hashes,
            separate,
        } => create_commit_patches(root, output_path, hashes, *separate),
    }
}
```

Implementation rules:

- Validate every hash with `git cat-file -e <hash>^{commit}`.
- Build canonical order from `git rev-list --topo-order --reverse --all`, filter to exact selected hashes, then append selected unreachable hashes sorted by author timestamp and hash.
- Render one commit at a time. Normal commits use `git format-patch --binary --full-index --stdout -1 <hash>`. Root commits add `--root`. Merge commits use first-parent patch content and the merge commit's author/message envelope.
- Combined mode rewrites `[PATCH]` subjects to `[PATCH n/m]` and concatenates exact rendered messages.
- Separate mode writes one message per numbered path from `numbered_patch_paths`.
- Generate all sibling temporary files first. Before final rename, move existing confirmed-overwrite destinations to sibling backup names. If any rename fails, restore every backup and remove already-installed outputs. Remove backups only after the full set succeeds.
- Return final paths in application order.

- [ ] **Step 4: Run tests and verify GREEN**

```powershell
cargo test --lib git::tests::combined_commit_patch_is_exact_and_oldest_first
cargo test --lib git::tests::separate_commit_patches_are_numbered_oldest_first
cargo test --lib git::tests::commit_patch_handles_root_and_merge_commits
```

Expected: all exact commit patch tests pass.

- [ ] **Step 5: Commit history engine**

```powershell
git add src/git.rs src/patch.rs
git commit -m "feat: export exact commit patches"
```

---

### Task 4: Shared History Browser Multi-Selection Events

**Files:**
- Modify: `src/app.rs`
- Modify: `src/patch.rs`

**Interfaces:**
- Consumes: `CommitPatchSelection` and `PatchSelectionGesture`.
- Produces: `HistoryCommitBrowserOutcome.selection_intent: Option<HistorySelectionIntent>` and `visible_hashes: Vec<String>`.

- [ ] **Step 1: Write failing selection-intent tests**

Add source-structure regression tests in `app::ui_tests` plus pure modifier mapping tests in `src/patch.rs`:

```rust
#[test]
fn modifier_mapping_matches_windows_multi_selection() {
    assert_eq!(selection_gesture(false, false), PatchSelectionGesture::Plain);
    assert_eq!(selection_gesture(true, false), PatchSelectionGesture::Toggle);
    assert_eq!(selection_gesture(false, true), PatchSelectionGesture::ReplaceRange);
    assert_eq!(selection_gesture(true, true), PatchSelectionGesture::AddRange);
}

#[test]
fn history_browser_exposes_multi_select_without_duplicating_table() {
    let source = include_str!("app.rs");
    assert!(source.contains("selection_intent: Option<HistorySelectionIntent>"));
    assert!(source.contains("history_commit_browser("));
    assert!(source.contains("config.multi_select"));
    assert!(!source.contains("fn create_patch_history_table("));
}
```

- [ ] **Step 2: Run tests and verify RED**

```powershell
cargo test --lib modifier_mapping_matches_windows_multi_selection
cargo test --lib history_browser_exposes_multi_select_without_duplicating_table
```

Expected: tests fail because selection intent and config do not exist.

- [ ] **Step 3: Extend browser configuration and outcome**

Add:

```rust
#[derive(Clone, Debug)]
struct HistorySelectionIntent {
    hash: String,
    gesture: PatchSelectionGesture,
}

struct HistoryCommitBrowserConfig {
    // existing fields
    multi_select: bool,
}

struct HistoryCommitBrowserOutcome {
    // existing fields
    selection_intent: Option<HistorySelectionIntent>,
    toggle_all_visible: bool,
    visible_hashes: Vec<String>,
}
```

When `multi_select` is true:

- Render the existing shadow-based checkbox column.
- Checkbox click emits `Toggle`.
- Row click maps current Ctrl/Shift modifiers through `selection_gesture`.
- Header checkbox emits `toggle_all_visible`.
- Preserve normal focused-row `picked_commit` behavior for preview.
- Existing checkout, merge, and interactive-rebase callers set `multi_select: false` unless they already use checkbox rows; rebase keeps current behavior through its existing selection fields.

- [ ] **Step 4: Run selection tests and existing browser tests**

```powershell
cargo test --lib modifier_mapping_matches_windows_multi_selection
cargo test --lib history_browser_exposes_multi_select_without_duplicating_table
cargo test --lib history_commit_browser
cargo test --lib interactive_rebase
```

Expected: new and existing browser tests pass.

- [ ] **Step 5: Commit browser extension**

```powershell
git add src/app.rs src/patch.rs
git commit -m "feat: add history multi-selection events"
```

---

### Task 5: Create Patch Dialog Shell And Working-Copy Tab

**Files:**
- Modify: `src/app.rs`
- Modify: `src/i18n.rs`

**Interfaces:**
- Consumes: patch path validation and current `RepositorySnapshot` staged/unstaged lists.
- Produces: `CreatePatchDialog`, `create_patch_action_modal`, and a `git::CreatePatchRequest::Worktree` request.

- [ ] **Step 1: Write failing dialog/menu/i18n tests**

Add tests asserting:

```rust
#[test]
fn create_patch_menu_is_enabled_for_clean_repository() {
    let source = include_str!("app.rs");
    let start = source.find("menu_label(self.language, \"actions_create_patch\")").unwrap();
    let menu = &source[start.saturating_sub(320)..(start + 520).min(source.len())];
    assert!(!menu.contains("snapshot.status.is_empty"));
    assert!(source.contains("self.pending_create_patch_action = Some("));
}

#[test]
fn create_patch_dialog_has_two_localized_tabs_and_selected_worktree_paths() {
    let source = include_str!("app.rs");
    for required in [
        "CreatePatchTab::Worktree",
        "CreatePatchTab::History",
        "patch.create.worktree_tab",
        "patch.create.history_tab",
        "selected_worktree_paths",
        "create_patch_action_modal(ctx)",
    ] {
        assert!(source.contains(required), "missing {required}");
    }
}
```

Extend `i18n::tests::chinese_labels_are_not_mojibake` for every `patch.create.*` key.

- [ ] **Step 2: Run tests and verify RED**

```powershell
cargo test --lib create_patch_menu_is_enabled_for_clean_repository
cargo test --lib create_patch_dialog_has_two_localized_tabs_and_selected_worktree_paths
cargo test --lib i18n::tests::chinese_labels_are_not_mojibake
```

Expected: dialog tests fail on missing state and keys.

- [ ] **Step 3: Add dialog state and working-copy UI**

Add state:

```rust
#[derive(Clone, Debug)]
struct CreatePatchDialog {
    repo_root: PathBuf,
    tab: CreatePatchTab,
    output_path: String,
    selected_worktree_paths: HashSet<String>,
    commit_selection: CommitPatchSelection,
    focused_commit_hash: String,
    branch_scope: HistoryBranchScope,
    sort_order: HistorySortOrder,
    search: String,
    show_remote_refs: bool,
    separate_files: bool,
    validation_error_key: Option<&'static str>,
    history_cache: HistoryCommitBrowserCache,
}
```

Add `pending_create_patch_action: Option<CreatePatchDialog>` to `GitAgentApp`, initialize it, clear it on repository switches, call `create_patch_action_modal(ctx)` from `update`, and replace `create_worktree_patch()` with `open_create_patch_dialog()`.

Working-copy UI rules:

- Build unique rows from staged and unstaged `WorktreeFile` paths.
- Select all paths by default when changes exist.
- Show empty state when clean.
- Header checkbox toggles all paths.
- Use existing worktree icons and borderless row styling.
- Browse uses `rfd::FileDialog`, default `patch.diff` under repository root.
- Primary button disabled for no selection or invalid output.
- On submit, build `CreatePatchRequest::Worktree { output_path, paths }`; task execution arrives in Task 7.

- [ ] **Step 4: Add complete i18n keys**

Add Chinese and English values for title, both tabs, changed-files count, empty state, output path, browse tooltip, separate-files option, create, cancel, validation errors, overwrite confirmation, running status, success singular/plural, and disconnect error.

- [ ] **Step 5: Run dialog and i18n tests**

```powershell
cargo test --lib create_patch_menu_is_enabled_for_clean_repository
cargo test --lib create_patch_dialog_has_two_localized_tabs_and_selected_worktree_paths
cargo test --lib i18n::tests::chinese_labels_are_not_mojibake
```

Expected: all pass.

- [ ] **Step 6: Commit dialog shell**

```powershell
git add src/app.rs src/i18n.rs
git commit -m "feat: add create patch dialog"
```

---

### Task 6: History Tab, Selection, And Preview

**Files:**
- Modify: `src/app.rs`

**Interfaces:**
- Consumes: shared history browser events and dialog commit selection.
- Produces: `git::CreatePatchRequest::Commits` with exact selected hashes.

- [ ] **Step 1: Write failing history-tab tests**

Add tests:

```rust
#[test]
fn create_patch_history_reuses_complete_virtualized_browser() {
    let source = include_str!("app.rs");
    let start = source.find("fn create_patch_history_tab(").unwrap();
    let body = &source[start..];
    for required in [
        "history_commit_browser(",
        "multi_select: true",
        "show_view_controls: true",
        "show_search: true",
        "ScrollArea",
        "commit_selection.apply(",
        "commit_selection.toggle_visible(",
        "create_patch_commit_preview(",
    ] {
        assert!(body.contains(required), "missing {required}");
    }
}

#[test]
fn create_patch_history_submits_exact_selected_hashes() {
    let source = include_str!("app.rs");
    assert!(source.contains("CreatePatchRequest::Commits"));
    assert!(source.contains("dialog.commit_selection.ordered()"));
}
```

- [ ] **Step 2: Run tests and verify RED**

```powershell
cargo test --lib create_patch_history_reuses_complete_virtualized_browser
cargo test --lib create_patch_history_submits_exact_selected_hashes
```

Expected: tests fail because history tab helpers are missing.

- [ ] **Step 3: Implement history browser wiring**

Use `snapshot.all_date_commits` or `snapshot.all_topology_commits` according to dialog sort/scope. Feed selected hashes into the existing checkbox-selected set argument. Apply emitted row gestures and header toggles to `dialog.commit_selection`. Keep `focused_commit_hash` updated independently for preview.

Retain selection against the current snapshot's available hash set each frame. Search/filter changes keep hidden selected hashes.

- [ ] **Step 4: Implement compact preview**

Reuse `details_cache`, `diff_cache`, existing commit summary, changed-file list, and diff renderer. Focus changes request details asynchronously through existing detail/diff tasks. Preview file clicks do not alter patch selection.

History submit builds:

```rust
CreatePatchRequest::Commits {
    output_path: PathBuf::from(dialog.output_path.trim()),
    hashes: dialog.commit_selection.ordered(),
    separate: dialog.separate_files,
}
```

- [ ] **Step 5: Run history tests and browser regression tests**

```powershell
cargo test --lib create_patch_history
cargo test --lib history_commit_browser
cargo test --lib history_virtual
```

Expected: all pass; no duplicate non-virtualized table exists.

- [ ] **Step 6: Commit history tab**

```powershell
git add src/app.rs
git commit -m "feat: select commits for patch export"
```

---

### Task 7: Dedicated Async Patch Task And Busy Gate

**Files:**
- Modify: `src/app.rs`
- Modify: `src/i18n.rs`

**Interfaces:**
- Produces: `CreatePatchTaskResult`, `start_create_patch_task`, `poll_create_patch_task`, and `create_patch_busy`.
- Consumes: validated request from either dialog tab.

- [ ] **Step 1: Write failing transition tests**

Add source-backed regression tests following existing branch and benchmark transition tests:

```rust
#[test]
fn create_patch_uses_named_async_task_and_shared_busy_gate() {
    let source = include_str!("app.rs");
    for required in [
        "create_patch_task: Option<Receiver<CreatePatchTaskResult>>",
        "fn start_create_patch_task(",
        "fn poll_create_patch_task(",
        "fn create_patch_busy(&self)",
        "self.create_patch_task = Some(receiver)",
        "ctx.request_repaint_after(Duration::from_millis(80))",
        "self.create_patch_busy()",
    ] {
        assert!(source.contains(required), "missing {required}");
    }
}

#[test]
fn create_patch_gate_releases_on_success_failure_and_disconnect() {
    let source = include_str!("app.rs");
    let start = source.find("fn poll_create_patch_task(").unwrap();
    let body = &source[start..];
    assert!(body.contains("Ok((root, Ok(paths)))"));
    assert!(body.contains("Ok((root, Err(error)))"));
    assert!(body.contains("TryRecvError::Disconnected"));
    assert!(body.matches("self.create_patch_task = None").count() >= 3);
}
```

- [ ] **Step 2: Run tests and verify RED**

```powershell
cargo test --lib create_patch_uses_named_async_task_and_shared_busy_gate
cargo test --lib create_patch_gate_releases_on_success_failure_and_disconnect
```

Expected: tests fail because dedicated task fields and polling are missing.

- [ ] **Step 3: Add dedicated task state**

In `src/app.rs`, consume the dispatcher introduced in Task 3:

```rust
type CreatePatchTaskResult = (PathBuf, anyhow::Result<Vec<PathBuf>>);
```

Add receiver field and methods. `start_create_patch_task` sets pending state before spawning. `poll_create_patch_task`:

- Success: clear receiver, close dialog, show localized count/location toast.
- Failure: clear receiver, retain dialog selections, open copyable error.
- Empty: restore receiver and request repaint after 80 ms.
- Disconnected: clear receiver, retain dialog, show localized disconnect error.

- [ ] **Step 4: Integrate shared busy gating**

Add `create_patch_busy()` to `branch_actions_busy()`, toolbar busy state, Actions menu gating, dialog controls, context menus, shortcuts, and repository switching mutations. Read-only preview remains usable while generation runs.

Do not set `loading_repo` and do not call `load_repository` after completion.

- [ ] **Step 5: Add overwrite confirmation**

Before spawning:

- Combined mode checks the one destination.
- Separate mode computes every numbered destination.
- Any collision opens one localized confirmation listing count and directory.
- Confirm resumes with the exact frozen request; cancel returns to editable dialog.

- [ ] **Step 6: Run transition and integration tests**

```powershell
cargo test --lib create_patch_uses_named_async_task_and_shared_busy_gate
cargo test --lib create_patch_gate_releases_on_success_failure_and_disconnect
cargo test --lib create_patch
```

Expected: all pass.

- [ ] **Step 7: Commit async integration**

```powershell
git add src/app.rs src/git.rs src/i18n.rs
git commit -m "feat: generate patches asynchronously"
```

---

### Task 8: Full Verification And UI Polish

**Files:**
- Modify: `src/app.rs`
- Modify: `src/i18n.rs`
- Modify: `src/patch.rs`
- Modify: `src/git.rs`

**Interfaces:**
- Consumes all prior tasks.
- Produces release-ready behavior matching the approved design.

- [ ] **Step 1: Run focused test suites**

```powershell
cargo test --lib patch::tests
cargo test --lib git::tests::selected_worktree_patch
cargo test --lib git::tests::combined_commit_patch_is_exact_and_oldest_first
cargo test --lib create_patch
```

Expected: all focused tests pass.

- [ ] **Step 2: Run complete verification**

```powershell
cargo fmt -- --check
cargo test --lib
git diff --check
```

Expected: formatter clean, every library test passes, no whitespace errors.

- [ ] **Step 3: Inspect dialog manually**

Run the app and verify with one clean and one dirty repository:

```powershell
cargo run
```

Check:

- Menu opens on clean repository.
- Tabs use connected active styling and inactive tab shadow rules.
- No borders appear around panels, buttons, or checkboxes.
- Worktree create disables with no selected change.
- Ctrl, Shift, Ctrl+Shift, and checkbox selections agree.
- Long history scroll remains smooth.
- Pending state appears immediately and blocks same-repository mutation.
- Generated combined and numbered patches apply in a disposable clone.
- Chinese and English labels fit without raw keys.

- [ ] **Step 4: Fix only observed regressions with new failing tests**

For every observed regression, add one focused failing test, verify RED, apply the smallest fix, then rerun the focused and full suites.

- [ ] **Step 5: Commit final polish**

```powershell
git add src/app.rs src/git.rs src/i18n.rs src/patch.rs
git commit -m "fix: polish create patch workflow"
```
