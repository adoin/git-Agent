# Changelog

[English](CHANGELOG.md) · [简体中文](CHANGELOG.zh-CN.md)

User-facing changes are recorded here starting with version 1.4.1.
Earlier versions are documented in [GitHub Releases](https://github.com/adoin/git-Agent/releases).

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
