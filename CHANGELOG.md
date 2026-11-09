# Changelog

[English](CHANGELOG.md) · [简体中文](CHANGELOG.zh-CN.md)

User-facing changes are recorded here starting with version 1.4.1.
Earlier versions are documented in [GitHub Releases](https://github.com/adoin/git-Agent/releases).

## 2.2.3-beta.1 — 2026-11-09

### Fixed

- Keep pull, push, fetch, checkout, undo, merge-tool and other background operation state independent for each repository. A long-running task no longer disables unrelated repositories, and navigation preserves the original task and its retries.
- Keep background operation errors with their repository until it is viewed, preserving the current repository's errors and notices.
- Restore visible captions and icons for disabled toolbar buttons.
- Generate AI commit messages independently per repository. Preserve results while its tab is inactive and validate the original branch before applying a deferred suggestion.
- Unify Windows path case and separators for repository state while retaining saved tab aliases and sidebar preferences.

### Performance

- Automatically refresh only repositories activated in the current session. Use slower probes for visited background repositories and skip full reads when nothing changed.
- Remove queued automatic reads when a tab is closed or automatic refresh is disabled, while retaining explicit operations and their required completion reloads.
- Bound concurrent Git operations to reduce resource contention and retain a separate background-fetch deadline for each repository.

This is a prerelease for release validation and is excluded from stable update prompts.

## 2.2.2 — 2026-10-27

### Fixed

- Cancel repository-bound forms and reset search state when closing the active repository tab preselects another tab before its visible snapshot is replaced.

- Promote queued background reads when an explicit reload is required, preventing a foreground checkout refresh from waiting on its own blocked background request.

- Finish branch-switch busy state after the matching fresh reload even when another repository tab is selected. Release the original repository on terminal reload errors or disconnection, while preserving the gate during retries and queued reloads.
- Refresh a completed operation's original repository without switching the active tab or replacing another repository's draft, view or loading state. Discard buffered automatic snapshots while a newer explicit reload or mutation is pending.
- Bind post-pull pushes, credential retries and push-recovery confirmations to their original repository. Verify the expected local branch before an automatic quick push.
- Cancel delayed toolbar clicks and unsubmitted repository forms after navigation so they cannot act on a different repository.
- Keep ongoing mutation tasks tracked during branch-sensitive read-cache invalidation. Release exclusive repository access automatically after worker failures and skip abandoned exclusive waiters.
- Include the originating repository path in background Git command errors, and show save/undo success feedback only for the active owner.

Promoted after 2.2.2-beta.1 passed Windows, macOS and Linux release validation.

## 2.2.1 — 2026-10-27

Three-platform builds succeeded. Temporarily excluded from stable update prompts after final review found an additional active-tab-close dialog ownership case. Use 2.2.2 or its validated prerelease.

### Fixed

- Promote queued background reads when an explicit reload is required, preventing a foreground checkout refresh from waiting on its own blocked background request.

- Finish branch-switch busy state after the matching fresh reload even when another repository tab is selected. Release the original repository on terminal reload errors or disconnection, while preserving the gate during retries and queued reloads.
- Refresh a completed operation's original repository without switching the active tab or replacing another repository's draft, view or loading state. Discard buffered automatic snapshots while a newer explicit reload or mutation is pending.
- Bind post-pull pushes, credential retries and push-recovery confirmations to their original repository. Verify the expected local branch before an automatic quick push.
- Cancel delayed toolbar clicks and unsubmitted repository forms after navigation so they cannot act on a different repository.
- Keep ongoing mutation tasks tracked during branch-sensitive read-cache invalidation. Release exclusive repository access automatically after worker failures and skip abandoned exclusive waiters.
- Include the originating repository path in background Git command errors, and show save/undo success feedback only for the active owner.

Promoted after beta.4 passed all three platform builds and published its installers. Application code is unchanged from the validated prerelease.

## 2.2.1-beta.4 — 2026-10-27

- Isolate parallel context-test repositories using exclusively allocated directories and retain Git initialization diagnostics. Beta.3 stopped at fixture initialization on macOS before installer publication; production merge/AI behavior is unchanged by this test adjustment.

### Fixed

- Promote queued background reads when an explicit reload is required, preventing a foreground checkout refresh from waiting on its own blocked background request.

- Finish branch-switch busy state after the matching fresh reload even when another repository tab is selected. Release the original repository on terminal reload errors or disconnection, while preserving the gate during retries and queued reloads.
- Refresh a completed operation's original repository without switching the active tab or replacing another repository's draft, view or loading state. Discard buffered automatic snapshots while a newer explicit reload or mutation is pending.
- Bind post-pull pushes, credential retries and push-recovery confirmations to their original repository. Verify the expected local branch before an automatic quick push.
- Cancel delayed toolbar clicks and unsubmitted repository forms after navigation so they cannot act on a different repository.
- Keep ongoing mutation tasks tracked during branch-sensitive read-cache invalidation. Release exclusive repository access automatically after worker failures and skip abandoned exclusive waiters.
- Keep background Git errors and save/undo feedback associated with the correct repository.

This is a prerelease for release validation and is excluded from stable update prompts.

## 2.2.1-beta.3 — 2026-10-27

macOS CI stopped at temporary context-fixture Git initialization before installer publication. Use beta.4 or later.

### Fixed

- Promote queued background reads when an explicit reload is required, preventing a foreground checkout refresh from waiting on its own blocked background request.

- Finish branch-switch busy state after the matching fresh reload even when another repository tab is selected. Release the original repository on terminal reload errors or disconnection, while preserving the gate during retries and queued reloads.
- Refresh a completed operation's original repository without switching the active tab or replacing another repository's draft, view or loading state. Discard buffered automatic snapshots while a newer explicit reload or mutation is pending.
- Bind post-pull pushes, credential retries and push-recovery confirmations to their original repository. Verify the expected local branch before an automatic quick push.
- Cancel delayed toolbar clicks and unsubmitted repository forms after navigation so they cannot act on a different repository.
- Keep ongoing mutation tasks tracked during branch-sensitive read-cache invalidation. Release exclusive repository access automatically after worker failures and skip abandoned exclusive waiters.
- Keep background Git errors and save/undo feedback associated with the correct repository.

This is a prerelease for release validation and is excluded from stable update prompts.

## 2.2.1-beta.2 — 2026-10-27

Superseded before installer publication while adding regression coverage and a fix for background-to-foreground refresh promotion. Use beta.3 or later.

- Update the checkout regression assertion for authoritative post-operation reloads. Beta.1 was stopped before installer publication after this outdated source-condition assertion failed.

### Fixed

- Finish branch-switch busy state after the matching fresh reload even when another repository tab is selected. Release the original repository on terminal reload errors or disconnection, while preserving the gate during retries and queued reloads.
- Refresh a completed operation's original repository without switching the active tab or replacing another repository's draft, view or loading state. Discard buffered automatic snapshots while a newer explicit reload or mutation is pending.
- Bind post-pull pushes, credential retries and push-recovery confirmations to their original repository. Verify the expected local branch before an automatic quick push.
- Cancel delayed toolbar clicks and unsubmitted repository forms after navigation so they cannot act on a different repository.
- Keep ongoing mutation tasks tracked during branch-sensitive read-cache invalidation. Release exclusive repository access automatically after worker failures and skip abandoned exclusive waiters.
- Keep background Git errors and save/undo feedback associated with the correct repository.

This is a prerelease for release validation and is excluded from stable update prompts.

## 2.2.1-beta.1 — 2026-10-27

Build stopped before installer publication because a checkout source-condition assertion still expected the previous stale-snapshot rule. Use beta.2 or later.

### Fixed

- Finish branch-switch busy state after the matching fresh reload even when another repository tab is selected. Release the original repository on terminal reload errors or disconnection, while preserving the gate during retries and queued reloads.
- Refresh a completed operation's original repository without switching the active tab or replacing another repository's draft, view or loading state. Discard buffered automatic snapshots while a newer explicit reload or mutation is pending.
- Bind post-pull pushes, credential retries and push-recovery confirmations to their original repository. Verify the expected local branch before an automatic quick push.
- Cancel delayed toolbar clicks and unsubmitted repository forms after navigation so they cannot act on a different repository.
- Keep ongoing mutation tasks tracked during branch-sensitive read-cache invalidation. Release exclusive repository access automatically after worker failures and skip abandoned exclusive waiters.
- Keep background Git errors and save/undo feedback associated with the correct repository.

This is a prerelease for release validation and is excluded from stable update prompts.

## 2.2.0 — 2026-10-22

### Added

- Add “Delete local and tracked remote branch” to local branch context menus. Resolve and verify the actual upstream, delete the local branch first, and stop before touching the remote if local deletion fails. Report partial completion and refresh repository state if remote deletion fails.

### Improved

- Focus the first enabled input when action or settings dialogs open, including branch/tag creation, renaming, stash, patches, remotes, author/model settings and rebase message editing. Keep the user's subsequent focus changes intact.
- Align standalone Merge and Diff tools with the main application's saved theme colors, UI/code fonts, font weight, text contrast and UI scale. Share panel shadows, toolbar icons and window-control styling, and reuse unchanged styles between frames.
- Add fixed inset shadows and subtle highlights to the actual scrolling code viewports: separate edges for both Diff columns and all three Merge panes.

### Fixed

- Clear transparent Merge and Diff windows transparently so rounded corners no longer expose an opaque dark canvas.
- Keep deletion connectors tied to their code-row color and existing opacity when changing themes.

Promoted after beta.2 passed all three platform builds and published its installers. Application code is unchanged from the validated prerelease.

## 2.2.0-beta.2 — 2026-10-22

- Update the standalone merge integration test to verify shared theme/shadow helpers after the appearance code was consolidated. Beta.1 stopped at this outdated source-location assertion and published no installers.

### Added

- Add “Delete local and tracked remote branch” to local branch context menus. Resolve and verify the actual upstream, delete the local branch first, and stop before touching the remote if local deletion fails. Report partial completion and refresh repository state if remote deletion fails.

### Improved

- Focus the first enabled input when action or settings dialogs open, including branch/tag creation, renaming, stash, patches, remotes, author/model settings and rebase message editing. Keep the user's subsequent focus changes intact.
- Align standalone Merge and Diff tools with the main application's saved theme colors, UI/code fonts, font weight, text contrast and UI scale. Share panel shadows, toolbar icons and window-control styling, and reuse unchanged styles between frames.
- Add fixed inset shadows and subtle highlights to the actual scrolling code viewports: separate edges for both Diff columns and all three Merge panes.

### Fixed

- Clear transparent Merge and Diff windows transparently so rounded corners no longer expose an opaque dark canvas.
- Keep deletion connectors tied to their code-row color and existing opacity when changing themes.

This is a prerelease for release validation and is excluded from stable update prompts.

## 2.2.0-beta.1 — 2026-10-22

Build stopped before installer publication because a standalone integration test still expected shadow code in its previous file. Use beta.2 or later.

### Added

- Add “Delete local and tracked remote branch” to local branch context menus. Resolve and verify the actual upstream, delete the local branch first, and stop before touching the remote if local deletion fails. Report partial completion and refresh repository state if remote deletion fails.

### Improved

- Focus the first enabled input when action or settings dialogs open, including branch/tag creation, renaming, stash, patches, remotes, author/model settings and rebase message editing. Keep the user's subsequent focus changes intact.
- Align standalone Merge and Diff tools with the main application's saved theme colors, UI/code fonts, font weight, text contrast and UI scale. Share panel shadows, toolbar icons and window-control styling, and reuse unchanged styles between frames.
- Add fixed inset shadows and subtle highlights to the actual scrolling code viewports: separate edges for both Diff columns and all three Merge panes.

### Fixed

- Clear transparent Merge and Diff windows transparently so rounded corners no longer expose an opaque dark canvas.
- Keep deletion connectors tied to their code-row color and existing opacity when changing themes.

This is a prerelease for release validation and is excluded from stable update prompts.

## 2.1.0 — 2026-10-15

### Added

- Browse repository files in an IDE-style tree with progressive folder expansion, folders-first name sorting, a hidden-file toggle, and fuzzy name or wildcard path filtering.
- Open file histories in tabs with the same overflow, close and reorder behavior as repository tabs. Activating a tab reloads its history and follows file renames.
- View another branch's history from its context menu without checking it out, and return directly to the current branch history.
- Switch changed-file lists between flat and tree views from the Path column header.
- Open side-by-side diffs from an icon in the diff header. The large comparison dialog keeps both columns and line numbers visible while scrolling code horizontally.

### Fixed

- Remember the pull dialog's “push current branch after a successful pull” choice separately for each repository, including after restarting.
- Prevent wrapped context-menu titles and long paths from stretching character spacing.
- Align the branch-history banner and return action, remove redundant file-history graph and filename rows, and remove the empty staged-list dash.

Promoted after beta.1 passed all three platform builds and published its installers. Application code is unchanged from the validated prerelease.

## 2.1.0-beta.1 — 2026-10-15

### Added

- Browse repository files in an IDE-style tree with progressive folder expansion, folders-first name sorting, a hidden-file toggle, and fuzzy name or wildcard path filtering.
- Open file histories in tabs with the same overflow, close and reorder behavior as repository tabs. Activating a tab reloads its history and follows file renames.
- View another branch's history from its context menu without checking it out, and return directly to the current branch history.
- Switch changed-file lists between flat and tree views from the Path column header.
- Open side-by-side diffs from an icon in the diff header. The large comparison dialog keeps both columns and line numbers visible while scrolling code horizontally.

### Fixed

- Remember the pull dialog's “push current branch after a successful pull” choice separately for each repository, including after restarting.
- Prevent wrapped context-menu titles and long paths from stretching character spacing.
- Align the branch-history banner and return action, remove redundant file-history graph and filename rows, and remove the empty staged-list dash.

This is a prerelease for release validation and is excluded from stable update prompts.

## 2.0.3 — 2026-10-15

### Fixed

- Refresh automatically generated merge commit messages for the current merge instead of reusing a previous merge's branch and conflict-file list from a saved draft. Clear untouched generated defaults when a merge completes or is cancelled, while preserving manually edited drafts across refreshes and restarts.
- Recognize legacy standard conflict templates and save generated defaults under the correct repository branch.

Promoted after beta.2 passed all three platform builds and published its installers. Application code is unchanged from the validated prerelease.

## 2.0.3-beta.2 — 2026-10-15

- Synchronize the application version in Cargo.lock. The beta.1 build was cancelled before publication because its lockfile version was stale. Includes the merge-message fixes below.

This is a prerelease for release validation and is excluded from stable update prompts.

## 2.0.3-beta.1 — 2026-10-15

Build cancelled before installer publication; use beta.2 or later.

### Fixed

- Refresh automatically generated merge commit messages for the current merge instead of reusing a previous merge's branch and conflict-file list from a saved draft. Clear untouched generated defaults when a merge completes or is cancelled, while preserving manually edited drafts across refreshes and restarts.
- Recognize legacy standard conflict templates and save generated defaults under the correct repository branch.

This is a prerelease for release validation and is excluded from stable update prompts.

## 2.0.2 — 2026-10-13

### Fixed

- Repository refresh probes no longer write Git's optional index stat cache, preventing unchanged repositories from appearing changed on repeated refreshes. Added regression coverage for same-content file writes and index preservation.
- Includes the commit, refresh and AI merge improvements listed below. Version 2.0.1 failed the refresh regression on all three platforms and did not publish installers.

## 2.0.1 — 2026-10-13

Installer build failed. Use 2.0.2 or later for the changes below.

### Improved

- Record staged Git tree IDs for commit undo and redo instead of generating and copying binary patches, reducing overhead for commits containing many images or other binary files.
- Batch branch tracking and repository metadata reads. Refresh repositories concurrently with per-repository operation gates, reuse unchanged snapshots, and retain selections and history when only worktree status changes. Automatic refresh remains enabled and detects external HEAD, ref and configuration changes.
- Let AI merge assistance inspect complete Base, Left, Right, merged-index and worktree source views through repository read, search, history and diff tools. Validate captured revisions and index state before accepting recommendations, and reject mechanically proven broken module references.

### Fixed

- Increase Deepseek merge output allowance from 8K to 384K to accommodate reasoning and final recommendations; increase other Claude-compatible merge requests to 32K. Allow five minutes per request and reject truncated answers with an explicit output-limit error instead of reporting a missing answer.

AI recommendations still require review. Model reasoning and tool-format errors can still occur.

## 2.0.0 — 2026-10-10

### Added

- Released native dynamic-library plugins with a versioned SDK. Plugins provide declarative pages, bilingual dictionaries, functional SVG icons, optional repository-domain matching, synchronization hooks and isolated stores, with store-based permissions for buttons, layouts and actions.
- Install, update, enable, disable or remove plugins through Options → Plugins, with repository overrides in Repository Settings → Plugins. Imported packages are copied into application-managed storage, so the original download can be deleted after installation. Updates and removal finish after restarting.
- Bundled the independently released GitHub plugin 0.1.0 for PRs, issues, workflows, releases and repository summaries. It requires gh and gh auth login; management actions follow the current account's permissions.
- PR details include changed-file navigation, file diffs, AI review drafts and actions for creation, review, merging, closing and reopening. Lists use fixed-bottom pagination with 10, 20, 30 or 50 items per page and internal scrolling. Workflow status and results use distinct badges.
- Plugin remote synchronization runs alongside fetch, pull, push and scheduled background synchronization, refreshing PR counts and repository stars. Unmatched repositories do not show the plugin's buttons or pages.
- Added extensible feature guides. First-time users receive the basic guide followed by the plugin guide; existing users see only unseen features. Completion or skipping persists each guide's revision, and Help supports replay.
- Added bilingual development and installation guides, an independent plugin template and development scripts. The README and website list plugin names and repositories, and developers can share plugins through README pull requests.

### Fixed

- Correctly stage deleted files when resolving modify/delete merge conflicts, including non-ASCII paths, while preserving unrelated worktree changes.
- Share Claude-compatible request handling across commit generation and merge assistance without forcing unsupported thinking or tool-choice settings.
- Save guide progress asynchronously and independently of AI credentials, flushing on exit and blocking background shortcuts and keyboard-focus mistakes while a guide is active.

### Release

- This is a stable release and participates in existing users' update checks. Build and website workflows each retain their latest three runs without affecting published installers.

## 2.0.0-beta.5 — 2026-10-10

### Added

- Added a plugin guide covering installation, package retention, global and repository switches, updates, removal, matching rules, permissions, pages and synchronization. It previews the relevant plugin settings and uses the real panel position for highlighting.
- Introduced a registry of feature guides with stable IDs and revisions. First-time users receive the basic guide followed by the plugin guide; existing users keep their basic-guide status and receive only unseen feature guides. Completing or skipping a guide persists its revision, and Help offers explicit replay.
- Save guide progress independently of AI credentials, asynchronously with coalesced writes and an exit flush. Opening font settings at the end of the basic guide pauses the queue, then resumes the plugin guide when settings close. Background shortcuts and keyboard focus are blocked while a guide is active.

### Changed

- Replaced the website's large plugin instruction cards with compact, keyboard-accessible expandable sections. Plugin repository links remain directly visible, and instructions open on demand in Chinese and English.

This is a prerelease for manual installation. It is excluded from stable update prompts and continues to bundle GitHub plugin 0.1.0.

## 2.0.0-beta.3 — 2026-10-10

### Added

- Added native dynamic-library plugins with a versioned SDK. Plugins provide declarative layouts, bilingual dictionaries, functional SVG icons, repository matching rules, shared synchronization hooks and isolated stores for permission-based visibility and actions.
- Added plugin installation, global and per-repository switches, updates and removal through Options and Repository Settings. Plugin updates and removal finish after restarting the application.
- Bundled the independently released GitHub plugin 0.1.0. It uses an authenticated GitHub CLI to provide PR lists, file diffs, AI review drafts and permission-controlled PR management, plus issues, workflow runs, releases and repository summaries.
- Added fixed-bottom pagination with selectable page sizes and internally scrolling lists, shared plugin page styles and distinct workflow result badges. PR counts and repository stars refresh with remote synchronization.
- Added bilingual plugin development and installation guides, an independent template repository and development-script discovery of local and external plugin projects.

### Fixed

- Escaped the Debian prerelease separator in the packaging script and added early installer-version checks. The beta.2 tag passed tests but failed Linux packaging.
- Corrected the duplicate-entry ZIP test fixture so Linux validates the same malformed package as Windows and macOS. The beta.1 tag failed this fixture check and did not publish installers.
- Correctly stage deleted files when resolving modify/delete merge conflicts, including non-ASCII paths, while preserving unrelated worktree changes.
- Share Claude-compatible request handling across commit generation and merge assistance without forcing unsupported thinking or tool-choice settings.

### Release channel

- This is a prerelease for manual installation and testing. Tags with a SemVer prerelease suffix are published as GitHub prereleases and are excluded from the application's stable update check. Existing users continue to receive stable releases only.

## 1.4.8 — 2026-09-21

### Added

- Added an optional “Push current branch after a successful pull” action. It is available only when merged changes are committed immediately; turning that prerequisite off also clears the push option. Existing upstreams are reused, while branches without an upstream are pushed to a same-name branch on the selected remote and start tracking it.
- Added “View file commit history” to repository file context menus across the workspace, commit details, search results, conflicts, patch creation, and interactive rebase views. The action opens commit search with the complete repository-relative path.
- Full-path file searches now use exact `git log --follow -- <path>` history, preserving rename history and avoiding unrelated files with the same basename.

### Fixed

- Cleared file-search text, tasks, results, selected commits, selected files, and diffs immediately when switching repositories, preventing details from the previous repository from remaining visible.
- Manual navigation from Workspace or History into Commit Search now starts with an empty search. Explicit file-history navigation remains the only path that carries a filename into the search view.

## 1.4.7 — 2026-09-10

### Fixed

- Kept the commit action row anchored to the bottom of the commit panel, so long or AI-generated multi-line commit messages can no longer push the Commit button and commit options out of view.
- Made the commit-message editor scroll vertically inside its fixed viewport instead of expanding the surrounding layout as more lines are entered.
- Prevented the Windows development watcher from exiting when its daily log is briefly opened by another process. Log appends now allow concurrent readers, retry transient sharing violations, and degrade to a warning instead of terminating automatic rebuilds.

## 1.4.6 — 2026-09-10

### Changed

- AI model validation now exercises the real semantic commit-generation protocol instead of accepting a provider after only a minimal text completion.
- Claude-compatible commit and merge requests no longer send the deprecated `temperature` parameter, improving compatibility with newer Claude models routed through AWS Bedrock and third-party gateways.

### Fixed

- AI commit generation now accepts standards-compatible tool arguments encoded as objects or JSON strings, as well as validated JSON or text fallbacks returned by compatibility gateways.
- Multiple `read_file` and `search` tool calls returned in one model response are executed as one bounded batch and fed back together, preserving cross-file business context instead of rejecting legitimate parallel tool use.
- Merge AI responses containing multiple `submit_merge_suggestions` tool calls are combined before the existing uniqueness, target-coverage, and safety checks, preventing complex merges from losing all but the first suggestion batch.
- AI request failures now include bilingual summaries and safely truncated server details or response-structure diagnostics instead of an opaque HTTP status or unsupported-tool error.

## 1.4.5 — 2026-09-07

### Changed

- Reworked Git Agent Diff to use the same frameless shell as the merge tool, with rounded shadowed chrome, custom window controls, borderless sections, and in-window English / Chinese and light / dark switches.

### Fixed

- Preserved non-ASCII file names as real actionable paths throughout worktree status, staging and discarding, commit details and file search, patch creation, undo-state checks, conflict resolution, and AI merge context collection—even when `core.quotepath=true`.
- Switched machine-readable Git path output to NUL-delimited parsing, including rename and copy records, instead of treating Git's display quoting as a path. Patch headers, which do not support NUL-delimited output, now use a dedicated Git C-style quoted-path decoder.
- Kept Chinese and other non-ASCII paths readable and syntax-highlightable in Git Agent Diff, including created, deleted, renamed, and conflicted files.

## 1.4.4 — 2026-09-04

### Added

- Automatic update checks on startup, enabled by default and configurable in General settings. Checks run silently in the background; failures do not interrupt your work, and nothing is downloaded or installed automatically.
- A prominent download-arrow button in the title bar appears only when a newer release is found. Click it to view update details; no unsolicited dialog or hover tooltip is shown.

### Changed

- Moved the interface language selector to the first row of General settings.
- Limited update-check requests to 20 seconds and kept manual checks available through the Help menu.

## 1.4.3 — 2026-09-04

### Fixed

- Restored the shared button styling for Cancel search, retaining cancellation and disabled-state behavior and fixing the regression that blocked the 1.4.2 installer builds. This release includes the search and layout improvements listed below.

## 1.4.2 — 2026-09-04

Installer builds failed; use 1.4.3 or later for these changes.

### Changed

- Enhanced commit search: file searches show filename matches before content matches, support cancellation with Git process cleanup, and use localized progress/error messages. Slow searches allow up to 120 seconds and clearly mark incomplete results.
- Refined commit-detail cards: titles and branch lists are limited to two lines, with full content available in a single scrollable hover tooltip in both history and search.
- Improved layout: increased the commit-search input height and aligned the commit panel's AI generation, history, and options controls on one centerline, with a more compact AI button.

### Fixed

- Corrected the logo's branch-to-node connections and restored fully hollow circular nodes. Window, installer, and website icon resources now share the same SVG artwork.

## 1.4.1 — 2026-09-04

### Added

- AI commit-message generation from staged changes, with a conventional commit subject and numbered, business-oriented change details.
- On-demand AI context lookup in related tracked files and symbol references, including files outside the staged diff. Analysis uses a fixed index-tree snapshot, excluding unstaged edits and untracked files.
- An English / Chinese commit-message language setting, independent of the application interface language.
- A Simplified Chinese README with language-switching links, and bilingual changelogs accessible from the README and project website.

### Changed

- AI generation starts with one click; hovering displays the current model and data-sharing notice. Successful output goes straight into the commit editor, without a separate confirmation or suggestion-acceptance step.
- Draft edits made during generation are preserved. Generation can be cancelled, and changed index or HEAD state invalidates the result. Nothing is committed or pushed automatically.
- The AI generation entry uses a prominent icon button. Disabled primary buttons share the commit button's appearance, including the icon and label colors.

### Licensing

- Added Apache License 2.0 with Commons Clause License Condition v1.0, attribution notices, and license files in release packages. These terms apply together; the project is source-available, not licensed under unmodified Apache 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
