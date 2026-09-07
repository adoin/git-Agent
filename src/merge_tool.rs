use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    env, fs,
    ops::Range,
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

#[allow(unused_imports)]
use std::process::Command;

use anyhow::{Context, anyhow};
use eframe::egui::containers::scroll_area::ScrollBarVisibility;
use eframe::{
    App,
    egui::{
        self, Align, Align2, Color32, ComboBox, CursorIcon, FontId, Layout, Pos2, Rect, RichText,
        ScrollArea, Sense, Ui, Vec2,
        text::{LayoutJob, TextFormat},
    },
};
use similar::{Algorithm, DiffTag, capture_diff_slices};

#[allow(unused_imports)]
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::{
    dialog,
    syntax::{HighlightedDocument, HighlightedLine, SyntaxRole},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeTheme {
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeLanguage {
    English,
    Chinese,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MergeIgnoreMode {
    None,
    TrimWhitespace,
    IgnoreWhitespace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MergeHighlightMode {
    Lines,
    Words,
}

#[derive(Clone, Copy)]
enum MergeToolbarToggleIcon {
    Ai,
    Collapse,
    Expand,
    Sun,
    Moon,
    Language,
}

#[derive(Clone, Debug)]
struct MergeSourceText {
    base: String,
    local: String,
    remote: String,
}

#[derive(Clone, Debug, Default)]
struct MergeSyntaxHighlights {
    local: Option<HighlightedDocument>,
    remote: Option<HighlightedDocument>,
    result: Option<HighlightedDocument>,
    local_source_lines: Vec<String>,
    remote_source_lines: Vec<String>,
    local_unique_source_lines: HashMap<String, Option<usize>>,
    remote_unique_source_lines: HashMap<String, Option<usize>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MergeAiChoice {
    Local,
    Remote,
    Manual,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MergeAiSuggestion {
    target: MergeLineActionTarget,
    choice: MergeAiChoice,
    reason_zh: String,
    reason_en: String,
    manual_result: Option<String>,
    middle_edits: Vec<MergeAiMiddleEdit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MergeAiMiddleEdit {
    expected_text: String,
    replacement_text: String,
}

impl MergeAiSuggestion {
    fn reason(&self, language: MergeLanguage) -> &str {
        match language {
            MergeLanguage::Chinese => &self.reason_zh,
            MergeLanguage::English => &self.reason_en,
        }
    }

    fn is_actionable(&self) -> bool {
        self.choice != MergeAiChoice::Manual || self.manual_result.is_some()
    }

    fn change_count(&self) -> usize {
        usize::from(self.is_actionable()) + self.middle_edits.len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MergeAiNotice {
    Completed { suggestions: usize, changes: usize },
    NoSuggestions,
}

#[derive(Clone, Debug, Default)]
struct MergeAiContext {
    history: String,
    repository_state: String,
    related_files: String,
    symbol_references: String,
}

struct PreparedMergeDocument {
    initial_document: MergeDocument,
    document: MergeDocument,
    sources: MergeSourceText,
    result_text: String,
    manual_result_lines: Vec<String>,
    result_display_rows: Vec<CachedMergeResultDisplayRow>,
    local_display_rows: Vec<CachedMergeSideDisplayRow>,
    remote_display_rows: Vec<CachedMergeSideDisplayRow>,
    geometry_cache: MergeGeometryCache,
    local_scroll_anchors: Vec<(f32, f32)>,
    remote_scroll_anchors: Vec<(f32, f32)>,
    local_navigation_target: Option<MergeLineActionTarget>,
    remote_navigation_target: Option<MergeLineActionTarget>,
    syntax_highlights: MergeSyntaxHighlights,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MergeLoadStage {
    ReadingFiles,
    ComparingChanges,
    PreparingEditor,
}

#[derive(Clone, Copy, Debug)]
struct MergeLoadProgress {
    stage: MergeLoadStage,
    total_bytes: usize,
    total_lines: usize,
}

enum MergeLoadEvent {
    Progress(MergeLoadProgress),
    Finished(anyhow::Result<PreparedMergeDocument>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergeArgs {
    pub base: PathBuf,
    pub local: PathBuf,
    pub remote: PathBuf,
    pub output: PathBuf,
    pub repo_root: Option<PathBuf>,
    pub stage: bool,
    pub theme: MergeTheme,
    pub language: MergeLanguage,
    /// The selected configuration name is safe to pass from the main app. The standalone tool
    /// resolves the secret itself through the operating system key store.
    pub ai_model_name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergeDocument {
    pub lines: Vec<MergeLine>,
    conflicts: Vec<ConflictBlock>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergeLine {
    pub base: Option<String>,
    pub local: Option<String>,
    pub remote: Option<String>,
    pub result: String,
    pub include_in_result: bool,
    pub kind: MergeLineKind,
    pub conflict_index: Option<usize>,
    local_resolved: bool,
    remote_resolved: bool,
    local_taken: bool,
    remote_taken: bool,
    base_only_resolved: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeLineKind {
    Resolved,
    Conflict,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConflictBlock {
    pub index: usize,
    pub base: Vec<String>,
    pub local: Vec<String>,
    pub remote: Vec<String>,
    line_indices: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MergeSide {
    Local,
    Remote,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MergeLineAction {
    Take,
    Drop,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum MergeLineActionTarget {
    Conflict(usize),
    BaseOnlyGroup(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NavDirection {
    Previous,
    Next,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
enum MergeSearchPane {
    Left,
    #[default]
    Middle,
    Right,
}

#[derive(Clone, Debug, Default)]
struct MergeSearchState {
    open: bool,
    pane: MergeSearchPane,
    query: String,
    current: usize,
    request_focus: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum MergeSideLineTone {
    Unchanged,
    Added,
    BaseOnly,
    Deleted,
    Replaced,
    LocalDeletedRemoteEdited,
    LocalEditedRemoteDeleted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeConnectorDebug {
    Off,
    Guides,
    Log,
}

pub const MERGE_TOOL_CANCEL_EXIT_CODE: i32 = 10;

const MERGE_NAV_BUTTON_SIZE: f32 = 18.0;
const MERGE_PANEL_RADIUS: u8 = 6;
const MERGE_CODE_ROW_HEIGHT: f32 = 18.0;
const MERGE_CODE_FONT_SIZE: f32 = 12.0;
const MERGE_SIDE_CODE_GUTTER_WIDTH: f32 = 100.0;
const MERGE_RESULT_CODE_GUTTER_WIDTH: f32 = 62.0;
const MERGE_HORIZONTAL_SCROLLBAR_HEIGHT: f32 = 12.0;
const MERGE_HORIZONTAL_SCROLLBAR_GAP: f32 = 4.0;
const MERGE_WORD_BLOCK_OPACITY: f32 = 1.0;
const MERGE_WORD_ACTIVE_BLOCK_OPACITY: f32 = 1.0;
const MERGE_BASE_ONLY_MARKER_HEIGHT: f32 = 3.0;
const MERGE_VIRTUAL_ROW_THRESHOLD: usize = 2_000;
const MERGE_COLLAPSE_MIN_UNCHANGED_ROWS: usize = 12;
const MERGE_COLLAPSE_CONTEXT_ROWS: usize = 3;
const MERGE_BUILD_CONFIG: &str = include_str!("../config/merge-tool.toml");
const MERGE_AI_MAX_RELATED_FILE_BYTES: usize = 24 * 1024;
const MERGE_AI_MAX_CONTEXT_BYTES: usize = 96 * 1024;
const MERGE_AI_MAX_HISTORY_CHARS: usize = 12 * 1024;
const MERGE_AI_MAX_MANUAL_RESULT_CHARS: usize = 32 * 1024;
const MERGE_AI_MAX_MIDDLE_EDITS: usize = 12;
const MERGE_AI_MAX_MIDDLE_EDIT_EXPECTED_CHARS: usize = 512;
const MERGE_AI_MAX_MIDDLE_EDIT_REPLACEMENT_CHARS: usize = 8 * 1024;
const MERGE_AI_TARGET_CONTEXT_RADIUS: usize = 20;
const MERGE_AI_TOOL_NAME: &str = "submit_merge_suggestions";
const MERGE_AI_OVERLAY_DRAG_MARGIN: f32 = 8.0;
const MERGE_LOADING_CARD_WIDTH: f32 = 560.0;
const MERGE_LOADING_CARD_HEIGHT: f32 = 272.0;
#[cfg(target_os = "windows")]
const MERGE_WINDOWS_CREATE_NO_WINDOW: u32 = 0x0800_0000;
static MERGE_CONNECTOR_DEBUG_LOG_COUNT: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy)]
struct MergePalette {
    bg: Color32,
    panel: Color32,
    panel_soft: Color32,
    text: Color32,
    muted: Color32,
    accent: Color32,
    conflict_fill: Color32,
    active_conflict_fill: Color32,
    conflict_text: Color32,
    added_fill: Color32,
    added_text: Color32,
    base_only_fill: Color32,
    base_only_connector_fill: Color32,
    base_only_text: Color32,
    connector: Color32,
    result_fill: Color32,
    shadow: eframe::epaint::Shadow,
}

#[derive(Default)]
struct MergePanelGeometry {
    rows: Vec<(usize, Rect)>,
    horizontal_bounds: Option<(f32, f32)>,
}

#[derive(Clone, Copy)]
struct MergeConnectorColumns {
    local: Rect,
    result: Rect,
    remote: Rect,
}

impl MergePanelGeometry {
    fn record_row(&mut self, index: usize, rect: Rect) {
        self.rows.push((index, rect));
    }

    fn set_horizontal_bounds(&mut self, viewport: Rect) {
        self.horizontal_bounds = Some((viewport.left(), viewport.right()));
    }

    fn apply_horizontal_bounds(&self, rect: Rect) -> Rect {
        let Some((left, right)) = self.horizontal_bounds else {
            return rect;
        };
        Rect::from_min_max(Pos2::new(left, rect.top()), Pos2::new(right, rect.bottom()))
    }

    fn span_rect(&self, first: usize, count: usize) -> Option<Rect> {
        let last = first.saturating_add(count);
        self.rows
            .iter()
            .filter(|(index, _)| *index >= first && *index < last)
            .map(|(_, rect)| *rect)
            .reduce(|merged, rect| merged.union(rect))
            .map(|rect| self.apply_horizontal_bounds(rect))
    }

    fn boundary_marker_rect(&self, row_index: usize, height: f32) -> Option<Rect> {
        let next = self
            .rows
            .iter()
            .find(|(index, _)| *index == row_index)
            .map(|(_, rect)| *rect);
        let previous = row_index.checked_sub(1).and_then(|index| {
            self.rows
                .iter()
                .find(|(current, _)| *current == index)
                .map(|(_, rect)| *rect)
        });
        let reference = next.or(previous)?;
        let y = next.map_or(reference.bottom(), |rect| rect.top());
        Some(self.apply_horizontal_bounds(Rect::from_min_max(
            Pos2::new(reference.left(), y - height * 0.5),
            Pos2::new(reference.right(), y + height * 0.5),
        )))
    }
}

struct MergeSidePanelOutput {
    requested_result_scroll_y: Option<f32>,
    search_result_y: Option<f32>,
    navigation_target: Option<MergeLineActionTarget>,
    geometry: MergePanelGeometry,
    pending_line_action: Option<(MergeLineActionTarget, MergeLineAction)>,
}

struct MergeResultPanelOutput {
    scroll_y: f32,
    viewport_height: f32,
    search_result_y: Option<f32>,
    geometry: MergePanelGeometry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MergeAiOverlayAction {
    Apply(MergeLineActionTarget),
    Ignore(MergeLineActionTarget),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum MergeAiCardPlacement {
    Middle,
    Side(MergeSide),
}

#[derive(Clone, Copy, Debug)]
struct ConflictActionRects {
    take: Rect,
    drop: Rect,
}

#[derive(Clone, Copy, Debug)]
struct MergeSideDisplayRow<'a> {
    text: &'a str,
    reference_text: Option<&'a str>,
    line_number: Option<usize>,
    conflict_index: Option<usize>,
    side_resolved: bool,
    tone: MergeSideLineTone,
    show_conflict_actions: bool,
    action_target: Option<MergeLineActionTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CachedMergeSideDisplayRow {
    text: String,
    reference_text: Option<String>,
    line_number: Option<usize>,
    conflict_index: Option<usize>,
    side_resolved: bool,
    tone: MergeSideLineTone,
    show_conflict_actions: bool,
    action_target: Option<MergeLineActionTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CachedMergeResultDisplayRow {
    reference_text: Option<String>,
    conflict_index: Option<usize>,
    tone: MergeSideLineTone,
}

#[derive(Clone, Copy, Debug)]
struct MergeResultDisplayRow<'a> {
    text: &'a str,
    reference_text: Option<&'a str>,
    conflict_index: Option<usize>,
    tone: MergeSideLineTone,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BaseOnlyDisplayGroup {
    line_index: usize,
    line_count: usize,
    missing_side: MergeSide,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CachedConflictGeometry {
    result_span: Option<(usize, usize)>,
    result_boundary_row: Option<usize>,
    local_span: Option<(usize, usize)>,
    remote_span: Option<(usize, usize)>,
    tone: MergeSideLineTone,
}

impl Default for CachedConflictGeometry {
    fn default() -> Self {
        Self {
            result_span: None,
            result_boundary_row: None,
            local_span: None,
            remote_span: None,
            tone: MergeSideLineTone::Unchanged,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CachedBaseOnlyGeometry {
    group: BaseOnlyDisplayGroup,
    result_row: usize,
    side_boundary_row: usize,
    local_row: Option<usize>,
    remote_row: Option<usize>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct MergeGeometryCache {
    conflicts: HashMap<usize, CachedConflictGeometry>,
    base_only_groups: Vec<CachedBaseOnlyGeometry>,
}

#[derive(Clone, Debug, PartialEq)]
struct MergeEditSnapshot {
    document: MergeDocument,
    result_text: String,
    manual_result_lines: Vec<String>,
    manual_result_override: bool,
    local_conflict_cursor: usize,
    remote_conflict_cursor: usize,
    local_navigation_target: Option<MergeLineActionTarget>,
    remote_navigation_target: Option<MergeLineActionTarget>,
    ai_suggestions: HashMap<MergeLineActionTarget, MergeAiSuggestion>,
    ai_overlay_offsets: HashMap<(MergeLineActionTarget, MergeAiCardPlacement), Vec2>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MergeCancelRequest {
    ExitNow,
    ShowConfirm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MergeSideDiffRow<'a> {
    Equal(&'a str),
    Deleted(&'a str),
    Added(&'a str),
    Replaced(&'a str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MergeBaseLineState {
    Kept,
    Deleted,
    Replaced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MergeChangeSide {
    Local,
    Remote,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MergeChange {
    base_start: usize,
    base_end: usize,
    side_start: usize,
    side_end: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MergeBoundaryBias {
    Before,
    After,
}

pub fn parse_merge_args<I, S>(args: I) -> anyhow::Result<MergeArgs>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut items: Vec<String> = args.into_iter().map(Into::into).collect();
    if !items.is_empty() {
        items.remove(0);
    }

    let mut positional = Vec::new();
    let mut base = None;
    let mut local = None;
    let mut remote = None;
    let mut output = None;
    let mut repo_root = None;
    let mut stage = false;
    let mut theme = MergeTheme::Dark;
    let mut language = MergeLanguage::English;
    let mut ai_model_name = None;
    let mut iter = items.into_iter();

    while let Some(item) = iter.next() {
        if !item.starts_with("--") {
            positional.push(item);
            continue;
        }
        if item == "--stage" {
            stage = true;
            continue;
        }
        let Some(value) = iter.next() else {
            return Err(anyhow!("missing value for {item}"));
        };
        match item.as_str() {
            "--base" => base = Some(PathBuf::from(value)),
            "--local" => local = Some(PathBuf::from(value)),
            "--remote" | "--theirs" => remote = Some(PathBuf::from(value)),
            "--output" | "--merged" => output = Some(PathBuf::from(value)),
            "--repo-root" => repo_root = Some(PathBuf::from(value)),
            "--theme" => theme = parse_theme(&value)?,
            "--language" | "--lang" => language = parse_language(&value)?,
            "--ai-model" => ai_model_name = Some(value),
            other => return Err(anyhow!("unknown argument {other}")),
        }
    }

    if positional.len() == 4 {
        base.get_or_insert_with(|| PathBuf::from(&positional[0]));
        local.get_or_insert_with(|| PathBuf::from(&positional[1]));
        remote.get_or_insert_with(|| PathBuf::from(&positional[2]));
        output.get_or_insert_with(|| PathBuf::from(&positional[3]));
    } else if !positional.is_empty() {
        return Err(anyhow!("expected 4 positional paths"));
    }

    Ok(MergeArgs {
        base: base.context("missing --base")?,
        local: local.context("missing --local")?,
        remote: remote.context("missing --remote")?,
        output: output.context("missing --output")?,
        repo_root,
        stage,
        theme,
        language,
        ai_model_name,
    })
}

pub fn three_way_merge(base: &str, local: &str, remote: &str) -> MergeDocument {
    three_way_merge_with_options(base, local, remote, MergeIgnoreMode::None)
}

fn three_way_merge_with_options(
    base: &str,
    local: &str,
    remote: &str,
    ignore_mode: MergeIgnoreMode,
) -> MergeDocument {
    let base_lines = split_lines(base);
    let local_lines = split_lines(local);
    let remote_lines = split_lines(remote);
    if !merge_text_equal(base, remote, ignore_mode) && !base.is_empty() && local.is_empty() {
        return delete_modify_conflict_document(base_lines, local_lines, remote_lines);
    }
    if !merge_text_equal(base, local, ignore_mode) && !base.is_empty() && remote.is_empty() {
        return delete_modify_conflict_document(base_lines, local_lines, remote_lines);
    }
    merge_document_from_changes(&base_lines, &local_lines, &remote_lines, ignore_mode)
}

fn merge_document_from_changes(
    base_lines: &[String],
    local_lines: &[String],
    remote_lines: &[String],
    ignore_mode: MergeIgnoreMode,
) -> MergeDocument {
    let local_changes = diff_changes(base_lines, local_lines, ignore_mode);
    let remote_changes = diff_changes(base_lines, remote_lines, ignore_mode);
    let mut tagged_changes = local_changes
        .iter()
        .cloned()
        .map(|change| (MergeChangeSide::Local, change))
        .chain(
            remote_changes
                .iter()
                .cloned()
                .map(|change| (MergeChangeSide::Remote, change)),
        )
        .collect::<Vec<_>>();
    tagged_changes.sort_by_key(|(_, change)| (change.base_start, change.base_end));

    let mut lines = Vec::new();
    let mut conflicts = Vec::new();
    let mut base_cursor = 0;
    let mut change_index = 0;

    while change_index < tagged_changes.len() {
        let region_start = tagged_changes[change_index].1.base_start;
        push_resolved_lines(&mut lines, &base_lines[base_cursor..region_start]);

        let mut region_end = tagged_changes[change_index].1.base_end;
        let mut region_has_delete_only = tagged_changes[change_index].1.is_delete_only();
        change_index += 1;

        while change_index < tagged_changes.len() {
            let next = &tagged_changes[change_index].1;
            let overlaps = next.base_start < region_end
                || (next.base_start == region_end
                    && (region_start == region_end
                        || (region_has_delete_only && next.is_delete_only())));
            if !overlaps {
                break;
            }
            region_end = region_end.max(next.base_end);
            region_has_delete_only &= next.is_delete_only();
            change_index += 1;
        }

        push_merge_region(
            &mut lines,
            &mut conflicts,
            base_lines,
            local_lines,
            remote_lines,
            &local_changes,
            &remote_changes,
            region_start,
            region_end,
            ignore_mode,
        );
        base_cursor = region_end;
    }

    push_resolved_lines(&mut lines, &base_lines[base_cursor..]);

    MergeDocument { lines, conflicts }
}

#[allow(clippy::too_many_arguments)]
fn push_merge_region(
    lines: &mut Vec<MergeLine>,
    conflicts: &mut Vec<ConflictBlock>,
    base_lines: &[String],
    local_lines: &[String],
    remote_lines: &[String],
    local_changes: &[MergeChange],
    remote_changes: &[MergeChange],
    base_start: usize,
    base_end: usize,
    ignore_mode: MergeIgnoreMode,
) {
    let local_start =
        side_position_for_base_position(local_changes, base_start, MergeBoundaryBias::Before);
    let local_end = side_end_position_for_merge_region(local_changes, base_start, base_end);
    let remote_start =
        side_position_for_base_position(remote_changes, base_start, MergeBoundaryBias::Before);
    let remote_end = side_end_position_for_merge_region(remote_changes, base_start, base_end);
    let base_slice = &base_lines[base_start..base_end];
    let local_slice = &local_lines[local_start..local_end];
    let remote_slice = &remote_lines[remote_start..remote_end];

    if merge_lines_equal(local_slice, remote_slice, ignore_mode) {
        push_resolved_lines(lines, local_slice);
        return;
    }
    if merge_lines_equal(local_slice, base_slice, ignore_mode) {
        push_auto_resolved_side_region(lines, base_slice, remote_slice, MergeSide::Remote);
        return;
    }
    if merge_lines_equal(remote_slice, base_slice, ignore_mode) {
        push_auto_resolved_side_region(lines, base_slice, local_slice, MergeSide::Local);
        return;
    }

    push_conflict_region(lines, conflicts, base_slice, local_slice, remote_slice);
}

/// A zero-width insertion at a non-empty region's trailing boundary belongs to
/// the next merge region. Including it here turns an independent delete plus
/// insertion into a false conflict.
fn side_end_position_for_merge_region(
    changes: &[MergeChange],
    base_start: usize,
    base_end: usize,
) -> usize {
    let trailing_insertion = base_start < base_end
        && changes
            .iter()
            .any(|change| change.base_start == base_end && change.base_start == change.base_end);
    side_position_for_base_position(
        changes,
        base_end,
        if trailing_insertion {
            MergeBoundaryBias::Before
        } else {
            MergeBoundaryBias::After
        },
    )
}

fn push_resolved_lines(lines: &mut Vec<MergeLine>, result_lines: &[String]) {
    for result in result_lines {
        push_resolved_line(lines, result);
    }
}

fn push_resolved_line(lines: &mut Vec<MergeLine>, result: &str) {
    lines.push(MergeLine {
        base: Some(result.to_owned()),
        local: Some(result.to_owned()),
        remote: Some(result.to_owned()),
        result: result.to_owned(),
        include_in_result: true,
        kind: MergeLineKind::Resolved,
        conflict_index: None,
        local_resolved: true,
        remote_resolved: true,
        local_taken: false,
        remote_taken: false,
        base_only_resolved: false,
    });
}

fn push_auto_resolved_side_region(
    lines: &mut Vec<MergeLine>,
    base: &[String],
    side: &[String],
    changed_side: MergeSide,
) {
    for row in merge_diff_base_to_side(base, side) {
        match row {
            MergeSideDiffRow::Equal(text)
            | MergeSideDiffRow::Added(text)
            | MergeSideDiffRow::Replaced(text) => push_resolved_line(lines, text),
            MergeSideDiffRow::Deleted(text) => {
                push_base_only_display_line(lines, text, changed_side)
            }
        }
    }
}

fn push_base_only_display_line(lines: &mut Vec<MergeLine>, text: &str, changed_side: MergeSide) {
    let (local, remote) = match changed_side {
        MergeSide::Local => (None, Some(text.to_owned())),
        MergeSide::Remote => (Some(text.to_owned()), None),
    };
    lines.push(MergeLine {
        base: Some(text.to_owned()),
        local,
        remote,
        result: text.to_owned(),
        include_in_result: false,
        kind: MergeLineKind::Resolved,
        conflict_index: None,
        local_resolved: true,
        remote_resolved: true,
        local_taken: false,
        remote_taken: false,
        base_only_resolved: false,
    });
}

fn push_conflict_region(
    lines: &mut Vec<MergeLine>,
    conflicts: &mut Vec<ConflictBlock>,
    base: &[String],
    local: &[String],
    remote: &[String],
) {
    let conflict_index = conflicts.len();
    let max_len = base.len().max(local.len()).max(remote.len()).max(1);
    let mut line_indices = Vec::new();
    for index in 0..max_len {
        line_indices.push(lines.len());
        let base_line = base.get(index).cloned();
        lines.push(MergeLine {
            result: base_line.clone().unwrap_or_default(),
            include_in_result: false,
            base: base_line,
            local: local.get(index).cloned(),
            remote: remote.get(index).cloned(),
            kind: MergeLineKind::Conflict,
            conflict_index: Some(conflict_index),
            local_resolved: false,
            remote_resolved: false,
            local_taken: false,
            remote_taken: false,
            base_only_resolved: false,
        });
    }
    conflicts.push(ConflictBlock {
        index: conflict_index,
        base: base.to_vec(),
        local: local.to_vec(),
        remote: remote.to_vec(),
        line_indices,
    });
}

fn diff_changes(
    base: &[String],
    side: &[String],
    ignore_mode: MergeIgnoreMode,
) -> Vec<MergeChange> {
    let base_keys = base
        .iter()
        .map(|line| merge_line_key(line, ignore_mode))
        .collect::<Vec<_>>();
    let side_keys = side
        .iter()
        .map(|line| merge_line_key(line, ignore_mode))
        .collect::<Vec<_>>();

    let mut changes: Vec<MergeChange> = Vec::new();
    for operation in capture_diff_slices(Algorithm::Patience, &base_keys, &side_keys) {
        let (tag, base_range, side_range) = operation.as_tag_tuple();
        if tag == DiffTag::Equal {
            continue;
        }
        let change = MergeChange {
            base_start: base_range.start,
            base_end: base_range.end,
            side_start: side_range.start,
            side_end: side_range.end,
        };
        if let Some(previous) = changes.last_mut()
            && previous.base_end == change.base_start
            && previous.side_end == change.side_start
        {
            previous.base_end = change.base_end;
            previous.side_end = change.side_end;
        } else {
            changes.push(change);
        }
    }
    changes
}

fn merge_line_key(line: &str, ignore_mode: MergeIgnoreMode) -> Cow<'_, str> {
    match ignore_mode {
        MergeIgnoreMode::None => Cow::Borrowed(line),
        MergeIgnoreMode::TrimWhitespace => Cow::Borrowed(line.trim()),
        MergeIgnoreMode::IgnoreWhitespace => {
            Cow::Owned(line.chars().filter(|ch| !ch.is_whitespace()).collect())
        }
    }
}

fn merge_text_equal(left: &str, right: &str, ignore_mode: MergeIgnoreMode) -> bool {
    let left = split_lines(left);
    let right = split_lines(right);
    merge_lines_equal(&left, &right, ignore_mode)
}

fn merge_lines_equal(left: &[String], right: &[String], ignore_mode: MergeIgnoreMode) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| merge_line_equal(left, right, ignore_mode))
}

fn merge_line_equal(left: &str, right: &str, ignore_mode: MergeIgnoreMode) -> bool {
    match ignore_mode {
        MergeIgnoreMode::None => left == right,
        MergeIgnoreMode::TrimWhitespace => left.trim() == right.trim(),
        MergeIgnoreMode::IgnoreWhitespace => left
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .eq(right.chars().filter(|ch| !ch.is_whitespace())),
    }
}

fn side_position_for_base_position(
    changes: &[MergeChange],
    base_position: usize,
    bias: MergeBoundaryBias,
) -> usize {
    let mut base_cursor = 0;
    let mut side_cursor = 0;

    for change in changes {
        if base_position < change.base_start {
            return side_cursor + (base_position - base_cursor);
        }
        if base_position == change.base_start
            && change.base_start == change.base_end
            && bias == MergeBoundaryBias::Before
        {
            return side_cursor + (base_position - base_cursor);
        }
        if base_position == change.base_start && change.base_start < change.base_end {
            return side_cursor + (base_position - base_cursor);
        }

        if base_position < change.base_end {
            return match bias {
                MergeBoundaryBias::Before => change.side_start,
                MergeBoundaryBias::After => change.side_end,
            };
        }

        side_cursor = change.side_end;
        base_cursor = change.base_end;

        if base_position == change.base_end && bias == MergeBoundaryBias::Before {
            return side_cursor;
        }
    }

    side_cursor + (base_position - base_cursor)
}

impl MergeChange {
    fn is_delete_only(&self) -> bool {
        self.side_start == self.side_end && self.base_start < self.base_end
    }
}

fn delete_modify_conflict_document(
    base_lines: Vec<String>,
    local_lines: Vec<String>,
    remote_lines: Vec<String>,
) -> MergeDocument {
    let max_len = base_lines
        .len()
        .max(local_lines.len())
        .max(remote_lines.len());
    let line_indices = (0..max_len).collect::<Vec<_>>();
    let lines = (0..max_len)
        .map(|index| {
            let base = base_lines.get(index).cloned();
            let local = local_lines.get(index).cloned();
            let remote = remote_lines.get(index).cloned();
            MergeLine {
                result: base.clone().unwrap_or_default(),
                include_in_result: false,
                base,
                local,
                remote,
                kind: MergeLineKind::Conflict,
                conflict_index: Some(0),
                local_resolved: false,
                remote_resolved: false,
                local_taken: false,
                remote_taken: false,
                base_only_resolved: false,
            }
        })
        .collect();
    MergeDocument {
        lines,
        conflicts: vec![ConflictBlock {
            index: 0,
            base: base_lines,
            local: local_lines,
            remote: remote_lines,
            line_indices,
        }],
    }
}

impl MergeDocument {
    pub fn conflicts(&self) -> &[ConflictBlock] {
        &self.conflicts
    }

    pub fn result_text(&self) -> String {
        let mut text = self
            .lines
            .iter()
            .filter(|line| line.include_in_result)
            .flat_map(MergeLine::result_lines)
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        text
    }

    fn apply_ai_manual_target(
        &mut self,
        target: MergeLineActionTarget,
        replacement: &str,
    ) -> Result<(), String> {
        let line_indices = match target {
            MergeLineActionTarget::Conflict(index) => self
                .conflicts
                .get(index)
                .map(|conflict| conflict.line_indices.clone())
                .ok_or_else(|| format!("conflict target {index} is no longer available"))?,
            MergeLineActionTarget::BaseOnlyGroup(line_index) => {
                let group = base_only_display_groups(self)
                    .into_iter()
                    .find(|group| group.line_index == line_index)
                    .ok_or_else(|| {
                        format!("deletion target {line_index} is no longer available")
                    })?;
                (group.line_index..group.line_index + group.line_count).collect()
            }
        };
        let Some(first_index) = line_indices.first().copied() else {
            return Err("AI manual target has no document lines".to_owned());
        };
        let replacement = normalize_merge_ai_code(replacement);
        for line_index in line_indices {
            let Some(line) = self.lines.get_mut(line_index) else {
                return Err("AI manual target moved before it could be applied".to_owned());
            };
            line.local_resolved = true;
            line.remote_resolved = true;
            line.local_taken = false;
            line.remote_taken = false;
            line.base_only_resolved = true;
            line.kind = MergeLineKind::Resolved;
            line.include_in_result = false;
            line.result.clear();
        }
        let first = &mut self.lines[first_index];
        first.result = replacement;
        first.include_in_result = !first.result.is_empty();
        Ok(())
    }

    fn ai_middle_edit_location(&self, edit: &MergeAiMiddleEdit) -> Result<(usize, usize), String> {
        if edit.expected_text.contains('\r') || edit.expected_text.contains('\n') {
            return Err("AI middle edit expected_text must fit within one logical line".to_owned());
        }
        let expected = edit.expected_text.as_str();
        if expected.is_empty() {
            return Err("AI middle edit expected_text is empty".to_owned());
        }
        let matches = self
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| {
                line.conflict_index.is_none()
                    && line.kind == MergeLineKind::Resolved
                    && line.include_in_result
                    && !line.is_base_only_display()
            })
            .flat_map(|(line_index, line)| {
                line.result
                    .match_indices(expected)
                    .map(move |(offset, _)| (line_index, offset))
            })
            .collect::<Vec<_>>();
        let [(line_index, offset)] = matches.as_slice() else {
            return Err(format!(
                "AI middle edit expected one unique match for {:?}, found {}",
                expected,
                matches.len()
            ));
        };
        Ok((*line_index, *offset))
    }

    fn apply_ai_middle_edits<'a>(
        &mut self,
        edits: impl IntoIterator<Item = &'a MergeAiMiddleEdit>,
    ) -> Result<(), String> {
        // Resolve every anchor against the same original Middle result. Shared edits are
        // applied once; disjoint edits on one line must not shift each other's offsets.
        let mut seen = HashSet::new();
        let mut replacements = Vec::new();
        for edit in edits {
            let replacement = normalize_merge_ai_code(&edit.replacement_text);
            if !seen.insert((edit.expected_text.as_str(), replacement.clone())) {
                continue;
            }
            let (line_index, offset) = self.ai_middle_edit_location(edit)?;
            replacements.push((
                line_index,
                offset,
                offset + edit.expected_text.len(),
                replacement,
            ));
        }
        replacements.sort_by_key(|(line, start, _, _)| (*line, *start));
        for pair in replacements.windows(2) {
            if pair[0].0 == pair[1].0 && pair[0].2 > pair[1].1 {
                return Err("AI suggestions contain overlapping Middle edits".to_owned());
            }
        }
        for (line_index, start, end, replacement) in replacements.into_iter().rev() {
            let line = &mut self.lines[line_index];
            line.result.replace_range(start..end, &replacement);
            line.include_in_result = !line.result.is_empty();
        }
        Ok(())
    }

    fn apply_conflict(&mut self, index: usize, side: MergeSide) {
        self.accept_conflict_side_only(index, side);
    }

    pub fn accept_conflict_side_only(&mut self, index: usize, side: MergeSide) {
        let Some(conflict) = self.conflicts.get(index).cloned() else {
            return;
        };
        for line_index in conflict.line_indices {
            if let Some(line) = self.lines.get_mut(line_index) {
                match side {
                    MergeSide::Local => {
                        line.set_side(MergeSide::Local, true);
                        line.set_side(MergeSide::Remote, false);
                    }
                    MergeSide::Remote => {
                        line.set_side(MergeSide::Remote, true);
                        line.set_side(MergeSide::Local, false);
                    }
                }
            }
        }
    }

    pub fn take_conflict_side(&mut self, index: usize, side: MergeSide) {
        self.set_conflict_side(index, side, MergeLineAction::Take);
    }

    pub fn drop_conflict_side(&mut self, index: usize, side: MergeSide) {
        self.set_conflict_side(index, side, MergeLineAction::Drop);
    }

    fn take_base_only_group(&mut self, line_index: usize, side: MergeSide) {
        self.set_base_only_group(line_index, side, MergeLineAction::Take);
    }

    fn drop_base_only_group(&mut self, line_index: usize, side: MergeSide) {
        self.set_base_only_group(line_index, side, MergeLineAction::Drop);
    }

    pub fn unresolved_conflict_count(&self) -> usize {
        self.conflicts
            .iter()
            .filter(|conflict| {
                self.conflict_side_unresolved(conflict.index, MergeSide::Local)
                    || self.conflict_side_unresolved(conflict.index, MergeSide::Remote)
            })
            .count()
    }

    pub fn unresolved_conflict_count_for_side(&self, side: MergeSide) -> usize {
        self.conflicts
            .iter()
            .filter(|conflict| self.conflict_side_unresolved(conflict.index, side))
            .count()
    }

    fn conflict_fully_resolved(&self, index: usize) -> bool {
        !self.conflict_side_unresolved(index, MergeSide::Local)
            && !self.conflict_side_unresolved(index, MergeSide::Remote)
    }

    pub fn conflict_side_resolved(&self, index: usize, side: MergeSide) -> bool {
        !self.conflict_side_unresolved(index, side)
    }

    fn set_conflict_side(&mut self, index: usize, side: MergeSide, action: MergeLineAction) {
        let Some(conflict) = self.conflicts.get(index).cloned() else {
            return;
        };
        for line_index in conflict.line_indices {
            if let Some(line) = self.lines.get_mut(line_index) {
                line.set_side(side, action == MergeLineAction::Take);
            }
        }
    }

    fn set_base_only_group(&mut self, line_index: usize, side: MergeSide, action: MergeLineAction) {
        if !self.line_is_base_only_missing_side(line_index, side) {
            return;
        }

        let mut start = line_index;
        while start > 0 && self.line_is_base_only_missing_side(start - 1, side) {
            start -= 1;
        }

        let mut end = line_index;
        while end + 1 < self.lines.len() && self.line_is_base_only_missing_side(end + 1, side) {
            end += 1;
        }

        let include_base = action == MergeLineAction::Drop;
        for line in &mut self.lines[start..=end] {
            line.include_in_result = include_base;
            line.base_only_resolved = true;
        }
    }

    fn line_is_base_only_missing_side(&self, line_index: usize, side: MergeSide) -> bool {
        self.lines
            .get(line_index)
            .and_then(MergeLine::base_only_missing_side)
            == Some(side)
    }

    fn conflict_side_unresolved(&self, index: usize, side: MergeSide) -> bool {
        let Some(conflict) = self.conflicts.get(index) else {
            return false;
        };
        conflict.line_indices.iter().any(|line_index| {
            self.lines
                .get(*line_index)
                .is_some_and(|line| !line.side_resolved(side))
        })
    }
}

impl MergeLine {
    fn is_base_only_display(&self) -> bool {
        self.kind == MergeLineKind::Resolved
            && !self.include_in_result
            && !self.base_only_resolved
            && self.base_only_missing_side_raw().is_some()
    }

    fn base_only_missing_side(&self) -> Option<MergeSide> {
        if !self.is_base_only_display() {
            return None;
        }
        self.base_only_missing_side_raw()
    }

    fn base_only_missing_side_raw(&self) -> Option<MergeSide> {
        self.base.as_ref()?;
        match (self.local.is_none(), self.remote.is_none()) {
            (true, false) => Some(MergeSide::Local),
            (false, true) => Some(MergeSide::Remote),
            _ => None,
        }
    }

    fn result_lines(&self) -> Vec<&str> {
        let mut lines = Vec::new();
        if self.local_taken {
            if let Some(local) = &self.local {
                lines.push(local.as_str());
            }
        }
        if self.remote_taken {
            if let Some(remote) = &self.remote {
                lines.push(remote.as_str());
            }
        }
        if self.local_taken || self.remote_taken {
            return lines;
        }
        if lines.is_empty() && self.include_in_result {
            if self.result.is_empty() {
                lines.push("");
            } else {
                lines.extend(self.result.split_terminator('\n'));
            }
        }
        lines
    }

    fn side_resolved(&self, side: MergeSide) -> bool {
        match side {
            MergeSide::Local => self.local_resolved,
            MergeSide::Remote => self.remote_resolved,
        }
    }

    fn set_side(&mut self, side: MergeSide, take: bool) {
        match side {
            MergeSide::Local => {
                self.local_resolved = true;
                self.local_taken = take;
            }
            MergeSide::Remote => {
                self.remote_resolved = true;
                self.remote_taken = take;
            }
        }
        self.kind = if self.local_resolved && self.remote_resolved {
            MergeLineKind::Resolved
        } else {
            MergeLineKind::Conflict
        };
        self.include_in_result = if self.conflict_index.is_some() {
            self.local_taken || self.remote_taken
        } else {
            self.kind != MergeLineKind::Conflict
        };
    }
}

pub struct MergeToolApp {
    args: MergeArgs,
    sources: Option<MergeSourceText>,
    initial_document: MergeDocument,
    document: MergeDocument,
    result_text: String,
    manual_result_lines: Vec<String>,
    result_display_rows: Vec<CachedMergeResultDisplayRow>,
    local_display_rows: Vec<CachedMergeSideDisplayRow>,
    remote_display_rows: Vec<CachedMergeSideDisplayRow>,
    geometry_cache: MergeGeometryCache,
    local_scroll_anchors: Vec<(f32, f32)>,
    remote_scroll_anchors: Vec<(f32, f32)>,
    manual_result_override: bool,
    shared_scroll_x: f32,
    shared_scroll_y: f32,
    local_conflict_cursor: usize,
    remote_conflict_cursor: usize,
    local_navigation_target: Option<MergeLineActionTarget>,
    remote_navigation_target: Option<MergeLineActionTarget>,
    theme: MergeTheme,
    language: MergeLanguage,
    status: Option<String>,
    load_task: Option<Receiver<MergeLoadEvent>>,
    load_progress: MergeLoadProgress,
    load_started_at: Option<Instant>,
    write_task: Option<Receiver<anyhow::Result<()>>>,
    ai_task: Option<Receiver<Result<Vec<MergeAiSuggestion>, String>>>,
    ai_suggestions: HashMap<MergeLineActionTarget, MergeAiSuggestion>,
    ai_middle_edit_rows: HashMap<String, Option<usize>>,
    ai_overlay_offsets: HashMap<(MergeLineActionTarget, MergeAiCardPlacement), Vec2>,
    ai_logged_missing_anchors: HashSet<(MergeLineActionTarget, MergeSide)>,
    ai_notice: Option<MergeAiNotice>,
    ai_analysis_error: Option<String>,
    undo_stack: Vec<MergeEditSnapshot>,
    redo_stack: Vec<MergeEditSnapshot>,
    display_epoch: u64,
    show_cancel_confirm: bool,
    connector_debug: MergeConnectorDebug,
    ignore_mode: MergeIgnoreMode,
    highlight_mode: MergeHighlightMode,
    collapse_unchanged: bool,
    search: MergeSearchState,
    hovered_search_pane: Option<MergeSearchPane>,
    syntax_highlights: MergeSyntaxHighlights,
    result_highlight_task: Option<Receiver<(u64, Option<HighlightedDocument>)>>,
    result_highlight_revision: u64,
    result_highlight_due: Option<Instant>,
    frame_window_started: Instant,
    frame_count: u64,
    frame_total: Duration,
    frame_max: Duration,
    frame_logged_once: bool,
}

impl MergeToolApp {
    pub fn from_args(args: MergeArgs) -> anyhow::Result<Self> {
        let prepared = load_merge_document(&args, |_| {})?;
        Ok(Self::from_prepared(args, prepared))
    }

    fn loading(args: MergeArgs) -> Self {
        let mut app = Self::new(args.clone(), three_way_merge("", "", ""));
        let (sender, receiver) = mpsc::channel();
        app.load_task = Some(receiver);
        app.load_started_at = Some(Instant::now());
        thread::spawn(move || {
            let progress_sender = sender.clone();
            let result = load_merge_document(&args, |progress| {
                let _ = progress_sender.send(MergeLoadEvent::Progress(progress));
            });
            let _ = sender.send(MergeLoadEvent::Finished(result));
        });
        app
    }

    pub fn new(args: MergeArgs, document: MergeDocument) -> Self {
        let result_text = document.result_text();
        let manual_result_lines = merge_result_display_rows(&document)
            .into_iter()
            .map(|row| row.text.to_owned())
            .collect::<Vec<_>>();
        let result_highlight = highlight_merge_source(
            &args,
            &merge_highlight_source_from_lines(&manual_result_lines),
        );
        let local_display_rows = cached_merge_side_display_rows(&document, MergeSide::Local);
        let remote_display_rows = cached_merge_side_display_rows(&document, MergeSide::Remote);
        let result_display_rows = merge_result_display_rows(&document);
        let local_scroll_anchors = merge_cached_scroll_anchors(
            &document,
            MergeSide::Local,
            &result_display_rows,
            &local_display_rows,
        );
        let remote_scroll_anchors = merge_cached_scroll_anchors(
            &document,
            MergeSide::Remote,
            &result_display_rows,
            &remote_display_rows,
        );
        let result_display_rows = cached_merge_result_display_rows(&result_display_rows);
        let geometry_cache = merge_geometry_cache(
            &document,
            &result_display_rows,
            &local_display_rows,
            &remote_display_rows,
        );
        let local_navigation_target = merge_navigation_targets(&document, MergeSide::Local)
            .first()
            .copied();
        let remote_navigation_target = merge_navigation_targets(&document, MergeSide::Remote)
            .first()
            .copied();
        let initial_document = document.clone();
        Self {
            theme: args.theme,
            language: args.language,
            args,
            sources: None,
            initial_document,
            document,
            result_text,
            manual_result_lines,
            result_display_rows,
            local_display_rows,
            remote_display_rows,
            geometry_cache,
            local_scroll_anchors,
            remote_scroll_anchors,
            manual_result_override: false,
            shared_scroll_x: 0.0,
            shared_scroll_y: 0.0,
            local_conflict_cursor: 0,
            remote_conflict_cursor: 0,
            local_navigation_target,
            remote_navigation_target,
            status: None,
            load_task: None,
            load_progress: MergeLoadProgress {
                stage: MergeLoadStage::ReadingFiles,
                total_bytes: 0,
                total_lines: 0,
            },
            load_started_at: None,
            write_task: None,
            ai_task: None,
            ai_suggestions: HashMap::new(),
            ai_middle_edit_rows: HashMap::new(),
            ai_overlay_offsets: HashMap::new(),
            ai_logged_missing_anchors: HashSet::new(),
            ai_notice: None,
            ai_analysis_error: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            display_epoch: 0,
            show_cancel_confirm: false,
            connector_debug: merge_connector_debug_mode(),
            ignore_mode: MergeIgnoreMode::None,
            highlight_mode: MergeHighlightMode::Lines,
            collapse_unchanged: false,
            search: MergeSearchState::default(),
            hovered_search_pane: None,
            syntax_highlights: MergeSyntaxHighlights {
                result: result_highlight,
                ..Default::default()
            },
            result_highlight_task: None,
            result_highlight_revision: 0,
            result_highlight_due: None,
            frame_window_started: Instant::now(),
            frame_count: 0,
            frame_total: Duration::ZERO,
            frame_max: Duration::ZERO,
            frame_logged_once: false,
        }
    }

    fn from_prepared(args: MergeArgs, prepared: PreparedMergeDocument) -> Self {
        Self {
            theme: args.theme,
            language: args.language,
            args,
            sources: Some(prepared.sources),
            initial_document: prepared.initial_document,
            document: prepared.document,
            result_text: prepared.result_text,
            manual_result_lines: prepared.manual_result_lines,
            result_display_rows: prepared.result_display_rows,
            local_display_rows: prepared.local_display_rows,
            remote_display_rows: prepared.remote_display_rows,
            geometry_cache: prepared.geometry_cache,
            local_scroll_anchors: prepared.local_scroll_anchors,
            remote_scroll_anchors: prepared.remote_scroll_anchors,
            manual_result_override: false,
            shared_scroll_x: 0.0,
            shared_scroll_y: 0.0,
            local_conflict_cursor: 0,
            remote_conflict_cursor: 0,
            local_navigation_target: prepared.local_navigation_target,
            remote_navigation_target: prepared.remote_navigation_target,
            status: None,
            load_task: None,
            load_progress: MergeLoadProgress {
                stage: MergeLoadStage::PreparingEditor,
                total_bytes: 0,
                total_lines: 0,
            },
            load_started_at: None,
            write_task: None,
            ai_task: None,
            ai_suggestions: HashMap::new(),
            ai_middle_edit_rows: HashMap::new(),
            ai_overlay_offsets: HashMap::new(),
            ai_logged_missing_anchors: HashSet::new(),
            ai_notice: None,
            ai_analysis_error: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            display_epoch: 0,
            show_cancel_confirm: false,
            connector_debug: merge_connector_debug_mode(),
            ignore_mode: MergeIgnoreMode::None,
            highlight_mode: MergeHighlightMode::Lines,
            collapse_unchanged: false,
            search: MergeSearchState::default(),
            hovered_search_pane: None,
            syntax_highlights: prepared.syntax_highlights,
            result_highlight_task: None,
            result_highlight_revision: 0,
            result_highlight_due: None,
            frame_window_started: Instant::now(),
            frame_count: 0,
            frame_total: Duration::ZERO,
            frame_max: Duration::ZERO,
            frame_logged_once: false,
        }
    }

    pub fn run_from_env() -> eframe::Result<()> {
        let args = match parse_merge_args(env::args()) {
            Ok(args) => args,
            Err(error) => {
                eprintln!(
                    "Usage: git-agent-merge --base <base> --local <local> --remote <remote> --output <merged> [--repo-root <repo> --stage] [--theme dark|light] [--language en|zh]\n{error}"
                );
                std::process::exit(2);
            }
        };
        let title = format!("Git Agent Merge - {}", args.output.display());
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title(title.clone())
                .with_icon(merge_app_icon_data())
                .with_decorations(false)
                .with_transparent(true)
                .with_inner_size([1180.0, 760.0])
                .with_min_inner_size([980.0, 620.0]),
            ..Default::default()
        };
        eframe::run_native(
            &title,
            options,
            Box::new(move |cc| {
                prefer_rounded_merge_window_corners(cc);
                crate::theme::install(&cc.egui_ctx);
                // The standalone merge binary has its own egui context. Register SVG loaders here
                // as well, otherwise toolbar icons render as the missing-image warning glyph.
                egui_extras::install_image_loaders(&cc.egui_ctx);
                apply_merge_theme(&cc.egui_ctx, args.theme);
                let app = Self::loading(args);
                Ok(Box::new(app))
            }),
        )
    }

    fn snapshot(&self) -> MergeEditSnapshot {
        MergeEditSnapshot {
            document: self.document.clone(),
            result_text: self.result_text.clone(),
            manual_result_lines: self.manual_result_lines.clone(),
            manual_result_override: self.manual_result_override,
            local_conflict_cursor: self.local_conflict_cursor,
            remote_conflict_cursor: self.remote_conflict_cursor,
            local_navigation_target: self.local_navigation_target,
            remote_navigation_target: self.remote_navigation_target,
            ai_suggestions: self.ai_suggestions.clone(),
            ai_overlay_offsets: self.ai_overlay_offsets.clone(),
        }
    }

    fn restore_snapshot(&mut self, snapshot: MergeEditSnapshot) {
        self.document = snapshot.document;
        self.result_text = snapshot.result_text;
        self.manual_result_lines = snapshot.manual_result_lines;
        self.manual_result_override = snapshot.manual_result_override;
        self.local_conflict_cursor = snapshot.local_conflict_cursor;
        self.remote_conflict_cursor = snapshot.remote_conflict_cursor;
        self.local_navigation_target = snapshot.local_navigation_target;
        self.remote_navigation_target = snapshot.remote_navigation_target;
        self.ai_suggestions = snapshot.ai_suggestions;
        self.ai_overlay_offsets = snapshot.ai_overlay_offsets;
        self.ai_logged_missing_anchors.clear();
        self.rebuild_display_rows();
        self.schedule_result_highlight();
    }

    fn reset_from_sources(&mut self, ignore_mode: MergeIgnoreMode) {
        let Some(sources) = self.sources.as_ref() else {
            return;
        };
        self.ignore_mode = ignore_mode;
        self.document = three_way_merge_with_options(
            &sources.base,
            &sources.local,
            &sources.remote,
            ignore_mode,
        );
        self.initial_document = self.document.clone();
        self.result_text = self.document.result_text();
        self.manual_result_lines = merge_result_display_rows(&self.document)
            .into_iter()
            .map(|row| row.text.to_owned())
            .collect();
        self.manual_result_override = false;
        self.shared_scroll_x = 0.0;
        self.shared_scroll_y = 0.0;
        self.local_conflict_cursor = 0;
        self.remote_conflict_cursor = 0;
        self.local_navigation_target = merge_navigation_targets(&self.document, MergeSide::Local)
            .first()
            .copied();
        self.remote_navigation_target = merge_navigation_targets(&self.document, MergeSide::Remote)
            .first()
            .copied();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.rebuild_display_rows();
        self.schedule_result_highlight();
    }

    fn finish_document_edit(&mut self, before: MergeEditSnapshot) {
        self.rebuild_display_rows();
        if self.snapshot() != before {
            self.undo_stack.push(before);
            self.redo_stack.clear();
            self.schedule_result_highlight();
        }
    }

    fn has_unsaved_edits(&self) -> bool {
        self.document != self.initial_document
            || self.result_text != self.initial_document.result_text()
    }

    fn unresolved_conflict_count(&self) -> usize {
        if self.manual_result_override {
            0
        } else {
            self.document.unresolved_conflict_count()
        }
    }

    fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    fn can_apply_result(&self) -> bool {
        self.write_task.is_none() && self.unresolved_conflict_count() == 0
    }

    fn request_ai_analysis(&mut self) {
        if self.ai_task.is_some()
            || (self.document.conflicts().is_empty()
                && base_only_display_groups(&self.document).is_empty())
        {
            crate::diagnostics::merge_ai_trace(
                "request.skipped",
                "reason=already_running_or_no_targets",
            );
            return;
        }
        let Some(sources) = self.sources.clone() else {
            let error = mt(self.language, "ai_sources_unavailable").to_owned();
            crate::diagnostics::merge_ai_trace(
                "request.failed",
                &format!("stage=sources error={error}"),
            );
            self.ai_analysis_error = Some(error);
            return;
        };
        let args = self.args.clone();
        let document = self.document.clone();
        crate::diagnostics::merge_ai_trace(
            "request.clicked",
            &format!(
                "model_name={} output={} conflicts={} deletions={} language={:?}",
                args.ai_model_name.as_deref().unwrap_or("<selected>"),
                args.output.display(),
                document.conflicts().len(),
                base_only_display_groups(&document).len(),
                args.language,
            ),
        );
        let (sender, receiver) = mpsc::channel();
        self.ai_suggestions.clear();
        self.ai_middle_edit_rows.clear();
        self.ai_overlay_offsets.clear();
        self.ai_logged_missing_anchors.clear();
        self.ai_task = Some(receiver);
        self.ai_notice = None;
        self.ai_analysis_error = None;
        thread::spawn(move || {
            let result = crate::app::load_merge_ai_model_config(args.ai_model_name.as_deref())
                .and_then(|config| {
                    crate::diagnostics::merge_ai_trace(
                        "config.loaded",
                        &format!(
                            "name={} format={:?} base_url={} model_id={} api_key_present={}",
                            config.name,
                            config.api_format,
                            merge_ai_url_for_log(&config.base_url),
                            config.model_id,
                            !config.api_key.trim().is_empty(),
                        ),
                    );
                    let context = collect_merge_ai_context(&args, &sources, &document)?;
                    crate::diagnostics::merge_ai_trace(
                        "context.collected",
                        &format!(
                            "history_chars={} repository_state_chars={} related_chars={} symbol_reference_chars={} base_chars={} local_chars={} remote_chars={}",
                            context.history.chars().count(),
                            context.repository_state.chars().count(),
                            context.related_files.chars().count(),
                            context.symbol_references.chars().count(),
                            sources.base.chars().count(),
                            sources.local.chars().count(),
                            sources.remote.chars().count(),
                        ),
                    );
                    request_merge_ai_suggestions(&config, &args, &sources, &document, &context)
                });
            match &result {
                Ok(suggestions) => crate::diagnostics::merge_ai_trace(
                    "request.finished",
                    &format!("accepted_suggestions={}", suggestions.len()),
                ),
                Err(error) => {
                    crate::diagnostics::merge_ai_trace("request.failed", &format!("error={error}"))
                }
            }
            let _ = sender.send(result);
        });
    }

    fn poll_ai_task(&mut self, ctx: &egui::Context) {
        let Some(receiver) = self.ai_task.take() else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(suggestions)) => {
                let suggestion_count = suggestions.len();
                let change_count = suggestions
                    .iter()
                    .map(MergeAiSuggestion::change_count)
                    .sum::<usize>();
                self.ai_suggestions = suggestions
                    .into_iter()
                    .map(|suggestion| (suggestion.target, suggestion))
                    .collect();
                self.rebuild_ai_middle_edit_rows();
                self.ai_overlay_offsets.clear();
                self.ai_logged_missing_anchors.clear();
                self.ai_notice = Some(if suggestion_count == 0 {
                    MergeAiNotice::NoSuggestions
                } else {
                    MergeAiNotice::Completed {
                        suggestions: suggestion_count,
                        changes: change_count,
                    }
                });
                self.ai_analysis_error = None;
                crate::diagnostics::merge_ai_trace(
                    "ui.received",
                    &format!("suggestions={suggestion_count} changes={change_count}"),
                );
                ctx.request_repaint();
            }
            Ok(Err(error)) => {
                crate::diagnostics::merge_ai_trace("ui.error", &format!("error={error}"));
                self.ai_analysis_error = Some(error);
                ctx.request_repaint();
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.ai_task = Some(receiver);
                ctx.request_repaint_after(Duration::from_millis(80));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                let error = mt(self.language, "ai_analysis_stopped").to_owned();
                crate::diagnostics::merge_ai_trace(
                    "ui.error",
                    &format!("stage=channel_disconnected error={error}"),
                );
                self.ai_analysis_error = Some(error);
                ctx.request_repaint();
            }
        }
    }

    fn apply_ai_suggestion(&mut self, target: MergeLineActionTarget) {
        self.apply_ai_suggestions(&[target]);
    }

    fn ai_suggestion_actions_enabled(&self) -> bool {
        self.load_task.is_none()
            && self.write_task.is_none()
            && self.ai_task.is_none()
            && !self.manual_result_override
    }

    fn actionable_ai_suggestion_count(&self) -> usize {
        self.ai_suggestions
            .values()
            .filter(|suggestion| suggestion.is_actionable())
            .count()
    }

    fn apply_all_ai_suggestions(&mut self) {
        let mut targets = self
            .ai_suggestions
            .values()
            .filter(|suggestion| suggestion.is_actionable())
            .map(|suggestion| suggestion.target)
            .collect::<Vec<_>>();
        targets.sort_by_key(|target| match target {
            MergeLineActionTarget::Conflict(index) => (0, *index),
            MergeLineActionTarget::BaseOnlyGroup(index) => (1, *index),
        });
        self.apply_ai_suggestions(&targets);
    }

    fn apply_ai_suggestions(&mut self, targets: &[MergeLineActionTarget]) {
        if !self.ai_suggestion_actions_enabled() || targets.is_empty() {
            return;
        }
        let Some(suggestions) = targets
            .iter()
            .map(|target| self.ai_suggestions.get(target).cloned())
            .collect::<Option<Vec<_>>>()
        else {
            return;
        };
        let mut updated_document = self.document.clone();
        let apply_result = (|| -> Result<(), String> {
            updated_document.apply_ai_middle_edits(
                suggestions
                    .iter()
                    .flat_map(|suggestion| &suggestion.middle_edits),
            )?;
            for suggestion in &suggestions {
                let target = suggestion.target;
                if let MergeLineActionTarget::Conflict(index) = target {
                    if updated_document.conflicts.get(index).is_none()
                        || (!updated_document.conflict_side_unresolved(index, MergeSide::Local)
                            && !updated_document.conflict_side_unresolved(index, MergeSide::Remote))
                    {
                        return Err(format!("conflict target {index} is no longer available"));
                    }
                }
                match suggestion.choice {
                    MergeAiChoice::Local | MergeAiChoice::Remote => {
                        let chosen_side = if suggestion.choice == MergeAiChoice::Local {
                            MergeSide::Local
                        } else {
                            MergeSide::Remote
                        };
                        match target {
                            MergeLineActionTarget::Conflict(conflict_index) => {
                                updated_document.apply_conflict(conflict_index, chosen_side)
                            }
                            MergeLineActionTarget::BaseOnlyGroup(line_index) => {
                                let group = base_only_display_groups(&updated_document)
                                    .into_iter()
                                    .find(|group| group.line_index == line_index)
                                    .ok_or_else(|| {
                                        format!(
                                            "deletion target {line_index} is no longer available"
                                        )
                                    })?;
                                if chosen_side == group.missing_side {
                                    updated_document
                                        .take_base_only_group(line_index, group.missing_side);
                                } else {
                                    updated_document
                                        .drop_base_only_group(line_index, group.missing_side);
                                }
                            }
                        }
                    }
                    MergeAiChoice::Manual => {
                        let replacement = suggestion.manual_result.as_deref().ok_or_else(|| {
                            "manual AI suggestion does not contain an applicable result".to_owned()
                        })?;
                        updated_document.apply_ai_manual_target(target, replacement)?;
                    }
                }
            }
            Ok(())
        })();
        if let Err(error) = apply_result {
            crate::diagnostics::merge_ai_trace(
                "ui.apply_rejected",
                &format!("targets={targets:?} error={error}"),
            );
            self.ai_analysis_error = Some(error);
            return;
        }
        let before = self.snapshot();
        self.document = updated_document;
        let applied_targets = targets.iter().copied().collect::<HashSet<_>>();
        self.ai_suggestions
            .retain(|target, _| !applied_targets.contains(target));
        let applied_middle_edits = suggestions
            .iter()
            .flat_map(|suggestion| &suggestion.middle_edits)
            .map(|edit| {
                (
                    edit.expected_text.clone(),
                    normalize_merge_ai_code(&edit.replacement_text),
                )
            })
            .collect::<HashSet<_>>();
        // Applying one card and then "all" must not repeat a shared companion edit.
        for suggestion in self.ai_suggestions.values_mut() {
            suggestion.middle_edits.retain(|edit| {
                !applied_middle_edits.contains(&(
                    edit.expected_text.clone(),
                    normalize_merge_ai_code(&edit.replacement_text),
                ))
            });
        }
        self.ai_overlay_offsets
            .retain(|(current, _), _| !applied_targets.contains(current));
        self.ai_logged_missing_anchors
            .retain(|(current, _)| !applied_targets.contains(current));
        self.ai_analysis_error = None;
        self.result_text = self.document.result_text();
        self.manual_result_lines = merge_result_display_rows(&self.document)
            .into_iter()
            .map(|row| row.text.to_owned())
            .collect();
        self.finish_document_edit(before);
        crate::diagnostics::merge_ai_trace(
            "ui.applied",
            &format!("targets={targets:?} suggestions={}", suggestions.len()),
        );
    }

    fn ignore_ai_suggestion(&mut self, target: MergeLineActionTarget) {
        self.ai_suggestions.remove(&target);
        self.rebuild_ai_middle_edit_rows();
        self.ai_overlay_offsets
            .retain(|(current, _), _| *current != target);
    }

    fn undo(&mut self) -> bool {
        let Some(previous) = self.undo_stack.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.redo_stack.push(current);
        self.restore_snapshot(previous);
        true
    }

    fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.undo_stack.push(current);
        self.restore_snapshot(next);
        true
    }

    fn request_cancel(&mut self) -> MergeCancelRequest {
        if self.has_unsaved_edits() {
            self.show_cancel_confirm = true;
            MergeCancelRequest::ShowConfirm
        } else {
            MergeCancelRequest::ExitNow
        }
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let (undo_requested, redo_requested, find_requested, close_find_requested) =
            ctx.input(|i| {
                let ctrl = i.modifiers.ctrl || i.modifiers.command;
                (
                    ctrl && i.key_pressed(egui::Key::Z) && !i.modifiers.shift,
                    ctrl && i.key_pressed(egui::Key::Y)
                        || (ctrl && i.modifiers.shift && i.key_pressed(egui::Key::Z)),
                    ctrl && i.key_pressed(egui::Key::F),
                    i.key_pressed(egui::Key::Escape),
                )
            });
        if find_requested {
            self.search.open = true;
            self.search.pane = self.hovered_search_pane.unwrap_or(self.search.pane);
            self.search.request_focus = true;
            self.collapse_unchanged = false;
        } else if close_find_requested && self.search.open {
            self.search.open = false;
        }
        if undo_requested && self.can_undo() {
            self.undo();
        } else if redo_requested && self.can_redo() {
            self.redo();
        }
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && self.has_unsaved_edits() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.show_cancel_confirm = true;
        }
    }

    fn accept_conflict(&mut self, side: MergeSide) {
        if self.manual_result_override {
            return;
        }
        let before = self.snapshot();
        let index = match side {
            MergeSide::Local => self.local_conflict_cursor,
            MergeSide::Remote => self.remote_conflict_cursor,
        };
        self.document.apply_conflict(index, side);
        self.result_text = self.document.result_text();
        self.reset_manual_result_lines();
        let conflict_count = self.document.conflicts().len();
        if conflict_count > 0 {
            self.local_conflict_cursor = (self.local_conflict_cursor + 1).min(conflict_count - 1);
            self.remote_conflict_cursor = (self.remote_conflict_cursor + 1).min(conflict_count - 1);
        }
        self.finish_document_edit(before);
    }

    fn apply_line_action(
        &mut self,
        target: MergeLineActionTarget,
        side: MergeSide,
        action: MergeLineAction,
    ) {
        if self.manual_result_override {
            return;
        }
        let before = self.snapshot();
        match (target, action) {
            (MergeLineActionTarget::Conflict(index), MergeLineAction::Take) => {
                self.document.take_conflict_side(index, side)
            }
            (MergeLineActionTarget::Conflict(index), MergeLineAction::Drop) => {
                self.document.drop_conflict_side(index, side)
            }
            (MergeLineActionTarget::BaseOnlyGroup(line_index), MergeLineAction::Take) => {
                self.document.take_base_only_group(line_index, side)
            }
            (MergeLineActionTarget::BaseOnlyGroup(line_index), MergeLineAction::Drop) => {
                self.document.drop_base_only_group(line_index, side)
            }
        }
        self.result_text = self.document.result_text();
        self.reset_manual_result_lines();
        self.finish_document_edit(before);
    }

    fn reset_manual_result_lines(&mut self) {
        if self.manual_result_override {
            return;
        }
        self.manual_result_lines = merge_result_display_rows(&self.document)
            .into_iter()
            .map(|row| row.text.to_owned())
            .collect();
        self.rebuild_display_rows();
    }

    fn rebuild_display_rows(&mut self) {
        let local_display_rows = cached_merge_side_display_rows(&self.document, MergeSide::Local);
        let remote_display_rows = cached_merge_side_display_rows(&self.document, MergeSide::Remote);
        let result_display_rows = merge_result_display_rows(&self.document);
        let local_scroll_anchors = merge_cached_scroll_anchors(
            &self.document,
            MergeSide::Local,
            &result_display_rows,
            &local_display_rows,
        );
        let remote_scroll_anchors = merge_cached_scroll_anchors(
            &self.document,
            MergeSide::Remote,
            &result_display_rows,
            &remote_display_rows,
        );
        let result_display_rows = cached_merge_result_display_rows(&result_display_rows);
        let geometry_cache = merge_geometry_cache(
            &self.document,
            &result_display_rows,
            &local_display_rows,
            &remote_display_rows,
        );
        self.local_display_rows = local_display_rows;
        self.remote_display_rows = remote_display_rows;
        self.result_display_rows = result_display_rows;
        self.geometry_cache = geometry_cache;
        self.local_scroll_anchors = local_scroll_anchors;
        self.remote_scroll_anchors = remote_scroll_anchors;
        self.rebuild_ai_middle_edit_rows();
        // Rows may appear or disappear after accepting a deletion or undoing it. Give all
        // three ScrollAreas a fresh identity so egui cannot retain geometry from another shape.
        self.display_epoch = self.display_epoch.wrapping_add(1);
    }

    fn rebuild_ai_middle_edit_rows(&mut self) {
        self.ai_middle_edit_rows =
            merge_ai_middle_edit_row_cache(&self.ai_suggestions, &self.manual_result_lines);
    }

    fn uses_virtual_merge_rows(&self) -> bool {
        self.manual_result_lines.len() >= MERGE_VIRTUAL_ROW_THRESHOLD
            || self.local_display_rows.len() >= MERGE_VIRTUAL_ROW_THRESHOLD
            || self.remote_display_rows.len() >= MERGE_VIRTUAL_ROW_THRESHOLD
    }

    fn cached_scroll_anchors(&self, side: MergeSide) -> &[(f32, f32)] {
        match side {
            MergeSide::Local => &self.local_scroll_anchors,
            MergeSide::Remote => &self.remote_scroll_anchors,
        }
    }

    fn cached_side_scroll_y_for_result_scroll(&self, side: MergeSide, result_scroll_y: f32) -> f32 {
        let result_row = result_scroll_y / MERGE_CODE_ROW_HEIGHT;
        merge_mapped_scroll_row(self.cached_scroll_anchors(side), result_row, true)
            * MERGE_CODE_ROW_HEIGHT
    }

    fn cached_result_scroll_y_for_side_scroll(&self, side: MergeSide, side_scroll_y: f32) -> f32 {
        let side_row = side_scroll_y / MERGE_CODE_ROW_HEIGHT;
        merge_mapped_scroll_row(self.cached_scroll_anchors(side), side_row, false)
            * MERGE_CODE_ROW_HEIGHT
    }

    fn finish_manual_result_edit(&mut self, before: MergeEditSnapshot) {
        self.manual_result_override = true;
        self.result_text = self.manual_result_lines.join("\n");
        if !self.result_text.is_empty() {
            self.result_text.push('\n');
        }
        self.finish_document_edit(before);
    }

    fn schedule_result_highlight(&mut self) {
        self.result_highlight_revision = self.result_highlight_revision.wrapping_add(1);
        self.result_highlight_due = Some(Instant::now() + Duration::from_millis(140));
    }

    fn poll_result_highlight(&mut self, ctx: &egui::Context) {
        if let Some(receiver) = self.result_highlight_task.take() {
            match receiver.try_recv() {
                Ok((revision, highlight)) => {
                    if revision == self.result_highlight_revision {
                        self.syntax_highlights.result = highlight;
                    }
                    ctx.request_repaint();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.result_highlight_task = Some(receiver);
                }
                Err(mpsc::TryRecvError::Disconnected) => {}
            }
        }

        let Some(due) = self.result_highlight_due else {
            return;
        };
        if self.result_highlight_task.is_some() {
            ctx.request_repaint_after(Duration::from_millis(30));
            return;
        }
        if Instant::now() < due {
            ctx.request_repaint_after(due.saturating_duration_since(Instant::now()));
            return;
        }

        self.result_highlight_due = None;
        let revision = self.result_highlight_revision;
        let args = self.args.clone();
        let source = merge_highlight_source_from_lines(&self.manual_result_lines);
        let (sender, receiver) = mpsc::channel();
        self.result_highlight_task = Some(receiver);
        thread::spawn(move || {
            let highlight = highlight_merge_source(&args, &source);
            let _ = sender.send((revision, highlight));
        });
        ctx.request_repaint_after(Duration::from_millis(30));
    }

    fn toggle_theme(&mut self, ctx: &egui::Context) {
        self.theme = match self.theme {
            MergeTheme::Dark => MergeTheme::Light,
            MergeTheme::Light => MergeTheme::Dark,
        };
        apply_merge_theme(ctx, self.theme);
    }

    fn toggle_language(&mut self) {
        self.language = match self.language {
            MergeLanguage::English => MergeLanguage::Chinese,
            MergeLanguage::Chinese => MergeLanguage::English,
        };
    }

    fn write_output(&mut self) {
        if self.write_task.is_some() {
            return;
        }
        if self.unresolved_conflict_count() > 0 {
            self.status = Some(mt(self.language, "resolve_all_conflicts").to_owned());
            return;
        }
        let args = self.args.clone();
        let result_text = self.result_text.clone();
        let (sender, receiver) = mpsc::channel();
        self.write_task = Some(receiver);
        self.status = Some(mt(self.language, "applying").to_owned());
        thread::spawn(move || {
            let _ = sender.send(write_merge_output(&args, &result_text));
        });
    }

    fn poll_write_task(&mut self, ctx: &egui::Context) {
        let Some(receiver) = self.write_task.take() else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(())) => std::process::exit(0),
            Ok(Err(error)) => {
                self.status = Some(format!(
                    "{} {}: {error}",
                    mt(self.language, "write_failed"),
                    self.args.output.display()
                ));
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.write_task = Some(receiver);
                ctx.request_repaint_after(Duration::from_millis(60));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.status = Some(mt(self.language, "write_stopped").to_owned());
            }
        }
    }

    fn poll_load_task(&mut self, ctx: &egui::Context) {
        let Some(receiver) = self.load_task.take() else {
            return;
        };
        loop {
            match receiver.try_recv() {
                Ok(MergeLoadEvent::Progress(progress)) => {
                    self.load_progress = progress;
                    ctx.request_repaint();
                }
                Ok(MergeLoadEvent::Finished(Ok(prepared))) => {
                    *self = Self::from_prepared(self.args.clone(), prepared);
                    ctx.request_repaint();
                    return;
                }
                Ok(MergeLoadEvent::Finished(Err(error))) => {
                    self.status = Some(error.to_string());
                    ctx.request_repaint();
                    return;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.load_task = Some(receiver);
                    ctx.request_repaint_after(Duration::from_millis(40));
                    return;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.status = Some(mt(self.language, "analysis_stopped").to_owned());
                    ctx.request_repaint();
                    return;
                }
            }
        }
    }

    fn record_frame_performance(&mut self, elapsed: Duration) {
        self.frame_count = self.frame_count.saturating_add(1);
        self.frame_total += elapsed;
        self.frame_max = self.frame_max.max(elapsed);
        let window = self.frame_window_started.elapsed();
        let report_after = if self.frame_logged_once {
            Duration::from_secs(10)
        } else {
            Duration::from_secs(2)
        };
        if window < report_after {
            return;
        }

        let average_ms = self.frame_total.as_secs_f64() * 1_000.0 / self.frame_count.max(1) as f64;
        let max_ms = self.frame_max.as_secs_f64() * 1_000.0;
        if !self.frame_logged_once || average_ms >= 8.0 || max_ms >= 32.0 {
            crate::diagnostics::merge_tool_info(
                "frame.performance",
                &format!(
                    "output={} window_ms={} frames={} avg_ms={average_ms:.2} max_ms={max_ms:.2} document_lines={} result_rows={} local_rows={} remote_rows={} conflicts={} deletion_groups={}",
                    self.args.output.display(),
                    window.as_millis(),
                    self.frame_count,
                    self.document.lines.len(),
                    self.result_display_rows.len(),
                    self.local_display_rows.len(),
                    self.remote_display_rows.len(),
                    self.document.conflicts().len(),
                    self.geometry_cache.base_only_groups.len(),
                ),
            );
        }
        self.frame_window_started = Instant::now();
        self.frame_count = 0;
        self.frame_total = Duration::ZERO;
        self.frame_max = Duration::ZERO;
        self.frame_logged_once = true;
    }
}

fn merge_app_icon_data() -> egui::IconData {
    const SIZE: usize = 64;
    let mut rgba = vec![0_u8; SIZE * SIZE * 4];
    let green = [21, 196, 151, 255];
    let blue = [47, 111, 234, 255];

    // Same visual language as the main app: transparent canvas, crisp Git routes,
    // hollow nodes. A green change is inserted into the blue result line.
    paint_merge_icon_line(&mut rgba, 16, 39, 48, 39, blue);
    paint_merge_icon_line(&mut rgba, 24, 17, 24, 34, green);
    paint_merge_icon_line(&mut rgba, 18, 28, 24, 34, green);
    paint_merge_icon_line(&mut rgba, 30, 28, 24, 34, green);
    paint_merge_icon_ring(&mut rgba, 24, 17, green);
    paint_merge_icon_ring(&mut rgba, 48, 39, blue);

    egui::IconData {
        rgba,
        width: SIZE as u32,
        height: SIZE as u32,
    }
}

fn paint_merge_icon_ring(rgba: &mut [u8], cx: usize, cy: usize, color: [u8; 4]) {
    const RADIUS: usize = 8;
    const WIDTH: usize = 4;
    let outer_sq = (RADIUS * RADIUS) as isize;
    let inner_sq = ((RADIUS - WIDTH) * (RADIUS - WIDTH)) as isize;
    for y in cy - RADIUS..=cy + RADIUS {
        for x in cx - RADIUS..=cx + RADIUS {
            let dx = x as isize - cx as isize;
            let dy = y as isize - cy as isize;
            let distance_sq = dx * dx + dy * dy;
            if distance_sq <= outer_sq && distance_sq >= inner_sq {
                paint_merge_icon_pixel(rgba, x, y, color);
            }
        }
    }
}

fn paint_merge_icon_line(
    rgba: &mut [u8],
    start_x: usize,
    start_y: usize,
    end_x: usize,
    end_y: usize,
    color: [u8; 4],
) {
    let steps = start_x.abs_diff(end_x).max(start_y.abs_diff(end_y)).max(1);
    for step in 0..=steps {
        let progress = step as f32 / steps as f32;
        let x = (start_x as f32 + (end_x as f32 - start_x as f32) * progress).round() as usize;
        let y = (start_y as f32 + (end_y as f32 - start_y as f32) * progress).round() as usize;
        paint_merge_icon_disc(rgba, x, y, color);
    }
}

fn paint_merge_icon_disc(rgba: &mut [u8], cx: usize, cy: usize, color: [u8; 4]) {
    const SIZE: usize = 64;
    const RADIUS: usize = 4;
    let radius_sq = (RADIUS * RADIUS) as isize;
    for y in cy.saturating_sub(RADIUS)..=(cy + RADIUS).min(SIZE - 1) {
        for x in cx.saturating_sub(RADIUS)..=(cx + RADIUS).min(SIZE - 1) {
            let dx = x as isize - cx as isize;
            let dy = y as isize - cy as isize;
            if dx * dx + dy * dy <= radius_sq {
                paint_merge_icon_pixel(rgba, x, y, color);
            }
        }
    }
}

fn paint_merge_icon_pixel(rgba: &mut [u8], x: usize, y: usize, color: [u8; 4]) {
    const SIZE: usize = 64;
    let index = (y * SIZE + x) * 4;
    rgba[index..index + 4].copy_from_slice(&color);
}

pub fn write_merge_output(args: &MergeArgs, result_text: &str) -> anyhow::Result<()> {
    fs::write(&args.output, result_text)
        .with_context(|| format!("failed to write {}", args.output.display()))?;
    if args.stage {
        let repo_root = args
            .repo_root
            .as_deref()
            .context("missing --repo-root for --stage")?;
        stage_merge_output(repo_root, &args.output)?;
    }
    Ok(())
}

fn stage_merge_output(repo_root: &Path, output: &Path) -> anyhow::Result<()> {
    let path_arg = output.strip_prefix(repo_root).unwrap_or(output);
    let status = crate::git::git_command()
        .arg("-C")
        .arg(repo_root)
        .arg("add")
        .arg("--")
        .arg(path_arg)
        .status()
        .with_context(|| format!("failed to stage {}", output.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("git add failed for {}", output.display()))
    }
}

fn collect_merge_ai_context(
    args: &MergeArgs,
    _sources: &MergeSourceText,
    document: &MergeDocument,
) -> Result<MergeAiContext, String> {
    let Some(repo_root) = args.repo_root.as_deref() else {
        return Ok(MergeAiContext::default());
    };
    let relative_output = args
        .output
        .strip_prefix(repo_root)
        .unwrap_or(&args.output)
        .to_string_lossy()
        .replace('\\', "/");
    let head = git_context_output(
        repo_root,
        [
            "rev-parse".to_owned(),
            "--verify".to_owned(),
            "HEAD".to_owned(),
        ],
    )
    .unwrap_or_default();
    let merge_head = git_context_output(
        repo_root,
        [
            "rev-parse".to_owned(),
            "--verify".to_owned(),
            "MERGE_HEAD".to_owned(),
        ],
    )
    .unwrap_or_default();
    let head = head.trim();
    let merge_head = merge_head.trim();
    let merge_base = (!head.is_empty() && !merge_head.is_empty())
        .then(|| {
            git_context_output(
                repo_root,
                [
                    "merge-base".to_owned(),
                    head.to_owned(),
                    merge_head.to_owned(),
                ],
            )
            .unwrap_or_default()
        })
        .unwrap_or_default();
    let merge_base = merge_base.trim();

    let mut history = String::new();
    if !head.is_empty() && !merge_head.is_empty() {
        append_merge_ai_section(
            &mut history,
            "MERGE TIPS",
            &format!(
                "LEFT HEAD: {head}\nRIGHT MERGE_HEAD: {merge_head}\nMERGE BASE: {}",
                if merge_base.is_empty() {
                    "(unavailable)"
                } else {
                    merge_base
                }
            ),
        );
    }
    for (label, revision) in [
        ("LEFT FILE HISTORY", head),
        ("RIGHT FILE HISTORY", merge_head),
    ] {
        if revision.is_empty() {
            continue;
        }
        append_merge_ai_section(
            &mut history,
            label,
            &git_context_output(
                repo_root,
                [
                    "log".to_owned(),
                    "--format=%H%n%s%n%b%n---".to_owned(),
                    "-n".to_owned(),
                    "16".to_owned(),
                    revision.to_owned(),
                    "--".to_owned(),
                    relative_output.clone(),
                ],
            )
            .unwrap_or_default(),
        );
    }
    if !head.is_empty() && !merge_head.is_empty() {
        // Keep file-specific history ahead of the broader branch log. The final context is bounded,
        // so placing forty potentially verbose commits first could otherwise truncate the history
        // most directly related to the conflicted file.
        append_merge_ai_section(
            &mut history,
            "COMMITS UNIQUE TO LEFT AND RIGHT MERGE TIPS",
            &git_context_output(
                repo_root,
                [
                    "log".to_owned(),
                    "--left-right".to_owned(),
                    "--cherry-pick".to_owned(),
                    "--format=%m %H%n%s%n%b%n---".to_owned(),
                    "-n".to_owned(),
                    "40".to_owned(),
                    format!("{head}...{merge_head}"),
                ],
            )
            .unwrap_or_default(),
        );
    }
    if history.is_empty() {
        append_merge_ai_section(
            &mut history,
            "FILE HISTORY",
            &git_context_output(
                repo_root,
                [
                    "log".to_owned(),
                    "--all".to_owned(),
                    "--format=%H%n%s%n%b%n---".to_owned(),
                    "-n".to_owned(),
                    "16".to_owned(),
                    "--".to_owned(),
                    relative_output.clone(),
                ],
            )
            .unwrap_or_default(),
        );
    }

    let mut repository_state = String::new();
    for (label, command) in [
        (
            "MERGE STATUS",
            vec![
                "status".to_owned(),
                "--short".to_owned(),
                "--branch".to_owned(),
            ],
        ),
        (
            "CURRENT MERGE DIFF NAMES",
            vec![
                "diff".to_owned(),
                "--name-status".to_owned(),
                "HEAD".to_owned(),
            ],
        ),
        (
            "CURRENT MERGE DIFF",
            vec![
                "diff".to_owned(),
                "--no-ext-diff".to_owned(),
                "--unified=12".to_owned(),
                "HEAD".to_owned(),
                "--".to_owned(),
                ".".to_owned(),
            ],
        ),
    ] {
        append_merge_ai_section(
            &mut repository_state,
            label,
            &git_context_output(repo_root, command).unwrap_or_default(),
        );
    }
    if !merge_base.is_empty() {
        for (label, revision) in [
            ("LEFT BRANCH CHANGES", head),
            ("RIGHT BRANCH CHANGES", merge_head),
        ] {
            if revision.is_empty() {
                continue;
            }
            append_merge_ai_section(
                &mut repository_state,
                label,
                &git_context_output(
                    repo_root,
                    [
                        "diff".to_owned(),
                        "--name-status".to_owned(),
                        format!("{merge_base}..{revision}"),
                    ],
                )
                .unwrap_or_default(),
            );
        }
    }

    let commit_ids = git_context_output(
        repo_root,
        [
            "log".to_owned(),
            "--all".to_owned(),
            "--format=%H".to_owned(),
            "-n".to_owned(),
            "16".to_owned(),
            "--".to_owned(),
            relative_output.clone(),
        ],
    )
    .unwrap_or_default();
    let mut related_paths = git_context_output(
        repo_root,
        [
            "diff".to_owned(),
            "--name-only".to_owned(),
            "--diff-filter=U".to_owned(),
        ],
    )
    .unwrap_or_default()
    .lines()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    related_paths.push(relative_output);
    for command in [
        vec![
            "diff".to_owned(),
            "--name-only".to_owned(),
            "HEAD".to_owned(),
        ],
        vec![
            "diff".to_owned(),
            "--cached".to_owned(),
            "--name-only".to_owned(),
        ],
    ] {
        if let Ok(paths) = git_context_output(repo_root, command) {
            related_paths.extend(paths.lines().map(str::to_owned));
        }
    }
    if !merge_base.is_empty() {
        for revision in [head, merge_head] {
            if revision.is_empty() {
                continue;
            }
            if let Ok(paths) = git_context_output(
                repo_root,
                [
                    "diff".to_owned(),
                    "--name-only".to_owned(),
                    format!("{merge_base}..{revision}"),
                ],
            ) {
                related_paths.extend(paths.lines().map(str::to_owned));
            }
        }
    }
    for commit in commit_ids.lines().filter(|line| !line.trim().is_empty()) {
        if let Ok(paths) = git_context_output(
            repo_root,
            [
                "show".to_owned(),
                "--format=".to_owned(),
                "--name-only".to_owned(),
                "--no-renames".to_owned(),
                commit.to_owned(),
            ],
        ) {
            related_paths.extend(paths.lines().map(str::to_owned));
        }
    }

    let identifiers = merge_ai_candidate_identifiers(document);
    let symbol_references = if identifiers.is_empty() {
        String::new()
    } else {
        let mut command = vec![
            "grep".to_owned(),
            "-n".to_owned(),
            "-I".to_owned(),
            "-F".to_owned(),
        ];
        for identifier in &identifiers {
            command.push("-e".to_owned());
            command.push(identifier.clone());
        }
        command.push("--".to_owned());
        command.push(".".to_owned());
        git_context_output(repo_root, command).unwrap_or_default()
    };
    let mut prioritized_paths = merge_ai_reference_paths(&symbol_references);
    prioritized_paths.extend(related_paths);
    related_paths = prioritized_paths;

    let mut seen = HashSet::new();
    let mut related_files = String::new();
    for relative_path in related_paths {
        let normalized = relative_path.trim().replace('\\', "/");
        if normalized.is_empty() || !seen.insert(normalized.clone()) {
            continue;
        }
        let relative = Path::new(&normalized);
        if relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            continue;
        }
        let path = repo_root.join(&normalized);
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        if !metadata.is_file() || related_files.len() >= MERGE_AI_MAX_CONTEXT_BYTES {
            continue;
        }
        let remaining = MERGE_AI_MAX_CONTEXT_BYTES.saturating_sub(related_files.len());
        let max_file_bytes = MERGE_AI_MAX_RELATED_FILE_BYTES.min(remaining);
        let Ok(mut file) = fs::File::open(&path) else {
            continue;
        };
        let mut bytes = Vec::new();
        use std::io::Read;
        if file
            .by_ref()
            .take(max_file_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .is_err()
        {
            continue;
        }
        let truncated = bytes.len() > max_file_bytes;
        bytes.truncate(max_file_bytes);
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        related_files.push_str("\n--- related file: ");
        related_files.push_str(&normalized);
        related_files.push_str(" ---\n");
        related_files.push_str(&text);
        if !text.ends_with('\n') {
            related_files.push('\n');
        }
        if truncated {
            related_files.push_str("[file excerpt truncated]\n");
        }
    }
    Ok(MergeAiContext {
        history: truncate_merge_ai_text(&history, MERGE_AI_MAX_HISTORY_CHARS),
        repository_state: truncate_merge_ai_text(&repository_state, 64 * 1024),
        related_files,
        symbol_references: truncate_merge_ai_text(&symbol_references, 32 * 1024),
    })
}

fn append_merge_ai_section(target: &mut String, title: &str, content: &str) {
    if content.trim().is_empty() {
        return;
    }
    target.push_str("\n--- ");
    target.push_str(title);
    target.push_str(" ---\n");
    target.push_str(content.trim());
    target.push('\n');
}

fn merge_ai_reference_paths(grep_output: &str) -> Vec<String> {
    grep_output
        .lines()
        .filter_map(|line| line.split_once(':').map(|(path, _)| path.to_owned()))
        .collect()
}

fn merge_ai_candidate_identifiers(document: &MergeDocument) -> Vec<String> {
    let mut identifiers = HashSet::new();
    let conflict_lines = document.conflicts().iter().flat_map(|conflict| {
        conflict
            .base
            .iter()
            .chain(&conflict.local)
            .chain(&conflict.remote)
            .map(String::as_str)
    });
    let deletion_lines = document
        .lines
        .iter()
        .filter(|line| line.is_base_only_display())
        .flat_map(|line| {
            [
                line.base.as_deref(),
                line.local.as_deref(),
                line.remote.as_deref(),
            ]
            .into_iter()
            .flatten()
        });
    for line in conflict_lines.chain(deletion_lines) {
        for token in line.split(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == '$')
        }) {
            let token = token.trim_matches('$');
            if token.len() < 4
                || token.len() > 96
                || token.chars().all(|character| character.is_ascii_digit())
                || matches!(
                    token,
                    "const"
                        | "class"
                        | "function"
                        | "return"
                        | "import"
                        | "export"
                        | "default"
                        | "from"
                        | "true"
                        | "false"
                        | "null"
                        | "undefined"
                )
            {
                continue;
            }
            identifiers.insert(token.to_owned());
        }
    }
    let mut identifiers = identifiers.into_iter().collect::<Vec<_>>();
    identifiers.sort_by(|left, right| {
        let left_specific = left.chars().any(char::is_uppercase) || left.contains('_');
        let right_specific = right.chars().any(char::is_uppercase) || right.contains('_');
        right_specific
            .cmp(&left_specific)
            .then_with(|| right.len().cmp(&left.len()))
            .then_with(|| left.cmp(right))
    });
    identifiers.truncate(24);
    identifiers
}

fn git_context_output<I>(repo_root: &Path, args: I) -> Result<String, String>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter().collect::<Vec<_>>();
    let path_output = if args.iter().any(|arg| arg == "--name-status") {
        Some(GitPathOutput::NameStatus)
    } else if args.iter().any(|arg| arg == "--name-only") {
        Some(GitPathOutput::NameOnly)
    } else if args.first().is_some_and(|arg| arg == "status")
        && args.iter().any(|arg| arg == "--short")
    {
        Some(GitPathOutput::Status)
    } else {
        None
    };
    if path_output.is_some() && !args.iter().any(|arg| arg == "-z") {
        args.insert(1, "-z".to_owned());
    }
    let mut command = crate::git::git_command();
    let output = command
        .arg("-C")
        .arg(repo_root)
        .args(&args)
        .output()
        .map_err(|error| format!("Unable to read Git context: {error}"))?;
    if output.status.success() {
        Ok(match path_output {
            Some(GitPathOutput::NameOnly) => format_nul_name_only(&output.stdout),
            Some(GitPathOutput::NameStatus) => format_nul_name_status(&output.stdout),
            Some(GitPathOutput::Status) => format_nul_status(&output.stdout),
            None => String::from_utf8_lossy(&output.stdout).into_owned(),
        })
    } else {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if !error.is_empty() {
            crate::diagnostics::merge_ai_trace(
                "context.git_failed",
                &format!(
                    "command={} error={}",
                    truncate_merge_ai_text(&args.join(" "), 500),
                    truncate_merge_ai_text(&error, 1_000),
                ),
            );
        }
        Err(error)
    }
}

#[derive(Clone, Copy)]
enum GitPathOutput {
    NameOnly,
    NameStatus,
    Status,
}

fn format_nul_name_only(output: &[u8]) -> String {
    output
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(String::from_utf8_lossy)
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_nul_name_status(output: &[u8]) -> String {
    let fields = output
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>();
    let mut lines = Vec::new();
    let mut index = 0;
    while index < fields.len() {
        let status = String::from_utf8_lossy(fields[index]);
        index += 1;
        let Some(path) = fields.get(index) else { break };
        let path = String::from_utf8_lossy(path);
        index += 1;
        if (status.starts_with('R') || status.starts_with('C')) && index < fields.len() {
            let destination = String::from_utf8_lossy(fields[index]);
            index += 1;
            lines.push(format!("{status}\t{path} -> {destination}"));
        } else {
            lines.push(format!("{status}\t{path}"));
        }
    }
    lines.join("\n")
}

fn format_nul_status(output: &[u8]) -> String {
    let fields = output
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>();
    let mut lines = Vec::new();
    let mut index = 0;
    while index < fields.len() {
        let record = String::from_utf8_lossy(fields[index]);
        index += 1;
        if record.starts_with("## ") {
            lines.push(record.into_owned());
            continue;
        }
        let bytes = record.as_bytes();
        let renamed = bytes.len() >= 3
            && bytes[2] == b' '
            && (matches!(bytes[0], b'R' | b'C') || matches!(bytes[1], b'R' | b'C'));
        if renamed && index < fields.len() {
            let source = String::from_utf8_lossy(fields[index]);
            index += 1;
            lines.push(format!("{} {source} -> {}", &record[..2], &record[3..]));
        } else {
            lines.push(record.into_owned());
        }
    }
    lines.join("\n")
}

fn send_merge_ai_tool_request(
    config: &crate::app::MergeAiModelConfig,
    prompt: &str,
    request_stage: &str,
    target_count: usize,
) -> Result<String, String> {
    let endpoint = merge_ai_endpoint(config)?;
    crate::diagnostics::merge_ai_trace(
        "http.start",
        &format!(
            "stage={request_stage} format={:?} endpoint={} model_id={} prompt_chars={} targets={target_count}",
            config.api_format,
            merge_ai_url_for_log(&endpoint),
            config.model_id,
            prompt.chars().count(),
        ),
    );
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(90))
        .build();
    let response = match config.api_format {
        crate::app::MergeAiApiFormat::OpenAiCompatible => agent
            .post(&endpoint)
            .set(
                "Authorization",
                &format!("Bearer {}", config.api_key.trim()),
            )
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "model": config.model_id.trim(),
                "temperature": 0.1,
                "messages": [
                    { "role": "system", "content": merge_ai_system_prompt() },
                    { "role": "user", "content": prompt },
                ],
                "tools": [{
                    "type": "function",
                    "function": {
                        "name": MERGE_AI_TOOL_NAME,
                        "description": "Submit one validated recommendation for every merge conflict and deletion decision.",
                        "parameters": merge_ai_suggestions_input_schema(),
                    }
                }],
                "tool_choice": {
                    "type": "function",
                    "function": { "name": MERGE_AI_TOOL_NAME }
                },
            })),
        crate::app::MergeAiApiFormat::Claude => agent
            .post(&endpoint)
            .set("x-api-key", config.api_key.trim())
            .set("anthropic-version", "2023-06-01")
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "model": config.model_id.trim(),
                "max_tokens": 4096,
                "temperature": 0.1,
                "thinking": { "type": "disabled" },
                "system": merge_ai_system_prompt(),
                "messages": [{ "role": "user", "content": prompt }],
                "tools": [{
                    "name": MERGE_AI_TOOL_NAME,
                    "description": "Submit one validated recommendation for every merge conflict and deletion decision.",
                    "input_schema": merge_ai_suggestions_input_schema(),
                }],
                "tool_choice": { "type": "tool", "name": MERGE_AI_TOOL_NAME },
            })),
    }
    .map_err(|error| match error {
        ureq::Error::Status(status, response) => {
            let detail = response.into_string().unwrap_or_default();
            let detail = truncate_merge_ai_text(&detail, 500);
            if detail.is_empty() {
                format!("AI server returned HTTP {status}")
            } else {
                format!("AI server returned HTTP {status}: {detail}")
            }
        }
        ureq::Error::Transport(error) => format!("AI request failed: {error}"),
    })?;
    crate::diagnostics::merge_ai_trace(
        "http.success",
        &format!("stage={request_stage} status=2xx"),
    );
    let response: serde_json::Value = response
        .into_json()
        .map_err(|_| "AI server returned an invalid JSON response".to_owned())?;
    crate::diagnostics::merge_ai_trace(
        "response.structure",
        &format!(
            "stage={request_stage} {}",
            merge_ai_response_structure(config.api_format, &response)
        ),
    );
    let (content, response_mode) = merge_ai_response_payload(config.api_format, &response)?;
    crate::diagnostics::merge_ai_trace(
        "response.payload",
        &format!(
            "stage={request_stage} mode={} chars={} preview={}",
            response_mode,
            content.chars().count(),
            serde_json::to_string(&truncate_merge_ai_text(&content, 4_000))
                .unwrap_or_else(|_| "\"<encode error>\"".to_owned()),
        ),
    );
    Ok(content)
}

fn request_merge_ai_suggestions(
    config: &crate::app::MergeAiModelConfig,
    args: &MergeArgs,
    sources: &MergeSourceText,
    document: &MergeDocument,
    context: &MergeAiContext,
) -> Result<Vec<MergeAiSuggestion>, String> {
    let endpoint = merge_ai_endpoint(config)?;
    let prompt = merge_ai_prompt(args, sources, document, context);
    crate::diagnostics::merge_ai_trace(
        "http.start",
        &format!(
            "format={:?} endpoint={} model_id={} prompt_chars={} targets={}",
            config.api_format,
            merge_ai_url_for_log(&endpoint),
            config.model_id,
            prompt.chars().count(),
            document.conflicts().len() + base_only_display_groups(document).len(),
        ),
    );
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(90))
        .build();
    let response = match config.api_format {
        crate::app::MergeAiApiFormat::OpenAiCompatible => agent
            .post(&endpoint)
            .set(
                "Authorization",
                &format!("Bearer {}", config.api_key.trim()),
            )
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "model": config.model_id.trim(),
                "temperature": 0.1,
                "messages": [
                    { "role": "system", "content": merge_ai_system_prompt() },
                    { "role": "user", "content": prompt },
                ],
                "tools": [{
                    "type": "function",
                    "function": {
                        "name": MERGE_AI_TOOL_NAME,
                        "description": "Submit one validated recommendation for every merge conflict and deletion decision.",
                        "parameters": merge_ai_suggestions_input_schema(),
                    }
                }],
                "tool_choice": {
                    "type": "function",
                    "function": { "name": MERGE_AI_TOOL_NAME }
                },
            })),
        crate::app::MergeAiApiFormat::Claude => agent
            .post(&endpoint)
            .set("x-api-key", config.api_key.trim())
            .set("anthropic-version", "2023-06-01")
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({
                "model": config.model_id.trim(),
                "max_tokens": 4096,
                "temperature": 0.1,
                // Merge suggestions are a compact machine-readable result. Disabling extended
                // thinking prevents compatible providers whose thinking defaults to on from
                // spending the whole output budget before emitting the final text block.
                "thinking": { "type": "disabled" },
                "system": merge_ai_system_prompt(),
                "messages": [{ "role": "user", "content": prompt }],
                "tools": [{
                    "name": MERGE_AI_TOOL_NAME,
                    "description": "Submit one validated recommendation for every merge conflict and deletion decision.",
                    "input_schema": merge_ai_suggestions_input_schema(),
                }],
                "tool_choice": { "type": "tool", "name": MERGE_AI_TOOL_NAME },
            })),
    }
    .map_err(|error| match error {
        ureq::Error::Status(status, response) => {
            let detail = response.into_string().unwrap_or_default();
            let detail = truncate_merge_ai_text(&detail, 500);
            if detail.is_empty() {
                format!("AI server returned HTTP {status}")
            } else {
                format!("AI server returned HTTP {status}: {detail}")
            }
        }
        ureq::Error::Transport(error) => format!("AI request failed: {error}"),
    })?;
    crate::diagnostics::merge_ai_trace("http.success", "status=2xx");
    let response: serde_json::Value = response
        .into_json()
        .map_err(|_| "AI server returned an invalid JSON response".to_owned())?;
    crate::diagnostics::merge_ai_trace(
        "response.structure",
        &merge_ai_response_structure(config.api_format, &response),
    );
    let (content, response_mode) = merge_ai_response_payload(config.api_format, &response)?;
    crate::diagnostics::merge_ai_trace(
        "response.payload",
        &format!(
            "mode={} chars={} preview={}",
            response_mode,
            content.chars().count(),
            serde_json::to_string(&truncate_merge_ai_text(&content, 4_000))
                .unwrap_or_else(|_| "\"<encode error>\"".to_owned()),
        ),
    );
    let mut valid_targets = document
        .conflicts()
        .iter()
        .map(|conflict| MergeLineActionTarget::Conflict(conflict.index))
        .collect::<HashSet<_>>();
    valid_targets.extend(
        base_only_display_groups(document)
            .into_iter()
            .map(|group| MergeLineActionTarget::BaseOnlyGroup(group.line_index)),
    );
    let payload_has_exact_coverage =
        merge_ai_response_has_exact_target_coverage(&content, &valid_targets);
    let suggestions = parse_merge_ai_suggestions(&content, &valid_targets, args.language)?;
    let suggestions = guard_merge_ai_suggestions(document, suggestions);
    let accepted_targets = suggestions
        .iter()
        .map(|suggestion| suggestion.target)
        .collect::<HashSet<_>>();
    let exact_coverage = payload_has_exact_coverage && accepted_targets == valid_targets;
    let needs_completeness_repair = merge_ai_needs_completeness_repair(&suggestions);
    if !exact_coverage || needs_completeness_repair {
        crate::diagnostics::merge_ai_trace(
            "validation.repair_requested",
            &format!(
                "reason={}{}",
                (!exact_coverage).then_some("target_coverage").unwrap_or(""),
                if !exact_coverage && needs_completeness_repair {
                    "+manual_result_without_confirmed_middle_edits"
                } else if needs_completeness_repair {
                    "manual_result_without_confirmed_middle_edits"
                } else {
                    ""
                }
            ),
        );
        let repair_prompt = merge_ai_validation_repair_prompt(
            &prompt,
            &content,
            !exact_coverage,
            needs_completeness_repair,
        );
        let repaired_content = send_merge_ai_tool_request(
            config,
            &repair_prompt,
            "completeness_repair",
            valid_targets.len(),
        )?;
        if !merge_ai_response_has_exact_target_coverage(&repaired_content, &valid_targets) {
            return Err(format!(
                "AI validation repair did not return exactly {} current merge targets",
                valid_targets.len()
            ));
        }
        let repaired =
            parse_merge_ai_suggestions(&repaired_content, &valid_targets, args.language)?;
        let repaired = guard_merge_ai_suggestions(document, repaired);
        let repaired_targets = repaired
            .iter()
            .map(|suggestion| suggestion.target)
            .collect::<HashSet<_>>();
        if repaired_targets != valid_targets {
            return Err(format!(
                "AI completeness repair returned {} of {} merge targets",
                repaired_targets.len(),
                valid_targets.len()
            ));
        }
        crate::diagnostics::merge_ai_trace(
            "validation.repair_accepted",
            &format!(
                "suggestions={} middle_edits={}",
                repaired.len(),
                repaired
                    .iter()
                    .map(|suggestion| suggestion.middle_edits.len())
                    .sum::<usize>()
            ),
        );
        return Ok(repaired);
    }
    Ok(suggestions)
}

fn merge_ai_needs_completeness_repair(suggestions: &[MergeAiSuggestion]) -> bool {
    suggestions.iter().any(|suggestion| {
        suggestion.choice == MergeAiChoice::Manual
            && suggestion.manual_result.is_some()
            && suggestion.middle_edits.is_empty()
    })
}

fn merge_ai_validation_repair_prompt(
    prompt: &str,
    previous_payload: &str,
    repair_coverage: bool,
    repair_completeness: bool,
) -> String {
    let coverage_instruction = if repair_coverage {
        "The previous payload had missing, duplicate, or extra targets. Return exactly one suggestion for each target explicitly listed under CONFLICTS and DELETION DECISIONS, and no suggestion for any other target."
    } else {
        "Preserve the exact target coverage from the supplied request."
    };
    let completeness_instruction = if repair_completeness {
        "At least one manual result lacked confirmed dependent Middle edits, so audit every manual replacement and encode every mechanically required non-target edit."
    } else {
        "Keep manual results and Middle edits internally consistent."
    };
    format!(
        "{prompt}\n\nSTRUCTURED COVERAGE, COMPLETENESS, AND CONSISTENCY AUDIT:\nThe previous tool result is reproduced below. Re-derive it from the supplied code instead of trusting any target, number, or claim in that result. {coverage_instruction} {completeness_instruction} Audit every decision against the target-local neighborhoods, resolved Middle result, assertions, references, and related files above. Never use an unresolved target's provisional presence or omission as evidence. Recount every explicit array element, object property, argument, parameter, enum member, and ordered operation in manual_result, then compare those counts and orders with every numeric or derived middle_edit and assertion. For example, six listed elements require a count of 6, never 5. A reason must never describe a concrete required code change that is absent from the structured payload. If applying manual_result changes a collection, signature, enum, or order and a supplied derived value, assertion, caller, or reference must also change, encode every exact non-target change in middle_edits. Do not label a mechanically provable mismatch as pre-existing when the exact correction is available. Keep middle_edits empty only after verifying no additional Middle line must change. Keep reason_zh and reason_en to one or two short sentences and merge order to one short sentence. Return the complete corrected tool payload once.\n\nPREVIOUS TOOL RESULT:\n{previous_payload}",
    )
}

fn merge_ai_url_for_log(value: &str) -> &str {
    value.split_once('?').map_or(value, |(path, _)| path)
}

fn merge_ai_suggestions_input_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "suggestions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "target_type": {
                            "type": "string",
                            "enum": ["conflict", "deletion"]
                        },
                        "target_index": { "type": "integer", "minimum": 0 },
                        "choice": {
                            "type": "string",
                            "enum": ["left", "right", "manual"]
                        },
                        "manual_result_provided": {
                            "type": "boolean",
                            "description": "For a manual choice, true only when manual_result contains the exact complete replacement for this target. It may be an empty string to delete the target. Use false when evidence is insufficient or for left/right choices."
                        },
                        "manual_result": {
                            "type": "string",
                            "description": "Exact code replacing this conflict/deletion target when manual_result_provided is true. Do not include conflict markers or Markdown fences. Use an empty string otherwise."
                        },
                        "middle_edits": {
                            "type": "array",
                            "description": "Additional exact edits to already resolved Middle code, including non-diff lines. Each expected_text must be a unique substring contained within one logical Middle line. replacement_text may contain multiple lines and should include the anchor text when inserting around it.",
                            "items": {
                                "type": "object",
                                "additionalProperties": false,
                                "properties": {
                                    "expected_text": { "type": "string", "minLength": 1 },
                                    "replacement_text": { "type": "string" }
                                },
                                "required": ["expected_text", "replacement_text"]
                            }
                        },
                        "reason_zh": {
                            "type": "string",
                            "maxLength": 220,
                            "description": "Exactly one or two short Simplified Chinese sentences: state the conclusion first, then only the decisive Middle/history/reference evidence. Use 左边、右边、中间 for the panes. Do not repeat code, the merge order, or the full reasoning process."
                        },
                        "reason_en": {
                            "type": "string",
                            "maxLength": 320,
                            "description": "Exactly one or two short English sentences: state the conclusion first, then only the decisive Middle/history/reference evidence. Use Left, Right, and Middle for the panes. Do not repeat code, the merge order, or the full reasoning process."
                        },
                        "merge_order_zh": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 220,
                            "description": "In one short sentence, state the exact execution/precedence order when both sides are retained. For overlapping control flow state which branch wins; for parallel edits state that there is no runtime order and give placement. If evidence cannot decide, name the candidate orders and behavioral difference compactly. For left/right suggestions use 不适用."
                        },
                        "merge_order_en": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 320,
                            "description": "In one short sentence, state the exact execution/precedence order when both sides are retained. For overlapping control flow state which branch wins; for parallel edits state that there is no runtime order and give placement. If evidence cannot decide, name the candidate orders and behavioral difference compactly. For left/right suggestions use Not applicable."
                        }
                    },
                    "required": ["target_type", "target_index", "choice", "manual_result_provided", "manual_result", "middle_edits", "reason_zh", "reason_en", "merge_order_zh", "merge_order_en"]
                }
            }
        },
        "required": ["suggestions"]
    })
}

fn merge_ai_endpoint(config: &crate::app::MergeAiModelConfig) -> Result<String, String> {
    let base_url = config.base_url.trim().trim_end_matches('/');
    if !(base_url.starts_with("https://") || base_url.starts_with("http://")) {
        return Err("AI base URL must start with http:// or https://".to_owned());
    }
    let suffix = match config.api_format {
        crate::app::MergeAiApiFormat::OpenAiCompatible => "chat/completions",
        crate::app::MergeAiApiFormat::Claude if base_url.ends_with("/v1") => "messages",
        crate::app::MergeAiApiFormat::Claude => "v1/messages",
    };
    Ok(format!("{base_url}/{suffix}"))
}

fn merge_ai_response_content(
    format: crate::app::MergeAiApiFormat,
    response: &serde_json::Value,
) -> Result<String, String> {
    let content = match format {
        crate::app::MergeAiApiFormat::OpenAiCompatible => response
            .pointer("/choices/0/message/content")
            .and_then(merge_ai_content_value_text),
        crate::app::MergeAiApiFormat::Claude => response
            .get("content")
            .and_then(merge_ai_content_value_text)
            // A few API gateways expose an Anthropic request endpoint but keep an OpenAI-style
            // response envelope. Accept that common compatibility shape without weakening target
            // validation of the suggestion JSON itself.
            .or_else(|| {
                response
                    .pointer("/choices/0/message/content")
                    .and_then(merge_ai_content_value_text)
            }),
    };
    content
        .filter(|content| !content.trim().is_empty())
        .ok_or_else(|| "AI response did not include a text completion".to_owned())
}

fn merge_ai_response_payload(
    format: crate::app::MergeAiApiFormat,
    response: &serde_json::Value,
) -> Result<(String, &'static str), String> {
    let preferred = match format {
        crate::app::MergeAiApiFormat::OpenAiCompatible => {
            merge_ai_openai_tool_arguments(response).map(|payload| (payload, "openai_function"))
        }
        crate::app::MergeAiApiFormat::Claude => {
            merge_ai_claude_tool_input(response).map(|payload| (payload, "anthropic_tool_use"))
        }
    };
    if let Some((payload, mode)) = preferred {
        return payload
            .map(|payload| (payload, mode))
            .map_err(|error| format!("AI tool arguments were invalid: {error}"));
    }

    // Compatibility gateways occasionally keep the other provider's response envelope even when
    // accepting the configured request style.
    let compatible = match format {
        crate::app::MergeAiApiFormat::OpenAiCompatible => {
            merge_ai_claude_tool_input(response).map(|payload| (payload, "anthropic_tool_use"))
        }
        crate::app::MergeAiApiFormat::Claude => {
            merge_ai_openai_tool_arguments(response).map(|payload| (payload, "openai_function"))
        }
    };
    if let Some((payload, mode)) = compatible {
        return payload
            .map(|payload| (payload, mode))
            .map_err(|error| format!("AI tool arguments were invalid: {error}"));
    }

    merge_ai_response_content(format, response).map(|content| (content, "text_fallback"))
}

fn merge_ai_openai_tool_arguments(response: &serde_json::Value) -> Option<Result<String, String>> {
    let function = response
        .pointer("/choices/0/message/tool_calls")
        .and_then(serde_json::Value::as_array)
        .and_then(|calls| {
            calls.iter().find_map(|call| {
                let function = call.get("function")?;
                (function.get("name").and_then(serde_json::Value::as_str)
                    == Some(MERGE_AI_TOOL_NAME))
                .then_some(function)
            })
        })
        .or_else(|| {
            let function = response.pointer("/choices/0/message/function_call")?;
            (function.get("name").and_then(serde_json::Value::as_str) == Some(MERGE_AI_TOOL_NAME))
                .then_some(function)
        })?;
    Some(merge_ai_tool_arguments_value(function.get("arguments")))
}

fn merge_ai_claude_tool_input(response: &serde_json::Value) -> Option<Result<String, String>> {
    let input = response
        .get("content")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .find_map(|block| {
            (block.get("type").and_then(serde_json::Value::as_str) == Some("tool_use")
                && block.get("name").and_then(serde_json::Value::as_str)
                    == Some(MERGE_AI_TOOL_NAME))
            .then(|| block.get("input"))
            .flatten()
        })?;
    Some(
        serde_json::to_string(input)
            .map_err(|error| format!("unable to encode Anthropic tool input: {error}")),
    )
}

fn merge_ai_tool_arguments_value(value: Option<&serde_json::Value>) -> Result<String, String> {
    let value = value.ok_or_else(|| "missing function arguments".to_owned())?;
    if let Some(arguments) = value.as_str() {
        serde_json::from_str::<serde_json::Value>(arguments)
            .map_err(|error| format!("function arguments are not valid JSON: {error}"))?;
        return Ok(arguments.to_owned());
    }
    serde_json::to_string(value).map_err(|error| format!("unable to encode arguments: {error}"))
}

fn merge_ai_content_value_text(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_owned());
    }
    let blocks = value.as_array()?;
    Some(
        blocks
            .iter()
            .filter_map(|block| {
                block
                    .get("text")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| block.get("content").and_then(serde_json::Value::as_str))
            })
            .collect::<String>(),
    )
}

fn merge_ai_response_structure(
    format: crate::app::MergeAiApiFormat,
    response: &serde_json::Value,
) -> String {
    let top_level_keys = response
        .as_object()
        .map(|object| object.keys().cloned().collect::<Vec<_>>().join(","))
        .unwrap_or_else(|| "<non-object>".to_owned());
    match format {
        crate::app::MergeAiApiFormat::Claude => {
            let blocks = response
                .get("content")
                .and_then(serde_json::Value::as_array);
            let block_types = blocks
                .map(|blocks| {
                    blocks
                        .iter()
                        .map(|block| {
                            block
                                .get("type")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("<unknown>")
                        })
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_else(|| "<not-array>".to_owned());
            let tool_names = blocks
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter(|block| {
                            block.get("type").and_then(serde_json::Value::as_str)
                                == Some("tool_use")
                        })
                        .filter_map(|block| block.get("name").and_then(serde_json::Value::as_str))
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .filter(|names| !names.is_empty())
                .unwrap_or_else(|| "<none>".to_owned());
            format!(
                "format=Claude keys={top_level_keys} stop_reason={} content_types={} tool_names={} input_tokens={} output_tokens={}",
                response
                    .get("stop_reason")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("<missing>"),
                block_types,
                tool_names,
                response
                    .pointer("/usage/input_tokens")
                    .and_then(serde_json::Value::as_u64)
                    .map_or_else(|| "<missing>".to_owned(), |value| value.to_string()),
                response
                    .pointer("/usage/output_tokens")
                    .and_then(serde_json::Value::as_u64)
                    .map_or_else(|| "<missing>".to_owned(), |value| value.to_string()),
            )
        }
        crate::app::MergeAiApiFormat::OpenAiCompatible => {
            let tool_names = response
                .pointer("/choices/0/message/tool_calls")
                .and_then(serde_json::Value::as_array)
                .map(|calls| {
                    calls
                        .iter()
                        .filter_map(|call| {
                            call.pointer("/function/name")
                                .and_then(serde_json::Value::as_str)
                        })
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .filter(|names| !names.is_empty())
                .unwrap_or_else(|| "<none>".to_owned());
            format!(
                "format=OpenAiCompatible keys={top_level_keys} finish_reason={} content_kind={} tool_names={} prompt_tokens={} completion_tokens={}",
                response
                    .pointer("/choices/0/finish_reason")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("<missing>"),
                response
                    .pointer("/choices/0/message/content")
                    .map(|content| if content.is_string() {
                        "string"
                    } else if content.is_array() {
                        "array"
                    } else {
                        "other"
                    })
                    .unwrap_or("<missing>"),
                tool_names,
                response
                    .pointer("/usage/prompt_tokens")
                    .and_then(serde_json::Value::as_u64)
                    .map_or_else(|| "<missing>".to_owned(), |value| value.to_string()),
                response
                    .pointer("/usage/completion_tokens")
                    .and_then(serde_json::Value::as_u64)
                    .map_or_else(|| "<missing>".to_owned(), |value| value.to_string()),
            )
        }
    }
}

fn merge_ai_response_has_exact_target_coverage(
    response: &str,
    valid_targets: &HashSet<MergeLineActionTarget>,
) -> bool {
    let response = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```");
    let response = response.trim_end_matches("```").trim();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(response) else {
        return false;
    };
    let Some(items) = value
        .get("suggestions")
        .and_then(serde_json::Value::as_array)
        .or_else(|| value.as_array())
    else {
        return false;
    };
    if items.len() != valid_targets.len() {
        return false;
    }

    let mut targets = HashSet::new();
    for item in items {
        let Some(index) = item
            .get("target_index")
            .or_else(|| item.get("conflict_index"))
            .and_then(serde_json::Value::as_u64)
        else {
            return false;
        };
        let target_type = item
            .get("target_type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("conflict")
            .trim()
            .to_ascii_lowercase();
        let target = match target_type.as_str() {
            "conflict" => MergeLineActionTarget::Conflict(index as usize),
            "deletion" | "base_only" | "base-only" => {
                MergeLineActionTarget::BaseOnlyGroup(index as usize)
            }
            _ => return false,
        };
        if !valid_targets.contains(&target) || !targets.insert(target) {
            return false;
        }
    }
    targets == *valid_targets
}

fn parse_merge_ai_suggestions(
    response: &str,
    valid_targets: &HashSet<MergeLineActionTarget>,
    _language: MergeLanguage,
) -> Result<Vec<MergeAiSuggestion>, String> {
    let response = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```");
    let response = response.trim_end_matches("```").trim();
    let value: serde_json::Value = serde_json::from_str(response)
        .map_err(|_| "AI did not return the requested suggestion JSON".to_owned())?;
    let items = value
        .get("suggestions")
        .and_then(serde_json::Value::as_array)
        .or_else(|| value.as_array())
        .ok_or_else(|| "AI response did not include suggestions".to_owned())?;
    let mut used = HashSet::new();
    let mut suggestions = Vec::new();
    let mut missing_index = 0usize;
    let mut unsupported_target_type = 0usize;
    let mut invalid_target = 0usize;
    let mut duplicate_target = 0usize;
    let mut unsupported_choice = 0usize;
    let mut missing_manual_order = 0usize;
    let mut invalid_manual_result = 0usize;
    let mut invalid_middle_edits = 0usize;
    for item in items {
        let Some(index) = item
            .get("target_index")
            .or_else(|| item.get("conflict_index"))
            .and_then(serde_json::Value::as_u64)
        else {
            missing_index += 1;
            trace_rejected_merge_ai_item("missing_index", item);
            continue;
        };
        let index = index as usize;
        let target_type = item
            .get("target_type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("conflict")
            .trim()
            .to_ascii_lowercase();
        let target = match target_type.as_str() {
            "conflict" => MergeLineActionTarget::Conflict(index),
            "deletion" | "base_only" | "base-only" => MergeLineActionTarget::BaseOnlyGroup(index),
            _ => {
                unsupported_target_type += 1;
                trace_rejected_merge_ai_item("unsupported_target_type", item);
                continue;
            }
        };
        if !valid_targets.contains(&target) {
            invalid_target += 1;
            trace_rejected_merge_ai_item("target_not_in_document", item);
            continue;
        }
        if !used.insert(target) {
            duplicate_target += 1;
            trace_rejected_merge_ai_item("duplicate_target", item);
            continue;
        }
        let choice_value = item
            .get("choice")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let choice = match choice_value.as_str() {
            "local" | "left" => MergeAiChoice::Local,
            "remote" | "right" | "theirs" => MergeAiChoice::Remote,
            "manual" | "none" | "uncertain" => MergeAiChoice::Manual,
            _ => {
                unsupported_choice += 1;
                trace_rejected_merge_ai_item("unsupported_choice", item);
                continue;
            }
        };
        let manual_result_provided = item
            .get("manual_result_provided")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let manual_result_value = item
            .get("manual_result")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let manual_result = if manual_result_provided {
            if choice != MergeAiChoice::Manual
                || manual_result_value.chars().count() > MERGE_AI_MAX_MANUAL_RESULT_CHARS
            {
                invalid_manual_result += 1;
                trace_rejected_merge_ai_item("invalid_manual_result", item);
                continue;
            }
            Some(normalize_merge_ai_code(manual_result_value))
        } else {
            None
        };
        let mut middle_edits = Vec::new();
        let middle_edit_values = item
            .get("middle_edits")
            .and_then(serde_json::Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut item_has_invalid_middle_edit = middle_edit_values.len() > MERGE_AI_MAX_MIDDLE_EDITS;
        for edit in middle_edit_values.iter().take(MERGE_AI_MAX_MIDDLE_EDITS) {
            let expected_text = edit
                .get("expected_text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let replacement_text = edit
                .get("replacement_text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            if expected_text.is_empty()
                || expected_text.contains('\r')
                || expected_text.contains('\n')
                || expected_text.chars().count() > MERGE_AI_MAX_MIDDLE_EDIT_EXPECTED_CHARS
                || replacement_text.chars().count() > MERGE_AI_MAX_MIDDLE_EDIT_REPLACEMENT_CHARS
            {
                item_has_invalid_middle_edit = true;
                break;
            }
            middle_edits.push(MergeAiMiddleEdit {
                expected_text: expected_text.to_owned(),
                replacement_text: normalize_merge_ai_code(replacement_text),
            });
        }
        if item_has_invalid_middle_edit {
            invalid_middle_edits += 1;
            trace_rejected_merge_ai_item("invalid_middle_edit", item);
            continue;
        }
        let legacy_reason = item
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .filter(|reason| !reason.trim().is_empty());
        let mut reason_zh = item
            .get("reason_zh")
            .and_then(serde_json::Value::as_str)
            .or(legacy_reason)
            .map(|reason| compact_merge_ai_text(reason, 220))
            .filter(|reason| !reason.trim().is_empty())
            .unwrap_or_else(|| mt(MergeLanguage::Chinese, "ai_no_reason").to_owned());
        let mut reason_en = item
            .get("reason_en")
            .and_then(serde_json::Value::as_str)
            .or(legacy_reason)
            .map(|reason| compact_merge_ai_text(reason, 320))
            .filter(|reason| !reason.trim().is_empty())
            .unwrap_or_else(|| mt(MergeLanguage::English, "ai_no_reason").to_owned());
        if choice == MergeAiChoice::Manual {
            let merge_order_zh = item
                .get("merge_order_zh")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|order| !order.is_empty());
            let merge_order_en = item
                .get("merge_order_en")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|order| !order.is_empty());
            let (Some(merge_order_zh), Some(merge_order_en)) = (merge_order_zh, merge_order_en)
            else {
                missing_manual_order += 1;
                trace_rejected_merge_ai_item("missing_manual_merge_order", item);
                continue;
            };
            reason_zh.push_str("\n\n合并顺序：");
            reason_zh.push_str(&compact_merge_ai_text(merge_order_zh, 220));
            reason_en.push_str("\n\nMerge order: ");
            reason_en.push_str(&compact_merge_ai_text(merge_order_en, 320));
        }
        crate::diagnostics::merge_ai_trace(
            "parse.accepted",
            &format!("target={target:?} choice={choice:?}"),
        );
        suggestions.push(MergeAiSuggestion {
            target,
            choice,
            reason_zh,
            reason_en,
            manual_result,
            middle_edits,
        });
    }
    crate::diagnostics::merge_ai_trace(
        "parse.summary",
        &format!(
            "items={} accepted={} missing_index={} unsupported_target_type={} invalid_target={} duplicate_target={} unsupported_choice={} missing_manual_order={} invalid_manual_result={} invalid_middle_edits={}",
            items.len(),
            suggestions.len(),
            missing_index,
            unsupported_target_type,
            invalid_target,
            duplicate_target,
            unsupported_choice,
            missing_manual_order,
            invalid_manual_result,
            invalid_middle_edits,
        ),
    );
    Ok(suggestions)
}

fn trace_rejected_merge_ai_item(reason: &str, item: &serde_json::Value) {
    let preview = serde_json::to_string(item).unwrap_or_else(|_| "<encode error>".to_owned());
    crate::diagnostics::merge_ai_trace(
        "parse.rejected",
        &format!(
            "reason={reason} item={}",
            truncate_merge_ai_text(&preview, 1_000)
        ),
    );
}

fn guard_merge_ai_suggestions(
    document: &MergeDocument,
    suggestions: Vec<MergeAiSuggestion>,
) -> Vec<MergeAiSuggestion> {
    suggestions
        .into_iter()
        .map(|mut suggestion| {
            if let MergeLineActionTarget::BaseOnlyGroup(line_index) = suggestion.target {
                return guard_merge_ai_deletion_suggestion(document, suggestion, line_index);
            }
            let MergeLineActionTarget::Conflict(conflict_index) = suggestion.target else {
                return suggestion;
            };
            let chosen_side = match suggestion.choice {
                MergeAiChoice::Local => MergeSide::Local,
                MergeAiChoice::Remote => MergeSide::Remote,
                MergeAiChoice::Manual => {
                    return guard_merge_ai_manual_suggestion(document, suggestion);
                }
            };
            let Some(conflict) = document.conflicts().get(conflict_index) else {
                return suggestion;
            };
            let (chosen_lines, other_lines, other_choice) = match chosen_side {
                MergeSide::Local => (&conflict.local, &conflict.remote, MergeAiChoice::Remote),
                MergeSide::Remote => (&conflict.remote, &conflict.local, MergeAiChoice::Local),
            };
            let bindings = merge_import_bindings(chosen_lines);
            if bindings.is_empty()
                || bindings.iter().any(|binding| {
                    other_lines
                        .iter()
                        .any(|line| contains_identifier(line, binding))
                })
            {
                return suggestion;
            }

            let mut chosen_document = document.clone();
            chosen_document.apply_conflict(conflict_index, chosen_side);
            let chosen_result = chosen_document.result_text();
            let unused = bindings
                .iter()
                .all(|binding| count_identifier_occurrences(&chosen_result, binding) <= 1);
            if !unused {
                return suggestion;
            }

            let names = bindings.join(", ");
            let import_only_choice = chosen_lines.iter().all(|line| {
                let line = line.trim();
                line.is_empty() || (line.starts_with("import ") && line.contains(" from "))
            });
            crate::diagnostics::merge_ai_trace(
                "validation.corrected",
                &format!(
                    "target={:?} reason=unused_import chosen={chosen_side:?} bindings={names} import_only={import_only_choice}",
                    suggestion.target,
                ),
            );
            if import_only_choice {
                suggestion.choice = other_choice;
                suggestion.manual_result = None;
                suggestion.middle_edits.clear();
                suggestion.reason_zh = format!(
                    "中间完整结果中仅剩导入声明本身，没有发现 {names} 的实际使用；因此不保留该未使用导入，采用另一边的删除结果。"
                );
                suggestion.reason_en = format!(
                    "In the complete Middle result, {names} appears only in the import declaration and has no remaining usage, so the unused import is removed by choosing the other side."
                );
            } else {
                suggestion.choice = MergeAiChoice::Manual;
                suggestion.manual_result = None;
                suggestion.middle_edits.clear();
                suggestion.reason_zh = format!(
                    "中间完整结果中没有发现 {names} 的实际使用，但包含该导入的一边还有其他改动，不能安全地整体切换到另一边；建议手动合并其他改动并删除未使用导入。"
                );
                suggestion.reason_en = format!(
                    "The complete Middle result has no usage of {names}, but that side also contains other changes, so switching the whole conflict is unsafe; manually keep the other changes and remove the unused import."
                );
            }
            suggestion
        })
        .collect()
}

fn guard_merge_ai_manual_suggestion(
    document: &MergeDocument,
    mut suggestion: MergeAiSuggestion,
) -> MergeAiSuggestion {
    let Some(manual_result) = suggestion.manual_result.as_deref() else {
        return suggestion;
    };
    let middle_lines = merge_result_display_rows(document)
        .into_iter()
        .map(|row| row.text)
        .collect::<Vec<_>>();
    for manual_line in manual_result.lines() {
        let Some((collection_name, item_count)) = explicit_array_assignment(manual_line) else {
            continue;
        };
        let Some(count_name) = middle_lines
            .iter()
            .find_map(|line| equality_count_for_array(line, &collection_name))
        else {
            continue;
        };
        let declarations = middle_lines
            .iter()
            .filter(|line| {
                contains_identifier(line, &count_name)
                    && line.contains('=')
                    && assignment_integer(line).is_some()
            })
            .copied()
            .collect::<Vec<_>>();
        let [declaration] = declarations.as_slice() else {
            continue;
        };
        let replacement = replace_assignment_integer(declaration, item_count);
        let Some(replacement) = replacement else {
            continue;
        };
        let mut corrected = false;
        let mut verified = false;
        if let Some(edit) = suggestion.middle_edits.iter_mut().find(|edit| {
            contains_identifier(&edit.expected_text, &count_name)
                || contains_identifier(&edit.replacement_text, &count_name)
        }) {
            verified = true;
            if edit.replacement_text != replacement {
                edit.expected_text = (*declaration).to_owned();
                edit.replacement_text = replacement;
                corrected = true;
            }
        } else if assignment_integer(declaration) != Some(item_count) {
            suggestion.middle_edits.push(MergeAiMiddleEdit {
                expected_text: (*declaration).to_owned(),
                replacement_text: replacement,
            });
            corrected = true;
            verified = true;
        }
        if verified {
            let order_zh = suggestion
                .reason_zh
                .split_once("\n\n")
                .map(|(_, order)| order.to_owned());
            let order_en = suggestion
                .reason_en
                .split_once("\n\n")
                .map(|(_, order)| order.to_owned());
            suggestion.reason_zh = format!(
                "建议同时保留左边和右边中仍被引用的改动。中间代码明确约束 {collection_name}.length 与 {count_name} 一致，已按合并后的 {item_count} 项生成联动修改。"
            );
            suggestion.reason_en = format!(
                "Keep the still-referenced changes from both Left and Right. Middle explicitly requires {collection_name}.length to match {count_name}, so the linked edit now uses the merged count of {item_count}."
            );
            if let Some(order) = order_zh {
                suggestion.reason_zh.push_str("\n\n");
                suggestion.reason_zh.push_str(&order);
            }
            if let Some(order) = order_en {
                suggestion.reason_en.push_str("\n\n");
                suggestion.reason_en.push_str(&order);
            }
            crate::diagnostics::merge_ai_trace(
                if corrected {
                    "validation.corrected"
                } else {
                    "validation.verified"
                },
                &format!(
                    "target={:?} reason=derived_array_count collection={} count_binding={} corrected_count={item_count}",
                    suggestion.target, collection_name, count_name
                ),
            );
        }
    }
    suggestion
}

fn explicit_array_assignment(line: &str) -> Option<(String, usize)> {
    let (left, right) = line.split_once('=')?;
    let name = left
        .trim_end()
        .chars()
        .rev()
        .take_while(|character| is_identifier_character(*character))
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    if name.is_empty() {
        return None;
    }
    let start = right.find('[')?;
    count_top_level_array_items(&right[start..]).map(|count| (name, count))
}

fn count_top_level_array_items(value: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut commas = 0usize;
    let mut has_item = false;
    for character in value.chars() {
        if let Some(current_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == current_quote {
                quote = None;
            }
            if depth == 1 && !character.is_whitespace() {
                has_item = true;
            }
            continue;
        }
        match character {
            '\'' | '"' | '`' => {
                quote = Some(character);
                if depth == 1 {
                    has_item = true;
                }
            }
            '[' | '(' | '{' => {
                depth += 1;
                if depth > 1 {
                    has_item = true;
                }
            }
            ']' => {
                if depth == 1 {
                    return Some(if has_item { commas + 1 } else { 0 });
                }
                depth = depth.checked_sub(1)?;
            }
            ')' | '}' => depth = depth.checked_sub(1)?,
            ',' if depth == 1 => commas += 1,
            _ if depth == 1 && !character.is_whitespace() => has_item = true,
            _ => {}
        }
    }
    None
}

fn equality_count_for_array(line: &str, collection_name: &str) -> Option<String> {
    let marker = format!("{collection_name}.length");
    let tail = line.split_once(&marker)?.1.trim_start();
    let tail = ["!==", "===", "!=", "=="]
        .into_iter()
        .find_map(|operator| tail.strip_prefix(operator))?
        .trim_start();
    let count_name = tail
        .chars()
        .take_while(|character| is_identifier_character(*character))
        .collect::<String>();
    (!count_name.is_empty()).then_some(count_name)
}

fn assignment_integer(line: &str) -> Option<usize> {
    let (_, right) = line.split_once('=')?;
    right
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}

fn replace_assignment_integer(line: &str, value: usize) -> Option<String> {
    let equals = line.find('=')?;
    let digit_start = line[equals + 1..]
        .char_indices()
        .find(|(_, character)| character.is_ascii_digit())?
        .0
        + equals
        + 1;
    let digit_end = line[digit_start..]
        .char_indices()
        .take_while(|(_, character)| character.is_ascii_digit())
        .map(|(index, character)| index + character.len_utf8())
        .last()?
        + digit_start;
    Some(format!(
        "{}{value}{}",
        &line[..digit_start],
        &line[digit_end..]
    ))
}

fn guard_merge_ai_deletion_suggestion(
    document: &MergeDocument,
    mut suggestion: MergeAiSuggestion,
    line_index: usize,
) -> MergeAiSuggestion {
    let chosen_side = match suggestion.choice {
        MergeAiChoice::Local => MergeSide::Local,
        MergeAiChoice::Remote => MergeSide::Remote,
        MergeAiChoice::Manual => return suggestion,
    };
    let Some(group) = base_only_display_groups(document)
        .into_iter()
        .find(|group| group.line_index == line_index)
    else {
        return suggestion;
    };
    if chosen_side == group.missing_side {
        return suggestion;
    }

    let kept_lines = document.lines[group.line_index..group.line_index + group.line_count]
        .iter()
        .filter_map(|line| match chosen_side {
            MergeSide::Local => line.local.clone(),
            MergeSide::Remote => line.remote.clone(),
        })
        .collect::<Vec<_>>();
    let bindings = merge_import_bindings(&kept_lines);
    if bindings.is_empty()
        || bindings.iter().any(|binding| {
            document.conflicts().iter().any(|conflict| {
                conflict.local.iter().chain(&conflict.remote).any(|line| {
                    !line.trim_start().starts_with("import ") && contains_identifier(line, binding)
                })
            })
        })
    {
        return suggestion;
    }

    let mut kept_document = document.clone();
    kept_document.drop_base_only_group(line_index, group.missing_side);
    let kept_result = kept_document.result_text();
    if !bindings
        .iter()
        .all(|binding| count_identifier_usages_outside_imports(&kept_result, binding) == 0)
    {
        return suggestion;
    }

    let names = bindings.join(", ");
    suggestion.choice = match group.missing_side {
        MergeSide::Local => MergeAiChoice::Local,
        MergeSide::Remote => MergeAiChoice::Remote,
    };
    suggestion.manual_result = None;
    suggestion.middle_edits.clear();
    suggestion.reason_zh = format!(
        "中间完整结果和所有待解决冲突中都没有发现 {names} 的实际使用；保留该块只会留下未使用导入，因此采用删除它的一边。"
    );
    suggestion.reason_en = format!(
        "Neither the complete Middle result nor any unresolved conflict contains a remaining use of {names}; keeping this block would only retain an unused import, so the side that deletes it is selected."
    );
    crate::diagnostics::merge_ai_trace(
        "validation.corrected",
        &format!(
            "target={:?} reason=unused_base_only_import bindings={names}",
            suggestion.target,
        ),
    );
    suggestion
}

fn merge_import_bindings(lines: &[String]) -> Vec<String> {
    let mut bindings = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        let Some(mut clause) = trimmed.strip_prefix("import ") else {
            continue;
        };
        clause = clause.strip_prefix("type ").unwrap_or(clause);
        let Some((clause, _)) = clause.split_once(" from ") else {
            continue;
        };
        let clause = clause.trim();
        if let Some((default, rest)) = clause.split_once(',') {
            push_import_binding(&mut bindings, default);
            collect_named_import_bindings(&mut bindings, rest);
        } else if clause.starts_with('{') {
            collect_named_import_bindings(&mut bindings, clause);
        } else if let Some(alias) = clause.strip_prefix("* as ") {
            push_import_binding(&mut bindings, alias);
        } else {
            push_import_binding(&mut bindings, clause);
        }
    }
    bindings.sort();
    bindings.dedup();
    bindings
}

fn collect_named_import_bindings(bindings: &mut Vec<String>, clause: &str) {
    let clause = clause.trim().trim_start_matches('{').trim_end_matches('}');
    for item in clause.split(',') {
        let item = item.trim().strip_prefix("type ").unwrap_or(item.trim());
        let binding = item.rsplit_once(" as ").map_or(item, |(_, alias)| alias);
        push_import_binding(bindings, binding);
    }
}

fn push_import_binding(bindings: &mut Vec<String>, binding: &str) {
    let binding = binding.trim();
    if binding.len() >= 2
        && binding.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '$'
        })
    {
        bindings.push(binding.to_owned());
    }
}

fn contains_identifier(text: &str, identifier: &str) -> bool {
    count_identifier_occurrences(text, identifier) > 0
}

fn count_identifier_occurrences(text: &str, identifier: &str) -> usize {
    text.match_indices(identifier)
        .filter(|(index, _)| {
            let before = text[..*index].chars().next_back();
            let after = text[*index + identifier.len()..].chars().next();
            !before.is_some_and(is_identifier_character)
                && !after.is_some_and(is_identifier_character)
        })
        .count()
}

fn count_identifier_usages_outside_imports(text: &str, identifier: &str) -> usize {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("import "))
        .map(|line| count_identifier_occurrences(line, identifier))
        .sum()
}

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '$'
}

fn merge_ai_system_prompt() -> &'static str {
    "You are a conservative Git merge assistant. Never claim to edit files and never execute changes. Analyze each conflict and deletion decision using the complete Left/Base/Right texts, the resolved Middle result, target-local neighborhoods, branch-specific Git history, the current repository merge diff, symbol references, and related files. The panes are named Left, Middle, and Right; they do not imply local or remote repository ownership. Treat Middle as authoritative only for changes already resolved outside the current target. Every unresolved target is omitted from the resolved Middle result, so its presence or absence there is never evidence to keep or delete that target. Never argue that a deletion should be kept merely because a provisional editor row or the other pane still contains it. Compare the target-local neighborhoods and verify whether the changed side moved, consolidated, or replaced the target behavior elsewhere. When the same operation is relocated to a broader or shared execution point that already covers the old location, retaining both copies duplicates behavior; keep both only when supplied evidence proves that each copy still has a distinct responsibility. Never preserve an import, declaration, dependency, or call merely because it exists on one side: first verify that the merged Middle result or current related code still uses it. When either side adds, removes, renames, or reorders array elements, object properties, method or function parameters and arguments, enum members, or ordered operations, inspect the complete Middle result plus related definitions and callers for resulting effects before choosing. Recommend left only when evidence shows the Left pane should win, right only when evidence shows the Right pane should win, or manual when the correct Middle result requires editing or evidence is insufficient. For an exact manual resolution, set manual_result_provided to true and return the complete replacement for that target in manual_result. Use middle_edits only for additional exact changes to already-resolved Middle code, including non-diff lines; every expected_text must be a unique substring within one logical Middle line, and replacement_text may contain multiple lines. Leave manual_result_provided false and middle_edits empty when evidence is insufficient. These payloads are proposals applied only after user confirmation. For every manual recommendation that retains meaningful content from both sides, merge_order_zh and merge_order_en must state the exact execution or precedence order. For control-flow branches, analyze condition overlap and state which branch wins when both conditions match; never claim mutual exclusivity without concrete supplied proof. If the retained edits are structurally parallel, explicitly state that no runtime order exists and specify their placement. If evidence cannot determine the order, list the candidate orders and their behavioral difference instead of saying only to keep both. Keep each localized reason to one or two short sentences: conclusion first, then only decisive evidence. Keep merge order to one short sentence and do not duplicate it in the reason. Do not expose the full reasoning chain. Submit recommendations exactly once through the submit_merge_suggestions tool."
}

fn merge_ai_editable_middle_text(document: &MergeDocument) -> String {
    document.result_text()
}

fn merge_ai_target_neighborhoods(sources: &MergeSourceText, document: &MergeDocument) -> String {
    let base_lines = split_lines(&sources.base);
    let local_lines = split_lines(&sources.local);
    let remote_lines = split_lines(&sources.remote);
    let local_changes = diff_changes(&base_lines, &local_lines, MergeIgnoreMode::None);
    let remote_changes = diff_changes(&base_lines, &remote_lines, MergeIgnoreMode::None);
    let mut targets = document
        .conflicts()
        .iter()
        .map(|conflict| {
            let start = conflict.line_indices.first().copied().unwrap_or(0);
            let end = conflict
                .line_indices
                .last()
                .map_or(start, |index| index + 1);
            (
                start,
                end,
                format!("Conflict {}", conflict.index),
                conflict.base.clone(),
                conflict.local.len(),
                conflict.remote.len(),
            )
        })
        .collect::<Vec<_>>();
    for group in base_only_display_groups(document) {
        let end = group.line_index + group.line_count;
        let base = document.lines[group.line_index..end]
            .iter()
            .filter_map(|line| line.base.clone())
            .collect::<Vec<_>>();
        let local_len = document.lines[group.line_index..end]
            .iter()
            .filter(|line| line.local.is_some())
            .count();
        let remote_len = document.lines[group.line_index..end]
            .iter()
            .filter(|line| line.remote.is_some())
            .count();
        targets.push((
            group.line_index,
            end,
            format!("Deletion {}", group.line_index),
            base,
            local_len,
            remote_len,
        ));
    }
    targets.sort_by_key(|target| target.0);

    let mut output = String::new();
    let mut base_cursor = 0usize;
    for (document_start, document_end, label, base_target, local_len, remote_len) in targets {
        let base_start = if base_target.is_empty() {
            base_cursor
        } else {
            find_merge_ai_line_block(&base_lines, &base_target, base_cursor)
                .or_else(|| find_merge_ai_line_block(&base_lines, &base_target, 0))
                .unwrap_or(base_cursor.min(base_lines.len()))
        };
        base_cursor = base_start.saturating_add(base_target.len());
        let local_start =
            side_position_for_base_position(&local_changes, base_start, MergeBoundaryBias::Before);
        let remote_start =
            side_position_for_base_position(&remote_changes, base_start, MergeBoundaryBias::Before);

        output.push_str("\n### ");
        output.push_str(&label);
        output.push_str(" target-local neighborhoods\n");
        append_merge_ai_line_window(
            &mut output,
            "BASE",
            &base_lines,
            base_start,
            base_target.len(),
        );
        append_merge_ai_line_window(&mut output, "LEFT", &local_lines, local_start, local_len);
        append_merge_ai_line_window(
            &mut output,
            "RIGHT",
            &remote_lines,
            remote_start,
            remote_len,
        );
        append_merge_ai_relocation_evidence(&mut output, document, &base_target);
        append_merge_ai_middle_window(&mut output, document, document_start, document_end);
    }
    output
}

fn append_merge_ai_relocation_evidence(
    output: &mut String,
    document: &MergeDocument,
    target_lines: &[String],
) {
    let resolved_middle = document.result_text();
    let resolved_lines = resolved_middle
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<HashSet<_>>();
    let mut repeated = target_lines
        .iter()
        .map(|line| line.trim())
        .filter(|line| {
            line.chars().count() >= 4
                && line.chars().any(|character| character.is_alphanumeric())
                && resolved_lines.contains(*line)
        })
        .collect::<Vec<_>>();
    repeated.sort_unstable();
    repeated.dedup();
    if repeated.is_empty() {
        return;
    }

    output.push_str(
        "CROSS-TARGET RELATIONSHIP EVIDENCE: the resolved Middle already contains these exact target lines elsewhere:\n",
    );
    for line in repeated {
        output.push_str("  = ");
        output.push_str(line);
        output.push('\n');
    }
    output.push_str(
        "Treat this as possible relocation or consolidation, not proof that both copies are required. Compare responsibilities before keeping the old target.\n",
    );
}

fn find_merge_ai_line_block(lines: &[String], target: &[String], start: usize) -> Option<usize> {
    if target.is_empty() {
        return Some(start.min(lines.len()));
    }
    lines
        .windows(target.len())
        .enumerate()
        .skip(start)
        .find_map(|(index, window)| {
            window
                .iter()
                .map(String::as_str)
                .eq(target.iter().map(String::as_str))
                .then_some(index)
        })
}

fn append_merge_ai_line_window(
    output: &mut String,
    pane: &str,
    lines: &[String],
    target_start: usize,
    target_len: usize,
) {
    let target_start = target_start.min(lines.len());
    let target_end = target_start.saturating_add(target_len).min(lines.len());
    let start = target_start.saturating_sub(MERGE_AI_TARGET_CONTEXT_RADIUS);
    let end = target_end
        .saturating_add(MERGE_AI_TARGET_CONTEXT_RADIUS)
        .min(lines.len());
    output.push_str(pane);
    output.push_str(" neighborhood (>> marks target lines):\n");
    for index in start..end {
        if target_len == 0 && index == target_start {
            output.push_str(">> [TARGET ABSENT IN THIS PANE; DELETION/INSERTION POINT]\n");
        }
        let marker = if (target_start..target_end).contains(&index) {
            ">>"
        } else {
            "  "
        };
        output.push_str(&format!("{marker} {:>6}: {}\n", index + 1, lines[index]));
    }
    if target_len == 0 && target_start == end {
        output.push_str(">> [TARGET ABSENT IN THIS PANE; DELETION/INSERTION POINT]\n");
    }
}

fn append_merge_ai_middle_window(
    output: &mut String,
    document: &MergeDocument,
    target_start: usize,
    target_end: usize,
) {
    let start = target_start.saturating_sub(MERGE_AI_TARGET_CONTEXT_RADIUS);
    let end = target_end
        .saturating_add(MERGE_AI_TARGET_CONTEXT_RADIUS)
        .min(document.lines.len());
    output.push_str(
        "MIDDLE resolved surroundings (the unresolved target is deliberately omitted):\n",
    );
    for index in start..end {
        if index == target_start {
            output.push_str(">> [UNRESOLVED TARGET OMITTED; DO NOT INFER KEEP OR DELETE]\n");
        }
        if (target_start..target_end).contains(&index) {
            continue;
        }
        let line = &document.lines[index];
        if line.include_in_result {
            output.push_str(&format!("   {:>6}: {}\n", index + 1, line.result));
        }
    }
}

fn merge_ai_prompt(
    args: &MergeArgs,
    sources: &MergeSourceText,
    document: &MergeDocument,
    context: &MergeAiContext,
) -> String {
    let conflicts = document
        .conflicts()
        .iter()
        .map(|conflict| {
            format!(
                "\n## Conflict {}\nBASE:\n{}\nLEFT:\n{}\nRIGHT:\n{}\n",
                conflict.index,
                conflict.base.join("\n"),
                conflict.local.join("\n"),
                conflict.remote.join("\n"),
            )
        })
        .collect::<String>();
    let deletions = base_only_display_groups(document)
        .into_iter()
        .map(|group| {
            let lines = &document.lines[group.line_index..group.line_index + group.line_count];
            let side_name = match group.missing_side {
                MergeSide::Local => "LEFT",
                MergeSide::Remote => "RIGHT",
            };
            format!(
                "\n## Deletion {}\nThe {} pane deleted this block. Choosing {} accepts the deletion; choosing the other pane keeps the block.\nBASE:\n{}\nLEFT:\n{}\nRIGHT:\n{}\n",
                group.line_index,
                side_name,
                side_name,
                lines
                    .iter()
                    .filter_map(|line| line.base.as_deref())
                    .collect::<Vec<_>>()
                    .join("\n"),
                lines
                    .iter()
                    .filter_map(|line| line.local.as_deref())
                    .collect::<Vec<_>>()
                    .join("\n"),
                lines
                    .iter()
                    .filter_map(|line| line.remote.as_deref())
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        })
        .collect::<String>();
    let target_neighborhoods = merge_ai_target_neighborhoods(sources, document);
    let source_excerpts = format!(
        "FILE PREFIX EXCERPTS (may be truncated; never infer target absence from these prefixes):\nBASE FILE PREFIX:\n{}\nLEFT FILE PREFIX:\n{}\nRIGHT FILE PREFIX:\n{}\nRESOLVED MIDDLE RESULT (all unresolved targets are omitted; target absence is not a deletion decision):\n{}",
        truncate_merge_ai_text(&sources.base, 20 * 1024),
        truncate_merge_ai_text(&sources.local, 20 * 1024),
        truncate_merge_ai_text(&sources.remote, 20 * 1024),
        truncate_merge_ai_text(&merge_ai_editable_middle_text(document), 32 * 1024),
    );
    format!(
        "Analyze a merge for `{}`. Before choosing a side, reconcile each target with TARGET-LOCAL NEIGHBORHOODS, the RESOLVED MIDDLE RESULT, and CURRENT REPOSITORY MERGE STATE, because non-conflicting edits may make a line from either side obsolete. Middle contains only already-resolved code outside unresolved targets. Never use an unresolved target's provisional editor row, or its deliberate omission from Middle, as evidence to keep or delete it. For deletions, compare Base→Left and Base→Right intent in the supplied neighborhoods and check whether the changed side moved, consolidated, or replaced the same operation elsewhere. If a broader or shared execution point in the resolved Middle already covers the deleted location, retaining the old copy duplicates behavior; keep both only with concrete evidence that they still serve distinct responsibilities. For imports and declarations, explicitly verify current usage in the resolved Middle result and SYMBOL REFERENCES; absence of usage is evidence for removal, not preservation. When either side adds, removes, renames, or reorders array elements, object properties, method or function parameters and arguments, enum members, or ordered operations, inspect the complete Middle result plus related definitions and callers for resulting effects before choosing. Use branch-specific history to infer intent, but choose manual if the evidence conflicts or the correct result is neither side verbatim. When the correct target is neither side verbatim and evidence is sufficient, set manual_result_provided=true and put the exact complete target replacement in manual_result. Put any additional required edits to already-resolved Middle code in middle_edits. This includes non-diff lines. Each expected_text must occur exactly once inside one logical Middle line; replacement_text may contain multiple lines, and insertions must retain the chosen anchor text in replacement_text. Do not use line numbers or Markdown fences. For left/right choices, or when evidence is insufficient, use manual_result_provided=false, manual_result=\"\", and middle_edits=[] unless the selected side mechanically requires a proven additional Middle edit. For every suggestion, write both `reason_zh` in Simplified Chinese and `reason_en` in English. Each reason must be exactly one or two short sentences: the first states the recommendation, and the optional second cites only the decisive Middle/history/reference evidence. Do not repeat code, merge order, or the full analysis. Also always return `merge_order_zh` and `merge_order_en` as one short sentence: for a manual result that retains both sides, state the exact before/after or precedence order; for control-flow branches analyze whether conditions overlap and state which branch wins when both match, never asserting mutual exclusivity without concrete evidence; for parallel declarations/fields state that no runtime order exists and give their structural placement; when evidence is insufficient state the candidate orders and how behavior differs. Never respond only that both sides should be kept. For left/right choices use 不适用 and Not applicable. In those reasons call the panes 左边/中间/右边 and Left/Middle/Right respectively; never infer or say local branch or remote branch. Call `{MERGE_AI_TOOL_NAME}` exactly once and include exactly one suggestion for every listed conflict and deletion decision, with no extra targets. The call is advisory only: do not claim that any file or merge result was changed.\n\nCONFLICTS:{conflicts}\n\nDELETION DECISIONS:{deletions}\n\nTARGET-LOCAL NEIGHBORHOODS (these are authoritative for code around each target):\n{target_neighborhoods}\n\nGIT HISTORY:\n{}\n\nCURRENT REPOSITORY MERGE STATE:\n{}\n\nSYMBOL REFERENCES IN CURRENT WORKTREE:\n{}\n\nRELATED FILE CONTEXT:{}\n\n{}",
        args.output.display(),
        if context.history.is_empty() {
            "(unavailable)"
        } else {
            &context.history
        },
        if context.repository_state.is_empty() {
            "(unavailable)"
        } else {
            &context.repository_state
        },
        if context.symbol_references.is_empty() {
            "(none found)"
        } else {
            &context.symbol_references
        },
        if context.related_files.is_empty() {
            "(none found)"
        } else {
            &context.related_files
        },
        source_excerpts,
    )
}

fn truncate_merge_ai_text(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    let mut truncated = value.chars().take(max_chars).collect::<String>();
    truncated.push_str("\n[context truncated]");
    truncated
}

fn compact_merge_ai_text(value: &str, max_chars: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = normalized.chars();
    let compact = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{compact}…")
    } else {
        compact
    }
}

fn normalize_merge_ai_code(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

impl App for MergeToolApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let frame_started = Instant::now();
        self.poll_load_task(ctx);
        self.poll_result_highlight(ctx);
        self.poll_write_task(ctx);
        self.poll_ai_task(ctx);
        self.handle_close_request(ctx);
        let palette = merge_palette(self.theme);
        apply_merge_theme(ctx, self.theme);

        egui::TopBottomPanel::top("merge_window_titlebar")
            .exact_height(MERGE_WINDOW_TITLEBAR_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(palette.panel)
                    .stroke(egui::Stroke::NONE),
            )
            .show(ctx, |ui| merge_custom_title_bar(ui, ctx, self, palette));

        if self.load_task.is_some() {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(palette.bg))
                .show(ctx, |ui| merge_loading_panel(ui, self, palette));
            self.record_frame_performance(frame_started.elapsed());
            return;
        }

        self.handle_keyboard_shortcuts(ctx);
        egui::TopBottomPanel::top("merge_toolbar")
            .exact_height(38.0)
            .frame(
                egui::Frame::new()
                    .fill(palette.panel)
                    .stroke(egui::Stroke::NONE),
            )
            .show(ctx, |ui| merge_toolbar(ui, self, palette));

        egui::TopBottomPanel::bottom("merge_footer")
            .exact_height(56.0)
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ctx, |ui| merge_footer(ui, self, palette));

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.bg))
            .show(ctx, |ui| merge_editor_columns(ui, self, palette));

        merge_cancel_confirm_dialog(ctx, self, palette);
        self.record_frame_performance(frame_started.elapsed());
    }
}

fn split_lines(text: &str) -> Vec<String> {
    text.lines().map(ToOwned::to_owned).collect()
}

fn read_text(path: &Path) -> anyhow::Result<String> {
    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

fn load_merge_document(
    args: &MergeArgs,
    mut report: impl FnMut(MergeLoadProgress),
) -> anyhow::Result<PreparedMergeDocument> {
    let started_at = Instant::now();
    report(MergeLoadProgress {
        stage: MergeLoadStage::ReadingFiles,
        total_bytes: 0,
        total_lines: 0,
    });
    let base = read_text(&args.base)?;
    let local = read_text(&args.local)?;
    let remote = read_text(&args.remote)?;
    let read_elapsed = started_at.elapsed();
    let total_bytes = base.len() + local.len() + remote.len();
    let total_lines = [
        base.lines().count(),
        local.lines().count(),
        remote.lines().count(),
    ]
    .into_iter()
    .max()
    .unwrap_or(0);

    report(MergeLoadProgress {
        stage: MergeLoadStage::ComparingChanges,
        total_bytes,
        total_lines,
    });
    let compare_started_at = Instant::now();
    let document = three_way_merge(&base, &local, &remote);
    let compare_elapsed = compare_started_at.elapsed();

    report(MergeLoadProgress {
        stage: MergeLoadStage::PreparingEditor,
        total_bytes,
        total_lines,
    });
    let prepare_started_at = Instant::now();
    let prepared = prepare_merge_document(
        args,
        document,
        MergeSourceText {
            base,
            local,
            remote,
        },
    );
    let prepare_elapsed = prepare_started_at.elapsed();
    let load_fields = format!(
        "output={} bytes={} lines={} read_ms={} compare_ms={} prepare_ms={} total_ms={} document_lines={} result_rows={} local_rows={} remote_rows={} conflicts={} deletion_groups={}",
        args.output.display(),
        total_bytes,
        total_lines,
        read_elapsed.as_millis(),
        compare_elapsed.as_millis(),
        prepare_elapsed.as_millis(),
        started_at.elapsed().as_millis(),
        prepared.document.lines.len(),
        prepared.result_display_rows.len(),
        prepared.local_display_rows.len(),
        prepared.remote_display_rows.len(),
        prepared.document.conflicts().len(),
        prepared.geometry_cache.base_only_groups.len(),
    );
    crate::diagnostics::merge_tool_info("load.finished", &load_fields);
    eprintln!(
        "[merge-load] file={} bytes={} lines={} read_ms={} compare_ms={} prepare_ms={} total_ms={}",
        args.output.display(),
        total_bytes,
        total_lines,
        read_elapsed.as_millis(),
        compare_elapsed.as_millis(),
        prepare_elapsed.as_millis(),
        started_at.elapsed().as_millis(),
    );
    Ok(prepared)
}

fn prepare_merge_document(
    args: &MergeArgs,
    document: MergeDocument,
    sources: MergeSourceText,
) -> PreparedMergeDocument {
    let result_text = document.result_text();
    let result_display_rows = merge_result_display_rows(&document);
    let manual_result_lines = result_display_rows
        .iter()
        .map(|row| row.text.to_owned())
        .collect::<Vec<_>>();
    let local_display_rows = cached_merge_side_display_rows(&document, MergeSide::Local);
    let remote_display_rows = cached_merge_side_display_rows(&document, MergeSide::Remote);
    let local_scroll_anchors = merge_cached_scroll_anchors(
        &document,
        MergeSide::Local,
        &result_display_rows,
        &local_display_rows,
    );
    let remote_scroll_anchors = merge_cached_scroll_anchors(
        &document,
        MergeSide::Remote,
        &result_display_rows,
        &remote_display_rows,
    );
    let result_display_rows = cached_merge_result_display_rows(&result_display_rows);
    let geometry_cache = merge_geometry_cache(
        &document,
        &result_display_rows,
        &local_display_rows,
        &remote_display_rows,
    );
    let local_navigation_target = merge_navigation_targets(&document, MergeSide::Local)
        .first()
        .copied();
    let remote_navigation_target = merge_navigation_targets(&document, MergeSide::Remote)
        .first()
        .copied();
    let initial_document = document.clone();
    let repository_root = merge_syntax_repository_root(args);
    let path = merge_syntax_path(args);
    let local_highlight =
        crate::syntax::highlight_document(&repository_root, &path, &sources.local);
    let remote_highlight =
        crate::syntax::highlight_document(&repository_root, &path, &sources.remote);
    let local_source_lines = local_highlight
        .is_some()
        .then(|| sources.local.lines().map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or_default();
    let remote_source_lines = remote_highlight
        .is_some()
        .then(|| {
            sources
                .remote
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let syntax_highlights = MergeSyntaxHighlights {
        local_unique_source_lines: merge_unique_source_line_indices(&local_source_lines),
        remote_unique_source_lines: merge_unique_source_line_indices(&remote_source_lines),
        local_source_lines,
        remote_source_lines,
        local: local_highlight,
        remote: remote_highlight,
        result: crate::syntax::highlight_document(
            &repository_root,
            &path,
            &merge_highlight_source_from_lines(&manual_result_lines),
        ),
    };
    PreparedMergeDocument {
        initial_document,
        document,
        sources,
        result_text,
        manual_result_lines,
        result_display_rows,
        local_display_rows,
        remote_display_rows,
        geometry_cache,
        local_scroll_anchors,
        remote_scroll_anchors,
        local_navigation_target,
        remote_navigation_target,
        syntax_highlights,
    }
}

fn merge_syntax_repository_root(args: &MergeArgs) -> PathBuf {
    args.repo_root
        .clone()
        .or_else(|| args.output.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn merge_syntax_path(args: &MergeArgs) -> String {
    let relative = args
        .repo_root
        .as_deref()
        .and_then(|root| args.output.strip_prefix(root).ok())
        .unwrap_or(&args.output);
    relative.to_string_lossy().replace('\\', "/")
}

fn highlight_merge_source(args: &MergeArgs, source: &str) -> Option<HighlightedDocument> {
    crate::syntax::highlight_document(
        &merge_syntax_repository_root(args),
        &merge_syntax_path(args),
        source,
    )
}

fn merge_highlight_source_from_lines(lines: &[String]) -> String {
    let mut source = lines.join("\n");
    if !source.is_empty() {
        source.push('\n');
    }
    source
}

fn parse_theme(value: &str) -> anyhow::Result<MergeTheme> {
    match value.to_ascii_lowercase().as_str() {
        "dark" | "night" => Ok(MergeTheme::Dark),
        "light" | "day" => Ok(MergeTheme::Light),
        _ => Err(anyhow!("unknown theme {value}")),
    }
}

fn parse_language(value: &str) -> anyhow::Result<MergeLanguage> {
    match value.to_ascii_lowercase().as_str() {
        "en" | "english" => Ok(MergeLanguage::English),
        "zh" | "cn" | "chinese" => Ok(MergeLanguage::Chinese),
        _ => Err(anyhow!("unknown language {value}")),
    }
}

const MERGE_COLUMN_GAP: f32 = 12.0;
const MERGE_OVERVIEW_WIDTH: f32 = 22.0;
const MERGE_OVERVIEW_GAP: f32 = 6.0;
const MERGE_OVERVIEW_COLUMN_WIDTH: f32 = 4.0;
const MERGE_WINDOW_TITLEBAR_HEIGHT: f32 = 32.0;

fn merge_custom_title_bar(
    ui: &mut Ui,
    ctx: &egui::Context,
    app: &MergeToolApp,
    palette: MergePalette,
) {
    let rect = ui.max_rect();
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(8), palette.panel);

    let controls_width = 112.0;
    let drag_rect = Rect::from_min_max(
        rect.left_top(),
        Pos2::new(rect.right() - controls_width, rect.bottom()),
    );
    let drag_response = ui.interact(
        drag_rect,
        ui.id().with("merge_window_title_drag"),
        Sense::click_and_drag(),
    );
    if drag_response.drag_started() {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if drag_response.double_clicked() {
        let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }

    let controls_rect = Rect::from_min_max(
        Pos2::new(rect.right() - controls_width, rect.top() + 4.0),
        Pos2::new(rect.right() - 7.0, rect.bottom() - 4.0),
    );
    let title_rect = Rect::from_min_max(
        Pos2::new(rect.left() + 10.0, rect.top()),
        Pos2::new(controls_rect.left() - 8.0, rect.bottom()),
    );
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(title_rect), |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Git Agent Merge")
                    .strong()
                    .color(palette.text),
            );
            if let Some(name) = app.args.output.file_name().and_then(|name| name.to_str()) {
                ui.label(RichText::new(format!("- {name}")).color(palette.muted));
            }
        });
    });
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(controls_rect), |ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if merge_window_control_button(ui, "×", true, palette).clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
            if merge_window_control_button(ui, if maximized { "❐" } else { "□" }, false, palette)
                .clicked()
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            }
            if merge_window_control_button(ui, "−", false, palette).clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
        });
    });
}

fn merge_loading_panel(ui: &mut Ui, app: &MergeToolApp, palette: MergePalette) {
    let elapsed = app
        .load_started_at
        .map(|started_at| started_at.elapsed().as_secs_f32())
        .unwrap_or_default();
    let file_name = app
        .args
        .output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| app.args.output.to_str().unwrap_or("merge result"));
    let available = ui.available_rect_before_wrap();
    let card_rect = merge_loading_card_rect(available);

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(card_rect), |ui| {
        egui::Frame::new()
            .fill(palette.panel)
            .shadow(palette.shadow)
            .corner_radius(egui::CornerRadius::same(12))
            .inner_margin(egui::Margin::same(24))
            .show(ui, |ui| {
                let content_size = card_rect.size() - Vec2::splat(48.0);
                ui.set_min_size(content_size);
                ui.set_max_size(content_size);

                let (header_rect, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 42.0), Sense::hover());
                let spinner_rect = Rect::from_min_size(header_rect.min, Vec2::splat(40.0));
                let elapsed_rect = Rect::from_center_size(
                    Pos2::new(header_rect.right() - 34.0, header_rect.center().y),
                    Vec2::new(68.0, 28.0),
                );
                let title_rect = Rect::from_min_max(
                    Pos2::new(spinner_rect.right() + 14.0, header_rect.top()),
                    Pos2::new(elapsed_rect.left() - 12.0, header_rect.bottom()),
                );
                ui.painter().circle_filled(
                    spinner_rect.center(),
                    20.0,
                    color_with_opacity(palette.accent, 0.10),
                );
                ui.put(
                    spinner_rect.shrink(9.0),
                    egui::Spinner::new().size(22.0).color(palette.accent),
                );
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(title_rect), |ui| {
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.label(
                            RichText::new(mt(app.language, "loading_title"))
                                .size(18.0)
                                .strong()
                                .color(palette.text),
                        );
                        ui.label(
                            RichText::new(merge_loading_active_stage_label(
                                app.language,
                                app.load_progress.stage,
                            ))
                            .size(11.0)
                            .color(palette.accent),
                        );
                    });
                });
                ui.painter().rect_filled(
                    elapsed_rect,
                    egui::CornerRadius::same(14),
                    palette.panel_soft,
                );
                ui.painter().text(
                    elapsed_rect.center(),
                    Align2::CENTER_CENTER,
                    format!("{elapsed:.1} s"),
                    FontId::proportional(11.0),
                    palette.muted,
                );

                ui.add_space(14.0);
                egui::Frame::new()
                    .fill(palette.panel_soft)
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.add(
                            egui::Label::new(
                                RichText::new(file_name)
                                    .size(13.0)
                                    .strong()
                                    .color(palette.text),
                            )
                            .truncate(),
                        );
                        let scale = merge_loading_scale_label(app.language, app.load_progress);
                        if !scale.is_empty() {
                            ui.label(RichText::new(scale).size(11.0).color(palette.muted));
                        }
                    });
                ui.add_space(18.0);
                merge_loading_progress_track(ui, app.load_progress.stage, app.language, palette);
            });
    });
}

fn merge_loading_card_rect(available: Rect) -> Rect {
    let margin = Vec2::splat(24.0);
    let maximum = (available.size() - margin * 2.0).max(Vec2::ZERO);
    let size = Vec2::new(
        MERGE_LOADING_CARD_WIDTH.min(maximum.x),
        MERGE_LOADING_CARD_HEIGHT.min(maximum.y),
    );
    Rect::from_center_size(available.center(), size)
}

fn merge_loading_active_stage_label(
    language: MergeLanguage,
    current_stage: MergeLoadStage,
) -> String {
    let label_key = match current_stage {
        MergeLoadStage::ReadingFiles => "loading_reading",
        MergeLoadStage::ComparingChanges => "loading_comparing",
        MergeLoadStage::PreparingEditor => "loading_preparing",
    };
    format!(
        "{} · {}",
        mt(language, label_key),
        mt(language, "loading_active")
    )
}

fn merge_loading_progress_track(
    ui: &mut Ui,
    current_stage: MergeLoadStage,
    language: MergeLanguage,
    palette: MergePalette,
) {
    let current_index = merge_load_stage_index(current_stage);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 62.0), Sense::hover());
    let slot_width = rect.width() / 3.0;
    let centers = (0..3)
        .map(|index| {
            Pos2::new(
                rect.left() + slot_width * (index as f32 + 0.5),
                rect.top() + 9.0,
            )
        })
        .collect::<Vec<_>>();
    let painter = ui.painter();
    painter.line_segment(
        [centers[0], centers[2]],
        egui::Stroke::new(2.0, color_with_opacity(palette.muted, 0.22)),
    );
    painter.line_segment(
        [centers[0], centers[current_index]],
        egui::Stroke::new(2.0, palette.accent),
    );

    for (index, (label_key, center)) in
        ["loading_reading", "loading_comparing", "loading_preparing"]
            .into_iter()
            .zip(centers)
            .enumerate()
    {
        let completed = index < current_index;
        let active = index == current_index;
        if completed || active {
            painter.circle_filled(center, 7.0, palette.accent);
        } else {
            painter.circle_filled(center, 7.0, palette.panel);
            painter.circle_stroke(
                center,
                7.0,
                egui::Stroke::new(1.5, color_with_opacity(palette.muted, 0.46)),
            );
        }
        if completed {
            let stroke = egui::Stroke::new(1.4, Color32::WHITE);
            painter.line_segment(
                [center + Vec2::new(-2.4, 0.0), center + Vec2::new(-0.5, 2.0)],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(-0.5, 2.0), center + Vec2::new(3.0, -2.2)],
                stroke,
            );
        } else if active {
            painter.circle_filled(center, 2.5, palette.panel);
        }
        painter.text(
            Pos2::new(center.x, rect.top() + 24.0),
            Align2::CENTER_TOP,
            mt(language, label_key),
            FontId::proportional(12.0),
            if active { palette.text } else { palette.muted },
        );
        let state_key = if completed {
            "loading_done"
        } else if active {
            "loading_active"
        } else {
            "loading_waiting"
        };
        painter.text(
            Pos2::new(center.x, rect.top() + 43.0),
            Align2::CENTER_TOP,
            mt(language, state_key),
            FontId::proportional(10.0),
            if active {
                palette.accent
            } else {
                palette.muted
            },
        );
    }
}

fn merge_load_stage_index(stage: MergeLoadStage) -> usize {
    match stage {
        MergeLoadStage::ReadingFiles => 0,
        MergeLoadStage::ComparingChanges => 1,
        MergeLoadStage::PreparingEditor => 2,
    }
}

fn merge_loading_scale_label(language: MergeLanguage, progress: MergeLoadProgress) -> String {
    if progress.total_bytes == 0 && progress.total_lines == 0 {
        return String::new();
    }
    let size = merge_format_bytes(progress.total_bytes);
    match language {
        MergeLanguage::Chinese => format!("{size} · 最长版本 {} 行", progress.total_lines),
        MergeLanguage::English => {
            format!("{size} · longest version {} lines", progress.total_lines)
        }
    }
}

fn merge_format_bytes(bytes: usize) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    if bytes as f64 >= MIB {
        format!("{:.1} MB", bytes as f64 / MIB)
    } else if bytes as f64 >= KIB {
        format!("{:.0} KB", bytes as f64 / KIB)
    } else {
        format!("{bytes} B")
    }
}

fn merge_window_control_button(
    ui: &mut Ui,
    label: &str,
    close: bool,
    palette: MergePalette,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(34.0, 22.0), Sense::click());
    let fill = if close {
        if response.hovered() {
            Color32::from_rgb(192, 55, 43)
        } else {
            Color32::TRANSPARENT
        }
    } else if response.hovered() {
        palette.panel_soft
    } else {
        Color32::TRANSPARENT
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(4), fill);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(16.0),
        if close && response.hovered() {
            Color32::WHITE
        } else {
            palette.text
        },
    );
    response
}

#[cfg(target_os = "windows")]
fn prefer_rounded_merge_window_corners(cc: &eframe::CreationContext<'_>) {
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;

    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_ROUND: u32 = 2;
    let Ok(window_handle) = cc.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = window_handle.as_raw() else {
        return;
    };
    let preference = DWMWCP_ROUND;
    unsafe {
        let _ = DwmSetWindowAttribute(
            handle.hwnd.get() as _,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const u32 as _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn prefer_rounded_merge_window_corners(_: &eframe::CreationContext<'_>) {}

fn merge_toolbar(ui: &mut Ui, app: &mut MergeToolApp, palette: MergePalette) {
    let unresolved_conflicts = app.unresolved_conflict_count();
    let row_rect = ui.max_rect().shrink2(Vec2::new(
        8.0,
        (ui.max_rect().height() - 28.0).max(0.0) * 0.5,
    ));
    ui.allocate_new_ui(
        egui::UiBuilder::new()
            .max_rect(row_rect)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.spacing_mut().interact_size.y = 24.0;
            ui.label(
                RichText::new(mt(app.language, "title"))
                    .strong()
                    .color(palette.text),
            );
            ui.add_space(10.0);
            ui.label(
                RichText::new(format!(
                    "{} {}",
                    unresolved_conflicts,
                    mt(app.language, "conflicts")
                ))
                .monospace()
                .color(if unresolved_conflicts > 0 {
                    palette.conflict_text
                } else {
                    palette.muted
                }),
            );
            ui.add_space(10.0);
            ui.label(RichText::new(mt(app.language, "auto_applied")).color(palette.muted));
            ui.add_space(16.0);
            let controls_width = ui.available_width();
            ui.allocate_ui_with_layout(
                Vec2::new(controls_width, ui.available_height()),
                Layout::right_to_left(Align::Center),
                |ui| {
                    let ai_loading = app.ai_task.is_some();
                    if merge_toolbar_ai_button(ui, app.language, ai_loading, palette).clicked() {
                        app.request_ai_analysis();
                    }
                    if ai_loading {
                        ui.label(
                            RichText::new(mt(app.language, "ai_analyzing"))
                                .small()
                                .color(palette.accent),
                        );
                    } else if !app.ai_suggestions.is_empty() {
                        let count = app.actionable_ai_suggestion_count();
                        if ui
                            .add_enabled(
                                app.ai_suggestion_actions_enabled() && count > 0,
                                egui::Button::new(format!(
                                    "{} ({count})",
                                    mt(app.language, "ai_apply_all")
                                )),
                            )
                            .on_hover_text(mt(app.language, "ai_apply_all_hint"))
                            .clicked()
                        {
                            app.apply_all_ai_suggestions();
                            ui.ctx().request_repaint();
                        }
                    } else if let Some(notice) = app.ai_notice {
                        let text = match notice {
                            MergeAiNotice::Completed {
                                suggestions,
                                changes,
                            } => format!(
                                "{} {suggestions} {} · {changes} {}",
                                mt(app.language, "ai_completed_prefix"),
                                mt(app.language, "ai_completed_suffix"),
                                mt(app.language, "ai_changes_suffix")
                            ),
                            MergeAiNotice::NoSuggestions => {
                                mt(app.language, "ai_no_suggestions").to_owned()
                            }
                        };
                        ui.label(RichText::new(text).small().color(palette.accent));
                    }
                    if merge_toolbar_toggle_button(
                        ui,
                        MergeToolbarToggleIcon::Language,
                        merge_language_label(app.language),
                        false,
                        palette,
                    )
                    .clicked()
                    {
                        app.toggle_language();
                    }
                    if merge_toolbar_toggle_button(
                        ui,
                        match app.theme {
                            MergeTheme::Dark => MergeToolbarToggleIcon::Moon,
                            MergeTheme::Light => MergeToolbarToggleIcon::Sun,
                        },
                        merge_theme_label(app.language, app.theme),
                        false,
                        palette,
                    )
                    .clicked()
                    {
                        app.toggle_theme(ui.ctx());
                    }
                    let collapse_label = if app.collapse_unchanged {
                        mt(app.language, "expand_unchanged")
                    } else {
                        mt(app.language, "collapse_unchanged")
                    };
                    if merge_toolbar_toggle_button(
                        ui,
                        if app.collapse_unchanged {
                            MergeToolbarToggleIcon::Expand
                        } else {
                            MergeToolbarToggleIcon::Collapse
                        },
                        collapse_label,
                        false,
                        palette,
                    )
                    .clicked()
                    {
                        app.collapse_unchanged = !app.collapse_unchanged;
                        // The displayed document becomes shorter or longer. Do not carry an offset from
                        // the previous shape into the new ScrollAreas.
                        app.shared_scroll_y = 0.0;
                        app.rebuild_display_rows();
                    }
                    merge_highlight_mode_combo(ui, app, palette);
                    merge_ignore_mode_combo(ui, app, palette);
                    ui.label(
                        RichText::new(format!(
                            "{} {} {}",
                            mt(app.language, "no_changes"),
                            unresolved_conflicts,
                            mt(app.language, "conflict_count")
                        ))
                        .color(palette.muted),
                    );
                    if let Some(status) = &app.status {
                        ui.label(RichText::new(status).color(palette.conflict_text));
                    }
                    if let Some(error) = &app.ai_analysis_error {
                        ui.label(RichText::new(error).small().color(palette.conflict_text));
                    }
                },
            );
        },
    );
}

fn merge_toolbar_ai_button(
    ui: &mut Ui,
    language: MergeLanguage,
    loading: bool,
    palette: MergePalette,
) -> egui::Response {
    if !loading {
        return merge_toolbar_toggle_button(
            ui,
            MergeToolbarToggleIcon::Ai,
            mt(language, "ai_analyze"),
            false,
            palette,
        );
    }
    let output = egui::Frame::new()
        .fill(palette.accent)
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::same(5))
        .show(ui, |ui| {
            ui.add(egui::Spinner::new().size(18.0).color(Color32::WHITE));
        });
    output.response.on_hover_text(mt(language, "ai_analyzing"))
}

fn merge_toolbar_toggle_button(
    ui: &mut Ui,
    icon: MergeToolbarToggleIcon,
    tooltip: &str,
    active: bool,
    palette: MergePalette,
) -> egui::Response {
    let icon_color = if active {
        Color32::WHITE
    } else {
        palette.muted
    };
    let image = egui::Image::new(merge_toolbar_icon_source(icon))
        .fit_to_exact_size(Vec2::splat(15.0))
        .tint(icon_color);
    let response = ui.add(
        egui::Button::image(image)
            .min_size(Vec2::splat(28.0))
            .fill(if active {
                palette.accent
            } else {
                Color32::TRANSPARENT
            })
            .stroke(egui::Stroke::NONE)
            .corner_radius(egui::CornerRadius::same(4)),
    );
    response.on_hover_text(tooltip)
}

fn merge_toolbar_icon_source(icon: MergeToolbarToggleIcon) -> egui::ImageSource<'static> {
    match icon {
        MergeToolbarToggleIcon::Ai => egui::include_image!("../assets/icons/merge-ai.svg"),
        MergeToolbarToggleIcon::Collapse => {
            egui::include_image!("../assets/icons/merge-collapse.svg")
        }
        MergeToolbarToggleIcon::Expand => egui::include_image!("../assets/icons/merge-expand.svg"),
        MergeToolbarToggleIcon::Sun => egui::include_image!("../assets/icons/merge-sun.svg"),
        MergeToolbarToggleIcon::Moon => egui::include_image!("../assets/icons/merge-moon.svg"),
        MergeToolbarToggleIcon::Language => {
            egui::include_image!("../assets/icons/merge-language.svg")
        }
    }
}

fn merge_highlight_mode_combo(ui: &mut Ui, app: &mut MergeToolApp, palette: MergePalette) {
    ComboBox::from_id_salt("merge_highlight_mode")
        .width(124.0)
        .selected_text(
            RichText::new(merge_highlight_mode_label(app.language, app.highlight_mode))
                .size(11.0)
                .color(palette.text),
        )
        .show_ui(ui, |ui| {
            for mode in [MergeHighlightMode::Lines, MergeHighlightMode::Words] {
                ui.selectable_value(
                    &mut app.highlight_mode,
                    mode,
                    merge_highlight_mode_label(app.language, mode),
                );
            }
        });
}

fn merge_ignore_mode_combo(ui: &mut Ui, app: &mut MergeToolApp, palette: MergePalette) {
    let mut selected = app.ignore_mode;
    ComboBox::from_id_salt("merge_ignore_mode")
        .width(132.0)
        .selected_text(
            RichText::new(merge_ignore_mode_label(app.language, selected))
                .size(11.0)
                .color(palette.text),
        )
        .show_ui(ui, |ui| {
            for mode in [
                MergeIgnoreMode::None,
                MergeIgnoreMode::TrimWhitespace,
                MergeIgnoreMode::IgnoreWhitespace,
            ] {
                ui.selectable_value(
                    &mut selected,
                    mode,
                    merge_ignore_mode_label(app.language, mode),
                );
            }
        });
    if selected != app.ignore_mode {
        app.reset_from_sources(selected);
    }
}

fn merge_footer(ui: &mut Ui, app: &mut MergeToolApp, palette: MergePalette) {
    let writing = app.write_task.is_some();
    let can_apply = app.can_apply_result();
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        if ui
            .add_enabled(
                !app.manual_result_override,
                egui::Button::new(mt(app.language, "accept_left")),
            )
            .clicked()
        {
            app.accept_conflict(MergeSide::Local);
        }
        if ui
            .add_enabled(
                !app.manual_result_override,
                egui::Button::new(mt(app.language, "accept_right")),
            )
            .clicked()
        {
            app.accept_conflict(MergeSide::Remote);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(10.0);
            if ui
                .add_enabled(
                    !writing,
                    egui::Button::new(mt(app.language, "cancel")).min_size(Vec2::new(88.0, 30.0)),
                )
                .clicked()
                && app.request_cancel() == MergeCancelRequest::ExitNow
            {
                std::process::exit(MERGE_TOOL_CANCEL_EXIT_CODE);
            }
            let apply_label = if writing {
                mt(app.language, "applying")
            } else {
                mt(app.language, "apply")
            };
            if ui
                .add_enabled(
                    can_apply,
                    egui::Button::new(RichText::new(apply_label).strong().color(Color32::WHITE))
                        .min_size(Vec2::new(88.0, 30.0))
                        .fill(palette.accent),
                )
                .clicked()
            {
                app.write_output();
            }
        });
    });
}

fn merge_cancel_confirm_dialog(ctx: &egui::Context, app: &mut MergeToolApp, palette: MergePalette) {
    if !app.show_cancel_confirm {
        return;
    }

    let mut open = true;
    let mut discard = false;
    let mut continue_merge = false;
    egui::Window::new(mt(app.language, "cancel_merge_title"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_TOP, dialog::top_anchor_offset())
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.add_space(4.0);
            ui.label(RichText::new(mt(app.language, "cancel_merge_message")).color(palette.text));
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .button(mt(app.language, "cancel_merge_continue"))
                        .clicked()
                    {
                        continue_merge = true;
                    }
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(mt(app.language, "cancel_merge_discard"))
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(palette.accent),
                        )
                        .clicked()
                    {
                        discard = true;
                    }
                });
            });
        });

    if discard {
        std::process::exit(MERGE_TOOL_CANCEL_EXIT_CODE);
    }
    app.show_cancel_confirm = open && !continue_merge;
}

fn merge_pane_ui<R>(ui: &mut Ui, pane: Rect, body: impl FnOnce(&mut Ui) -> R) -> R {
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(pane), |ui| {
        // `UiBuilder::max_rect` is a layout hint, not a hard boundary: an oversized child can
        // expand the child Ui beyond it. Each merge pane must keep an explicit paint clip so a
        // wide editable line cannot cover its neighboring pane or the overview strip.
        ui.shrink_clip_rect(pane);
        body(ui)
    })
    .inner
}

fn merge_editor_columns(ui: &mut Ui, app: &mut MergeToolApp, palette: MergePalette) {
    let available = ui
        .available_rect_before_wrap()
        .shrink2(Vec2::new(10.0, 8.0));
    let rect = Rect::from_min_max(
        available.min,
        Pos2::new(
            available.right(),
            (available.bottom()
                - MERGE_HORIZONTAL_SCROLLBAR_HEIGHT
                - MERGE_HORIZONTAL_SCROLLBAR_GAP)
                .max(available.top()),
        ),
    );
    let gap = MERGE_COLUMN_GAP;
    let panes_width = rect.width() - MERGE_OVERVIEW_WIDTH - MERGE_OVERVIEW_GAP;
    let left_w = (panes_width * 0.32).max(250.0);
    let result_w = (panes_width * 0.34).max(280.0);
    let right_w = (panes_width - left_w - result_w - gap * 2.0).max(250.0);
    let left = Rect::from_min_size(rect.min, Vec2::new(left_w, rect.height()));
    let result = Rect::from_min_size(
        Pos2::new(left.right() + gap, rect.top()),
        Vec2::new(result_w, rect.height()),
    );
    let right = Rect::from_min_size(
        Pos2::new(result.right() + gap, rect.top()),
        Vec2::new(right_w, rect.height()),
    );
    let overview = Rect::from_min_size(
        Pos2::new(right.right() + MERGE_OVERVIEW_GAP, rect.top()),
        Vec2::new(MERGE_OVERVIEW_WIDTH, rect.height()),
    );
    app.hovered_search_pane = ui.ctx().pointer_hover_pos().and_then(|pointer| {
        if left.contains(pointer) {
            Some(MergeSearchPane::Left)
        } else if result.contains(pointer) {
            Some(MergeSearchPane::Middle)
        } else if right.contains(pointer) {
            Some(MergeSearchPane::Right)
        } else {
            None
        }
    });
    let horizontal_scrollbar = Rect::from_min_max(
        Pos2::new(left.left(), rect.bottom() + MERGE_HORIZONTAL_SCROLLBAR_GAP),
        Pos2::new(
            right.right(),
            rect.bottom() + MERGE_HORIZONTAL_SCROLLBAR_GAP + MERGE_HORIZONTAL_SCROLLBAR_HEIGHT,
        ),
    );
    let code_content_width = merge_code_content_width(ui, app, palette);
    let narrowest_code_viewport = [
        (left.width() - 12.0 - MERGE_SIDE_CODE_GUTTER_WIDTH).max(1.0),
        (result.width() - 12.0 - MERGE_RESULT_CODE_GUTTER_WIDTH).max(1.0),
        (right.width() - 12.0 - MERGE_SIDE_CODE_GUTTER_WIDTH).max(1.0),
    ]
    .into_iter()
    .fold(f32::INFINITY, f32::min);
    let max_scroll_x = (code_content_width - narrowest_code_viewport).max(0.0);
    let horizontal_scroll_delta = merge_horizontal_scroll_input(ui, left, result, right);
    let requested_scroll_x =
        (app.shared_scroll_x - horizontal_scroll_delta).clamp(0.0, max_scroll_x);
    if horizontal_scroll_delta.abs() > f32::EPSILON {
        ui.ctx().request_repaint();
    }
    // Capture side-pane wheel input before any ScrollArea has a chance to consume it. The side
    // panes are driven from result coordinates, so their programmatic offset can legitimately be
    // clamped when one side has fewer rows. That passive clamp must never be interpreted as user
    // input or it will write a smaller offset back and make bottom scrolling bounce upward.
    let side_scroll_input = merge_side_scroll_input(ui, left, right);

    let requested_scroll_y = app.shared_scroll_y;
    let mut result_output = MergeResultPanelOutput {
        scroll_y: requested_scroll_y,
        viewport_height: 0.0,
        search_result_y: None,
        geometry: MergePanelGeometry::default(),
    };
    // Collapsed tails replace many rows with one marker. Their display coordinates are no longer
    // the source-document coordinates used by the normal three-way anchor mapper.
    let use_direct_shared_scroll = app.collapse_unchanged;
    merge_pane_ui(ui, result, |ui| {
        result_output = merge_result_panel(
            ui,
            app,
            "merge_result_scroll",
            requested_scroll_x,
            code_content_width,
            requested_scroll_y,
            palette,
        );
    });
    let frame_scroll_y = result_output.scroll_y;
    let mut next_shared_scroll_y = frame_scroll_y;
    let local_scroll_y = if use_direct_shared_scroll {
        frame_scroll_y
    } else {
        app.cached_side_scroll_y_for_result_scroll(MergeSide::Local, frame_scroll_y)
    };
    let local_output = merge_pane_ui(ui, left, |ui| {
        merge_side_panel(
            ui,
            app,
            MergeSide::Local,
            "merge_local_scroll",
            requested_scroll_x,
            code_content_width,
            local_scroll_y,
            side_scroll_input.is_some_and(|(side, _)| side == MergeSide::Local),
            palette,
        )
    });
    let remote_scroll_y = if use_direct_shared_scroll {
        frame_scroll_y
    } else {
        app.cached_side_scroll_y_for_result_scroll(MergeSide::Remote, frame_scroll_y)
    };
    let remote_output = merge_pane_ui(ui, right, |ui| {
        merge_side_panel(
            ui,
            app,
            MergeSide::Remote,
            "merge_remote_scroll",
            requested_scroll_x,
            code_content_width,
            remote_scroll_y,
            side_scroll_input.is_some_and(|(side, _)| side == MergeSide::Remote),
            palette,
        )
    });

    // Navigation is a single result-coordinate action. Resolve it after both side panes render so
    // the other pane's ordinary scroll synchronization cannot overwrite the requested conflict.
    let navigation_target = remote_output
        .navigation_target
        .or(local_output.navigation_target);
    let stable_content_height = merge_result_content_height(app);
    next_shared_scroll_y = merge_next_shared_scroll_y(
        &app.document,
        next_shared_scroll_y,
        local_output.requested_result_scroll_y,
        remote_output.requested_result_scroll_y,
        navigation_target,
        result_output.viewport_height,
        stable_content_height,
        app.collapse_unchanged,
    );
    if let Some((_, scroll_delta_y)) = side_scroll_input {
        // ScrollArea uses `offset -= smooth_scroll_delta`. Apply the same movement directly in the
        // canonical result coordinate system; this also works when the pointed-at side contains
        // too few rows to scroll by itself.
        next_shared_scroll_y = merge_scroll_offset_after_input(
            frame_scroll_y,
            scroll_delta_y,
            result_output.viewport_height,
            stable_content_height,
        );
        ui.ctx().request_repaint();
    }
    // Search navigation, like conflict navigation, is resolved only after all three panes have
    // rendered. Writing `app.shared_scroll_y` from inside a pane is ineffective because this
    // outer synchronization pass still owns the frame's canonical offset and would overwrite it.
    let search_result_y = result_output
        .search_result_y
        .or(remote_output.search_result_y)
        .or(local_output.search_result_y);
    if let Some(search_result_y) = search_result_y {
        next_shared_scroll_y = merge_search_scroll_target(
            search_result_y,
            result_output.viewport_height,
            stable_content_height,
        );
        ui.ctx().request_repaint();
    }
    // Virtual ScrollAreas only record geometry for rows materialized in this frame.
    // The connector painter already ignores blocks without recorded geometry, so keep it
    // enabled for large files and paint only the currently visible conflict fragments.
    paint_merge_block_connectors(
        ui,
        &app.document,
        &app.geometry_cache,
        &local_output.geometry,
        &result_output.geometry,
        &remote_output.geometry,
        MergeConnectorColumns {
            local: left,
            result,
            remote: right,
        },
        app.local_conflict_cursor,
        app.remote_conflict_cursor,
        app.connector_debug,
        palette,
    );
    let ai_overlay_action = merge_ai_suggestion_overlays(
        ui.ctx(),
        app,
        &local_output.geometry,
        &result_output.geometry,
        &remote_output.geometry,
        palette,
    );
    if let Some(scroll_y) = merge_overview_target(
        ui,
        overview,
        app,
        next_shared_scroll_y,
        result_output.viewport_height,
        stable_content_height,
        palette,
    ) {
        next_shared_scroll_y = scroll_y;
        ui.ctx().request_repaint();
    }
    app.shared_scroll_x = merge_shared_horizontal_scrollbar(
        ui,
        horizontal_scrollbar,
        requested_scroll_x,
        narrowest_code_viewport,
        code_content_width,
        palette,
    );
    app.shared_scroll_y = next_shared_scroll_y;
    // Apply after every pane and connector used the same document snapshot. Mutating while the
    // local pane is rendering used to leave result/remote geometry from a different state.
    if let Some((target, side, action)) = local_output
        .pending_line_action
        .map(|(target, action)| (target, MergeSide::Local, action))
        .or_else(|| {
            remote_output
                .pending_line_action
                .map(|(target, action)| (target, MergeSide::Remote, action))
        })
    {
        app.apply_line_action(target, side, action);
        ui.ctx().request_repaint();
    }
    if let Some(action) = ai_overlay_action {
        match action {
            MergeAiOverlayAction::Apply(target) => app.apply_ai_suggestion(target),
            MergeAiOverlayAction::Ignore(target) => app.ignore_ai_suggestion(target),
        }
        ui.ctx().request_repaint();
    }
}

fn merge_side_scroll_input(ui: &Ui, local: Rect, remote: Rect) -> Option<(MergeSide, f32)> {
    let (scroll_delta_y, shift) = ui
        .ctx()
        .input(|input| (input.smooth_scroll_delta.y, input.modifiers.shift));
    if shift {
        return None;
    }
    if scroll_delta_y.abs() <= f32::EPSILON {
        return None;
    }
    let pointer = ui.ctx().pointer_hover_pos()?;
    if local.contains(pointer) {
        Some((MergeSide::Local, scroll_delta_y))
    } else if remote.contains(pointer) {
        Some((MergeSide::Remote, scroll_delta_y))
    } else {
        None
    }
}

fn merge_horizontal_scroll_input(ui: &Ui, local: Rect, result: Rect, remote: Rect) -> f32 {
    let (delta, shift) = ui
        .ctx()
        .input(|input| (input.smooth_scroll_delta, input.modifiers.shift));
    let horizontal_delta = if delta.x.abs() > f32::EPSILON {
        delta.x
    } else if shift {
        delta.y
    } else {
        0.0
    };
    if horizontal_delta.abs() <= f32::EPSILON {
        return 0.0;
    }
    let Some(pointer) = ui.ctx().pointer_hover_pos() else {
        return 0.0;
    };
    if local.contains(pointer) || result.contains(pointer) || remote.contains(pointer) {
        horizontal_delta
    } else {
        0.0
    }
}

fn merge_scroll_offset_after_input(
    current_scroll_y: f32,
    scroll_delta_y: f32,
    viewport_height: f32,
    content_height: f32,
) -> f32 {
    merge_clamp_scroll_offset(
        current_scroll_y - scroll_delta_y,
        content_height,
        viewport_height,
    )
}

fn merge_code_content_width(ui: &Ui, app: &MergeToolApp, palette: MergePalette) -> f32 {
    let longest_line_bytes = app
        .local_display_rows
        .iter()
        .map(|row| row.text.len())
        .chain(app.remote_display_rows.iter().map(|row| row.text.len()))
        .chain(app.manual_result_lines.iter().map(String::len))
        .max()
        .unwrap_or(0);
    let glyph_width = merge_text_width(
        ui,
        "M",
        &FontId::monospace(MERGE_CODE_FONT_SIZE),
        palette.text,
    )
    .max(1.0);
    longest_line_bytes as f32 * glyph_width + 24.0
}

fn merge_shared_horizontal_scrollbar(
    ui: &mut Ui,
    rect: Rect,
    current_scroll_x: f32,
    viewport_width: f32,
    content_width: f32,
    palette: MergePalette,
) -> f32 {
    let track = rect.shrink2(Vec2::new(2.0, 2.0));
    if track.width() <= 0.0 || track.height() <= 0.0 {
        return 0.0;
    }
    ui.painter().rect_filled(
        track,
        egui::CornerRadius::same(4),
        color_with_opacity(palette.panel_soft, 0.9),
    );
    let max_scroll_x = (content_width - viewport_width).max(0.0);
    if max_scroll_x <= f32::EPSILON {
        ui.painter().rect_filled(
            track,
            egui::CornerRadius::same(4),
            color_with_opacity(palette.muted, 0.22),
        );
        return 0.0;
    }

    let thumb_width = (track.width() * (viewport_width / content_width).clamp(0.0, 1.0))
        .clamp(32.0, track.width());
    let travel = (track.width() - thumb_width).max(0.0);
    let current_scroll_x = current_scroll_x.clamp(0.0, max_scroll_x);
    let thumb_left = track.left() + travel * (current_scroll_x / max_scroll_x);
    let thumb = Rect::from_min_size(
        Pos2::new(thumb_left, track.top()),
        Vec2::new(thumb_width, track.height()),
    );
    ui.painter().rect_filled(
        thumb,
        egui::CornerRadius::same(4),
        color_with_opacity(palette.accent, 0.72),
    );
    let response = ui
        .interact(
            rect,
            ui.make_persistent_id("merge_shared_horizontal_scrollbar"),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(CursorIcon::ResizeHorizontal);
    if !response.clicked() && !response.dragged() {
        return current_scroll_x;
    }
    let Some(pointer) = response.interact_pointer_pos() else {
        return current_scroll_x;
    };
    ui.ctx().request_repaint();
    merge_horizontal_scroll_target(track, pointer.x, thumb_width, max_scroll_x)
}

fn merge_horizontal_scroll_target(
    track: Rect,
    pointer_x: f32,
    thumb_width: f32,
    max_scroll_x: f32,
) -> f32 {
    let travel = (track.width() - thumb_width).max(0.0);
    if travel <= f32::EPSILON || max_scroll_x <= f32::EPSILON {
        return 0.0;
    }
    let thumb_left =
        (pointer_x - thumb_width * 0.5).clamp(track.left(), track.right() - thumb_width);
    ((thumb_left - track.left()) / travel * max_scroll_x).clamp(0.0, max_scroll_x)
}

fn merge_scrolled_code_text_rects(
    row_rect: Rect,
    gutter_width: f32,
    scroll_x: f32,
    content_width: f32,
) -> (Rect, Rect) {
    let clip_rect = Rect::from_min_max(
        Pos2::new(row_rect.left() + gutter_width, row_rect.top()),
        row_rect.right_bottom(),
    );
    let content_rect = Rect::from_min_size(
        Pos2::new(clip_rect.left() - scroll_x.max(0.0), row_rect.top()),
        Vec2::new(content_width.max(clip_rect.width()), row_rect.height()),
    );
    (clip_rect, content_rect)
}

fn merge_ai_suggestion_overlays(
    ctx: &egui::Context,
    app: &mut MergeToolApp,
    local_geometry: &MergePanelGeometry,
    result_geometry: &MergePanelGeometry,
    remote_geometry: &MergePanelGeometry,
    palette: MergePalette,
) -> Option<MergeAiOverlayAction> {
    if !app.ai_suggestion_actions_enabled() {
        return None;
    }
    // Copy only small stable keys. Cloning complete AI suggestions here used to duplicate long
    // reasons and replacement text on every frame, which made card dragging stall on large files.
    let suggestion_targets = app.ai_suggestions.keys().copied().collect::<Vec<_>>();
    let mut action = None;
    for suggestion_target in suggestion_targets {
        let Some(suggestion) = app.ai_suggestions.get(&suggestion_target) else {
            continue;
        };
        let local_action_anchor = merge_ai_action_anchor(
            &app.geometry_cache,
            local_geometry,
            &app.local_display_rows,
            suggestion.target,
            MergeSide::Local,
        );
        let remote_action_anchor = merge_ai_action_anchor(
            &app.geometry_cache,
            remote_geometry,
            &app.remote_display_rows,
            suggestion.target,
            MergeSide::Remote,
        );
        let (placement, anchor, connector_anchors) =
            match (local_action_anchor, remote_action_anchor) {
                (Some(local), Some(remote)) => {
                    let Some(anchor) = merge_ai_middle_anchor(
                        &app.geometry_cache,
                        result_geometry,
                        suggestion.target,
                        local,
                        remote,
                    ) else {
                        continue;
                    };
                    (MergeAiCardPlacement::Middle, anchor, vec![local, remote])
                }
                (Some(local), None) => {
                    let Some(anchor) = merge_ai_suggestion_anchor(
                        &app.geometry_cache,
                        local_geometry,
                        &app.local_display_rows,
                        suggestion.target,
                        MergeSide::Local,
                    ) else {
                        continue;
                    };
                    (
                        MergeAiCardPlacement::Side(MergeSide::Local),
                        anchor,
                        vec![local],
                    )
                }
                (None, Some(remote)) => {
                    let Some(anchor) = merge_ai_suggestion_anchor(
                        &app.geometry_cache,
                        remote_geometry,
                        &app.remote_display_rows,
                        suggestion.target,
                        MergeSide::Remote,
                    ) else {
                        continue;
                    };
                    (
                        MergeAiCardPlacement::Side(MergeSide::Remote),
                        anchor,
                        vec![remote],
                    )
                }
                (None, None) => {
                    for side in [MergeSide::Local, MergeSide::Remote] {
                        if app
                            .ai_logged_missing_anchors
                            .insert((suggestion.target, side))
                        {
                            crate::diagnostics::merge_ai_trace(
                                "ui.anchor_missing",
                                &format!("target={:?} side={side:?}", suggestion.target),
                            );
                        }
                    }
                    continue;
                }
            };
        let mut connector_anchors = connector_anchors;
        for edit in &suggestion.middle_edits {
            if let Some(edit_anchor) = merge_ai_middle_edit_anchor(
                result_geometry,
                &app.ai_middle_edit_rows,
                &edit.expected_text,
            ) {
                connector_anchors.push(edit_anchor);
            }
        }
        let width = 252.0_f32.min(anchor.width() - 12.0).max(164.0);
        let x = match placement {
            MergeAiCardPlacement::Middle => anchor.center().x - width * 0.5,
            MergeAiCardPlacement::Side(_) => {
                (anchor.left() + 64.0).min(anchor.right() - width - 8.0)
            }
        };
        let anchor_pos = Pos2::new(x, anchor.top() + 3.0);
        let offset_key = (suggestion.target, placement);
        let offset = app
            .ai_overlay_offsets
            .get(&offset_key)
            .copied()
            .unwrap_or(Vec2::ZERO);
        let area_id = egui::Id::new(("merge_ai_suggestion", placement, suggestion.target));
        let language = app.language;
        let output = egui::Area::new(area_id)
            .order(egui::Order::Foreground)
            .current_pos(anchor_pos + offset)
            .movable(true)
            .show(ctx, |ui| {
                ui.set_width(width);
                egui::Frame::new()
                    .fill(palette.panel)
                    .stroke(egui::Stroke::NONE)
                    .corner_radius(egui::CornerRadius::same(7))
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .shadow(palette.shadow)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "AI · {}",
                                    merge_ai_choice_label(language, &suggestion.choice)
                                ))
                                .small()
                                .strong()
                                .color(palette.accent),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .add(egui::Button::new(RichText::new("×").small()).frame(false))
                                    .on_hover_text(mt(language, "ai_ignore"))
                                    .clicked()
                                {
                                    action.get_or_insert(MergeAiOverlayAction::Ignore(
                                        suggestion.target,
                                    ));
                                }
                            });
                        });
                        ui.add_space(2.0);
                        for (index, paragraph) in
                            suggestion.reason(language).split("\n\n").enumerate()
                        {
                            if index > 0 {
                                ui.add_space(5.0);
                            }
                            ui.label(RichText::new(paragraph).small().color(if index == 0 {
                                palette.text
                            } else {
                                palette.muted
                            }));
                        }
                        if suggestion.is_actionable() {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!(
                                    "{} {}",
                                    mt(language, "ai_actual_changes"),
                                    suggestion.change_count()
                                ))
                                .small()
                                .strong()
                                .color(palette.accent),
                            );
                            if let Some(result) = suggestion.manual_result.as_deref() {
                                let preview = format!(
                                    "{}: {}",
                                    mt(language, "ai_target_result"),
                                    merge_ai_code_preview(result, 72)
                                );
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(preview).small().color(palette.muted),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(result);
                            }
                            for edit in &suggestion.middle_edits {
                                let preview = format!(
                                    "{}: {} → {}",
                                    mt(language, "ai_middle_edit"),
                                    merge_ai_code_preview(&edit.expected_text, 44),
                                    merge_ai_code_preview(&edit.replacement_text, 44),
                                );
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(preview).small().color(palette.muted),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(format!(
                                    "{}\n→\n{}",
                                    edit.expected_text, edit.replacement_text
                                ));
                            }
                        }
                        if suggestion.is_actionable() {
                            ui.add_space(4.0);
                            if ui
                                .small_button(mt(language, "ai_apply_suggestion"))
                                .on_hover_text(mt(language, "ai_apply_hint"))
                                .clicked()
                            {
                                action
                                    .get_or_insert(MergeAiOverlayAction::Apply(suggestion.target));
                            }
                        }
                    });
            });
        for (index, connector_anchor) in connector_anchors.into_iter().enumerate() {
            paint_merge_ai_suggestion_connector(
                ctx,
                area_id.with(index),
                connector_anchor,
                output.response.rect,
                palette,
            );
        }
        let moved = output.response.rect.min - anchor_pos;
        let allowed_offset = merge_ai_overlay_allowed_offset(
            ctx.screen_rect(),
            anchor_pos,
            output.response.rect.size(),
        );
        app.ai_overlay_offsets.insert(
            offset_key,
            Vec2::new(
                moved.x.clamp(allowed_offset.min.x, allowed_offset.max.x),
                moved.y.clamp(allowed_offset.min.y, allowed_offset.max.y),
            ),
        );
    }
    action
}

fn merge_ai_middle_anchor(
    cache: &MergeGeometryCache,
    result_geometry: &MergePanelGeometry,
    target: MergeLineActionTarget,
    local_anchor: Pos2,
    remote_anchor: Pos2,
) -> Option<Rect> {
    let rect = match target {
        MergeLineActionTarget::Conflict(index) => cache.conflicts.get(&index).and_then(|cached| {
            cached
                .result_span
                .and_then(|(first, count)| result_geometry.span_rect(first, count))
                .or_else(|| {
                    cached.result_boundary_row.and_then(|row| {
                        result_geometry.boundary_marker_rect(row, MERGE_BASE_ONLY_MARKER_HEIGHT)
                    })
                })
        }),
        MergeLineActionTarget::BaseOnlyGroup(line_index) => cache
            .base_only_groups
            .iter()
            .find(|cached| cached.group.line_index == line_index)
            .and_then(|cached| {
                result_geometry.span_rect(cached.result_row, cached.group.line_count)
            }),
    };
    rect.or_else(|| {
        let (left, right) = result_geometry.horizontal_bounds?;
        let y = (local_anchor.y + remote_anchor.y) * 0.5;
        Some(Rect::from_min_max(
            Pos2::new(left, y - MERGE_CODE_ROW_HEIGHT * 0.5),
            Pos2::new(right, y + MERGE_CODE_ROW_HEIGHT * 0.5),
        ))
    })
}

fn merge_ai_middle_edit_anchor(
    geometry: &MergePanelGeometry,
    middle_edit_rows: &HashMap<String, Option<usize>>,
    expected_text: &str,
) -> Option<Pos2> {
    let row_index = middle_edit_rows.get(expected_text).copied().flatten()?;
    geometry
        .rows
        .iter()
        .find(|(index, _)| *index == row_index)
        .map(|(_, rect)| rect.left_center())
}

fn merge_ai_pending_middle_edit_rows(
    suggestions: &HashMap<MergeLineActionTarget, MergeAiSuggestion>,
    middle_edit_rows: &HashMap<String, Option<usize>>,
) -> HashSet<usize> {
    suggestions
        .values()
        .flat_map(|suggestion| suggestion.middle_edits.iter())
        .filter_map(|edit| middle_edit_rows.get(&edit.expected_text).copied().flatten())
        .collect()
}

fn merge_ai_middle_edit_row_cache(
    suggestions: &HashMap<MergeLineActionTarget, MergeAiSuggestion>,
    lines: &[String],
) -> HashMap<String, Option<usize>> {
    let mut matches = suggestions
        .values()
        .flat_map(|suggestion| suggestion.middle_edits.iter())
        .map(|edit| edit.expected_text.as_str())
        .filter(|expected| !expected.is_empty() && !expected.contains(['\r', '\n']))
        .map(|expected| (expected.to_owned(), (0_usize, None)))
        .collect::<HashMap<_, _>>();

    for (row_index, line) in lines.iter().enumerate() {
        for (expected, (count, unique_row)) in &mut matches {
            if line.contains(expected.as_str()) {
                *count += 1;
                *unique_row = (*count == 1).then_some(row_index);
            }
        }
    }

    matches
        .into_iter()
        .map(|(expected, (count, row))| (expected, (count == 1).then_some(row).flatten()))
        .collect()
}

fn merge_ai_code_preview(value: &str, max_chars: usize) -> String {
    let flattened = value.lines().collect::<Vec<_>>().join(" ↵ ");
    let mut chars = flattened.chars();
    let preview = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
}

fn merge_ai_overlay_allowed_offset(viewport: Rect, anchor_pos: Pos2, card_size: Vec2) -> Rect {
    let min = viewport.min + Vec2::splat(MERGE_AI_OVERLAY_DRAG_MARGIN) - anchor_pos;
    let max =
        viewport.max - Vec2::splat(MERGE_AI_OVERLAY_DRAG_MARGIN) - card_size - anchor_pos.to_vec2();
    Rect::from_min_max(
        Pos2::new(min.x.min(max.x), min.y.min(max.y)),
        Pos2::new(min.x.max(max.x), min.y.max(max.y)),
    )
}

fn merge_ai_action_anchor(
    cache: &MergeGeometryCache,
    geometry: &MergePanelGeometry,
    rows: &[CachedMergeSideDisplayRow],
    target: MergeLineActionTarget,
    side: MergeSide,
) -> Option<Pos2> {
    let action_row = match target {
        MergeLineActionTarget::Conflict(_) => geometry.rows.iter().find_map(|(index, rect)| {
            rows.get(*index)
                .filter(|row| row.show_conflict_actions && row.action_target == Some(target))
                .map(|_| *rect)
        }),
        MergeLineActionTarget::BaseOnlyGroup(line_index) => {
            let cached = cache
                .base_only_groups
                .iter()
                .find(|cached| cached.group.line_index == line_index)?;
            (cached.group.missing_side == side)
                .then(|| {
                    geometry.boundary_marker_rect(
                        cached.side_boundary_row,
                        MERGE_BASE_ONLY_MARKER_HEIGHT,
                    )
                })
                .flatten()
                .map(|marker| {
                    Rect::from_center_size(
                        marker.center(),
                        Vec2::new(marker.width(), MERGE_CODE_ROW_HEIGHT),
                    )
                })
                .or_else(|| {
                    geometry.rows.iter().find_map(|(index, rect)| {
                        rows.get(*index)
                            .filter(|row| {
                                row.show_conflict_actions && row.action_target == Some(target)
                            })
                            .map(|_| *rect)
                    })
                })
        }
    }?;
    let actions = conflict_action_rects(action_row, side);
    Some(actions.drop.union(actions.take).right_center())
}

fn paint_merge_ai_suggestion_connector(
    ctx: &egui::Context,
    area_id: egui::Id,
    action_anchor: Pos2,
    card_rect: Rect,
    palette: MergePalette,
) {
    let card_anchor = closest_rect_edge_point(card_rect, action_anchor);
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        area_id.with("connector"),
    ));
    let stroke = egui::Stroke::new(1.5, color_with_opacity(palette.accent, 0.72));
    painter.line_segment([action_anchor, card_anchor], stroke);
    painter.circle_filled(action_anchor, 2.25, stroke.color);
}

fn closest_rect_edge_point(rect: Rect, point: Pos2) -> Pos2 {
    let clamped_x = point.x.clamp(rect.left(), rect.right());
    let clamped_y = point.y.clamp(rect.top(), rect.bottom());
    let edges = [
        (point.x - rect.left()).abs(),
        (point.x - rect.right()).abs(),
        (point.y - rect.top()).abs(),
        (point.y - rect.bottom()).abs(),
    ];
    let edge = edges
        .iter()
        .enumerate()
        .min_by(|left, right| left.1.total_cmp(right.1))
        .map(|(index, _)| index)
        .unwrap_or(0);
    match edge {
        0 => Pos2::new(rect.left(), clamped_y),
        1 => Pos2::new(rect.right(), clamped_y),
        2 => Pos2::new(clamped_x, rect.top()),
        _ => Pos2::new(clamped_x, rect.bottom()),
    }
}

fn merge_ai_suggestion_anchor(
    cache: &MergeGeometryCache,
    geometry: &MergePanelGeometry,
    rows: &[CachedMergeSideDisplayRow],
    target: MergeLineActionTarget,
    side: MergeSide,
) -> Option<Rect> {
    match target {
        MergeLineActionTarget::Conflict(conflict_index) => geometry
            .rows
            .iter()
            .filter_map(|(display_index, rect)| {
                rows.get(*display_index)
                    .filter(|row| row.conflict_index == Some(conflict_index))
                    .map(|_| *rect)
            })
            .reduce(|merged, rect| merged.union(rect)),
        MergeLineActionTarget::BaseOnlyGroup(line_index) => {
            let cached = cache
                .base_only_groups
                .iter()
                .find(|cached| cached.group.line_index == line_index)?;
            if side == cached.group.missing_side {
                geometry
                    .boundary_marker_rect(cached.side_boundary_row, MERGE_BASE_ONLY_MARKER_HEIGHT)
            } else {
                let first = match side {
                    MergeSide::Local => cached.local_row,
                    MergeSide::Remote => cached.remote_row,
                }?;
                geometry.span_rect(first, cached.group.line_count)
            }
        }
    }
}

fn merge_ai_choice_label(language: MergeLanguage, choice: &MergeAiChoice) -> &'static str {
    match choice {
        MergeAiChoice::Local => mt(language, "ai_choose_local"),
        MergeAiChoice::Remote => mt(language, "ai_choose_remote"),
        MergeAiChoice::Manual => mt(language, "ai_manual"),
    }
}

struct MergeSearchBarOutput {
    matches: Vec<usize>,
    current_row: Option<usize>,
    jump_row: Option<usize>,
}

fn merge_search_matches<'a>(lines: impl IntoIterator<Item = &'a str>, query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    lines
        .into_iter()
        .enumerate()
        .filter_map(|(index, line)| line.to_lowercase().contains(&query).then_some(index))
        .collect()
}

fn merge_next_search_index(current: usize, match_count: usize, direction: NavDirection) -> usize {
    if match_count == 0 {
        return 0;
    }
    match direction {
        NavDirection::Previous => current
            .checked_sub(1)
            .unwrap_or(match_count.saturating_sub(1)),
        NavDirection::Next => (current + 1) % match_count,
    }
}

fn merge_search_bar<'a>(
    ui: &mut Ui,
    search: &mut MergeSearchState,
    pane: MergeSearchPane,
    lines: impl IntoIterator<Item = &'a str>,
    language: MergeLanguage,
    palette: MergePalette,
) -> MergeSearchBarOutput {
    if !search.open || search.pane != pane {
        return MergeSearchBarOutput {
            matches: Vec::new(),
            current_row: None,
            jump_row: None,
        };
    }

    let lines = lines.into_iter().collect::<Vec<_>>();
    let mut direction = None;
    let mut close = false;
    let mut changed = false;
    ui.horizontal(|ui| {
        let id = ui.make_persistent_id(("merge_code_search", pane));
        let response = ui.add_sized(
            [ui.available_width().max(180.0) - 118.0, 24.0],
            egui::TextEdit::singleline(&mut search.query)
                .id(id)
                .hint_text(mt(language, "search_placeholder")),
        );
        if search.request_focus {
            response.request_focus();
            search.request_focus = false;
        }
        changed = response.changed();
        let preview_matches = merge_search_matches(lines.iter().copied(), &search.query);
        let preview_current = if preview_matches.is_empty() {
            0
        } else {
            search.current.min(preview_matches.len() - 1) + 1
        };
        ui.label(
            RichText::new(format!("{preview_current}/{}", preview_matches.len()))
                .small()
                .color(palette.muted),
        );
        let enter = response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        if enter {
            direction = Some(if ui.input(|input| input.modifiers.shift) {
                NavDirection::Previous
            } else {
                NavDirection::Next
            });
        }
        if ui.small_button("↑").clicked() {
            direction = Some(NavDirection::Previous);
        }
        if ui.small_button("↓").clicked() {
            direction = Some(NavDirection::Next);
        }
        if ui.small_button("×").clicked() {
            close = true;
        }
    });

    let matches = merge_search_matches(lines.iter().copied(), &search.query);
    if changed {
        search.current = 0;
    } else if let Some(direction) = direction {
        if !matches.is_empty() {
            search.current = merge_next_search_index(search.current, matches.len(), direction);
        }
    }
    if matches.is_empty() {
        search.current = 0;
    } else {
        search.current = search.current.min(matches.len() - 1);
    }
    let current_row = matches.get(search.current).copied();
    let jump_row = (changed || direction.is_some())
        .then_some(current_row)
        .flatten();
    if close {
        search.open = false;
    }
    MergeSearchBarOutput {
        matches,
        current_row,
        jump_row,
    }
}

fn paint_merge_search_match(ui: &Ui, rect: Rect, current: bool, palette: MergePalette) {
    let alpha = if current { 44 } else { 22 };
    let fill = Color32::from_rgba_unmultiplied(
        palette.accent.r(),
        palette.accent.g(),
        palette.accent.b(),
        alpha,
    );
    ui.painter().rect_filled(
        rect.intersect(ui.clip_rect()),
        egui::CornerRadius::ZERO,
        fill,
    );
}

fn merge_side_highlighted_line<'a>(
    highlights: &'a MergeSyntaxHighlights,
    side: MergeSide,
    row: &CachedMergeSideDisplayRow,
) -> Option<&'a HighlightedLine> {
    let line_index = row.line_number?.checked_sub(1)?;
    let (document, source_lines, unique_source_lines) = match side {
        MergeSide::Local => (
            highlights.local.as_ref()?,
            &highlights.local_source_lines,
            &highlights.local_unique_source_lines,
        ),
        MergeSide::Remote => (
            highlights.remote.as_ref()?,
            &highlights.remote_source_lines,
            &highlights.remote_unique_source_lines,
        ),
    };
    // Merge alignment rows can intentionally display text from a different side or from the
    // auto-merged result. Applying source byte ranges to that text colors arbitrary fragments of
    // identifiers, so fall back to the pane's base color unless the exact source line matches.
    if source_lines
        .get(line_index)
        .is_some_and(|source_line| source_line == &row.text)
    {
        return document.lines.get(line_index);
    }

    // A one-sided insertion can add alignment rows without adding source lines to the opposite
    // pane. In that case all following display line numbers are shifted. Recover only lines whose
    // exact text occurs once in that source; repeated lines remain plain instead of risking spans
    // from the wrong syntactic context.
    let source_index = unique_source_lines.get(&row.text).copied().flatten()?;
    source_lines
        .get(source_index)
        .filter(|source_line| *source_line == &row.text)?;
    document.lines.get(source_index)
}

fn merge_unique_source_line_indices(lines: &[String]) -> HashMap<String, Option<usize>> {
    let mut indices = HashMap::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        indices
            .entry(line.clone())
            .and_modify(|stored| *stored = None)
            .or_insert(Some(index));
    }
    indices
}

fn merge_side_panel(
    ui: &mut Ui,
    app: &mut MergeToolApp,
    side: MergeSide,
    scroll_id: &'static str,
    scroll_x: f32,
    code_content_width: f32,
    scroll_y: f32,
    accepts_scroll_input: bool,
    palette: MergePalette,
) -> MergeSidePanelOutput {
    let mut requested_result_scroll_y = None;
    let mut search_result_y = None;
    let mut navigation_target = None;
    let mut geometry = MergePanelGeometry::default();
    let mut pending_line_action = None;
    let path = match side {
        MergeSide::Local => app.args.local.clone(),
        MergeSide::Remote => app.args.remote.clone(),
    };
    let title = match side {
        MergeSide::Local => mt(app.language, "local"),
        MergeSide::Remote => mt(app.language, "remote"),
    };
    merge_panel_frame(ui, palette, |ui| {
        side_header(ui, title, &path, palette);
        let nav_target = side_conflict_nav(ui, app, side, palette);
        let search_pane = match side {
            MergeSide::Local => MergeSearchPane::Left,
            MergeSide::Remote => MergeSearchPane::Right,
        };
        let rows = match side {
            MergeSide::Local => &app.local_display_rows,
            MergeSide::Remote => &app.remote_display_rows,
        };
        let search_output = merge_search_bar(
            ui,
            &mut app.search,
            search_pane,
            rows.iter().map(|row| row.text.as_str()),
            app.language,
            palette,
        );
        if let Some(row) = search_output.jump_row {
            let side_scroll_y = row as f32 * MERGE_CODE_ROW_HEIGHT;
            search_result_y = Some(if app.collapse_unchanged {
                side_scroll_y
            } else {
                app.cached_result_scroll_y_for_side_scroll(side, side_scroll_y)
            });
        }
        ui.add_space(8.0);
        let visible_rows = merge_visible_tail_len(rows, app.collapse_unchanged);
        let hidden_rows = rows.len().saturating_sub(visible_rows);
        let display_rows = visible_rows + usize::from(hidden_rows > 0);
        // `show_rows` captures spacing before entering its callback and uses it to derive both
        // virtual row indices and total content height. Set dense spacing first; doing this inside
        // the callback accumulates one default gap per off-screen row and breaks large-file jumps.
        ui.spacing_mut().item_spacing.y = 0.0;
        let output = ScrollArea::vertical()
            .id_salt((scroll_id, app.display_epoch))
            .vertical_scroll_offset(scroll_y)
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
            .auto_shrink([false, false])
            .show_rows(ui, MERGE_CODE_ROW_HEIGHT, display_rows, |ui, row_range| {
                ui.set_min_width(ui.available_width());
                let cursor = match side {
                    MergeSide::Local => app.local_conflict_cursor,
                    MergeSide::Remote => app.remote_conflict_cursor,
                };
                for display_index in row_range.clone() {
                    if display_index >= visible_rows {
                        merge_collapsed_result_row(ui, hidden_rows, app.language, palette);
                        continue;
                    }
                    let row = &rows[display_index];
                    let highlighted_line =
                        merge_side_highlighted_line(&app.syntax_highlights, side, row);
                    let background = merge_side_background_run(
                        rows,
                        display_index,
                        row_range.start,
                        row_range.end.min(visible_rows),
                        cursor,
                        app.highlight_mode,
                        palette,
                    );
                    let rect = merge_code_row(
                        ui,
                        row.line_number,
                        side,
                        &row.text,
                        highlighted_line,
                        row.reference_text.as_deref(),
                        row.conflict_index,
                        row.side_resolved,
                        row.tone,
                        row.show_conflict_actions && !app.manual_result_override,
                        row.action_target,
                        background,
                        app.highlight_mode,
                        scroll_x,
                        code_content_width,
                        palette,
                        &mut pending_line_action,
                    );
                    if search_output.matches.binary_search(&display_index).is_ok() {
                        paint_merge_search_match(
                            ui,
                            rect,
                            search_output.current_row == Some(display_index),
                            palette,
                        );
                    }
                    geometry.record_row(display_index, rect);
                }
            });
        // Use the stable code viewport edge for connector x anchors. It matches the row fill edge
        // and spans the complete inter-column gap, while remaining independent of scroll_x.
        geometry.set_horizontal_bounds(output.inner_rect);
        // Missing-side markers use the same visible-row geometry as the virtual list. Keeping
        // them enabled is required for large files; off-screen groups naturally return None.
        if !app.manual_result_override {
            paint_base_only_side_overlays(
                ui,
                &app.geometry_cache,
                side,
                &geometry,
                palette,
                &mut pending_line_action,
            );
        }
        if let Some(target) = nav_target {
            navigation_target = Some(target);
            ui.ctx().request_repaint();
        } else {
            let clamped_scroll_y = merge_clamp_scroll_offset(
                output.state.offset.y,
                output.content_size.y,
                output.inner_rect.height(),
            );
            if merge_side_offset_changed_by_user(accepts_scroll_input, scroll_y, clamped_scroll_y) {
                requested_result_scroll_y = Some(if app.collapse_unchanged {
                    clamped_scroll_y
                } else {
                    app.cached_result_scroll_y_for_side_scroll(side, clamped_scroll_y)
                });
                ui.ctx().request_repaint();
            }
        }
    });
    MergeSidePanelOutput {
        requested_result_scroll_y,
        search_result_y,
        navigation_target,
        geometry,
        pending_line_action,
    }
}

fn merge_result_panel(
    ui: &mut Ui,
    app: &mut MergeToolApp,
    scroll_id: &'static str,
    scroll_x: f32,
    code_content_width: f32,
    scroll_y: f32,
    palette: MergePalette,
) -> MergeResultPanelOutput {
    let mut next_scroll_y = scroll_y;
    let mut viewport_height = 0.0;
    let mut search_result_y = None;
    let mut geometry = MergePanelGeometry::default();
    merge_panel_frame(ui, palette, |ui| {
        result_header(ui, app, palette);
        merge_result_nav_spacer(ui);
        let search_output = merge_search_bar(
            ui,
            &mut app.search,
            MergeSearchPane::Middle,
            app.manual_result_lines.iter().map(String::as_str),
            app.language,
            palette,
        );
        if let Some(row) = search_output.jump_row {
            search_result_y = Some(row as f32 * MERGE_CODE_ROW_HEIGHT);
        }
        ui.add_space(8.0);
        if app.manual_result_lines.is_empty() {
            app.manual_result_lines
                .push(mt(app.language, "result_placeholder").to_owned());
        }
        let mut changed_lines = Vec::new();
        let line_count = app.manual_result_lines.len();
        let result_row_styles = if app.manual_result_override {
            vec![(MergeSideLineTone::Unchanged, false); line_count]
        } else {
            app.result_display_rows
                .iter()
                .map(|row| {
                    let active = row.conflict_index.is_some_and(|conflict_index| {
                        (conflict_index == app.local_conflict_cursor
                            || conflict_index == app.remote_conflict_cursor)
                            && !app.document.conflict_fully_resolved(conflict_index)
                    });
                    (row.tone, active)
                })
                .collect::<Vec<_>>()
        };
        let ai_middle_edit_rows =
            merge_ai_pending_middle_edit_rows(&app.ai_suggestions, &app.ai_middle_edit_rows);
        let visible_rows = merge_visible_result_len(&result_row_styles, app.collapse_unchanged);
        let hidden_rows = line_count.saturating_sub(visible_rows);
        let display_rows = visible_rows + usize::from(hidden_rows > 0);
        // Keep this before `show_rows`; egui reads spacing while calculating virtual row ranges.
        ui.spacing_mut().item_spacing.y = 0.0;
        let output = ScrollArea::vertical()
            .id_salt((scroll_id, app.display_epoch))
            .vertical_scroll_offset(scroll_y)
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
            .auto_shrink([false, false])
            .show_rows(ui, MERGE_CODE_ROW_HEIGHT, display_rows, |ui, row_range| {
                ui.set_min_width(ui.available_width());
                for result_index in row_range.clone() {
                    if result_index >= visible_rows {
                        merge_collapsed_result_row(ui, hidden_rows, app.language, palette);
                        continue;
                    }
                    let (tone, _) = result_row_styles
                        .get(result_index)
                        .copied()
                        .unwrap_or((MergeSideLineTone::Unchanged, false));
                    let background = merge_result_background_run(
                        &result_row_styles,
                        result_index,
                        row_range.start,
                        row_range.end,
                        app.highlight_mode,
                        palette,
                    );
                    let reference_text = (!app.manual_result_override)
                        .then(|| app.result_display_rows.get(result_index))
                        .flatten()
                        .and_then(|row| row.reference_text.as_deref());
                    let highlighted_line = app
                        .syntax_highlights
                        .result
                        .as_ref()
                        .and_then(|document| document.lines.get(result_index));
                    let (rect, before_line) = merge_editable_result_row(
                        ui,
                        result_index,
                        &mut app.manual_result_lines[result_index],
                        highlighted_line,
                        tone,
                        background,
                        app.highlight_mode,
                        reference_text,
                        ai_middle_edit_rows.contains(&result_index),
                        scroll_x,
                        code_content_width,
                        palette,
                    );
                    if search_output.matches.binary_search(&result_index).is_ok() {
                        paint_merge_search_match(
                            ui,
                            rect,
                            search_output.current_row == Some(result_index),
                            palette,
                        );
                    }
                    geometry.record_row(result_index, rect);
                    if let Some(before_line) = before_line {
                        changed_lines.push((result_index, before_line));
                    }
                }
            });
        // TextEdit can be wider than the pane, but the connector starts where the visible result
        // row ends. Using the fixed inner viewport prevents both horizontal-scroll drift and the
        // panel-padding seam that appears when anchoring at the outer column shell.
        geometry.set_horizontal_bounds(output.inner_rect);
        if !changed_lines.is_empty() {
            let mut before = app.snapshot();
            for (index, line) in changed_lines {
                before.manual_result_lines[index] = line;
            }
            app.finish_manual_result_edit(before);
        }
        next_scroll_y = merge_clamp_scroll_offset(
            output.state.offset.y,
            output.content_size.y,
            output.inner_rect.height(),
        );
        viewport_height = output.inner_rect.height();
    });
    MergeResultPanelOutput {
        scroll_y: next_scroll_y,
        viewport_height,
        search_result_y,
        geometry,
    }
}

fn merge_visible_tail_len(rows: &[CachedMergeSideDisplayRow], collapse: bool) -> usize {
    if !collapse {
        return rows.len();
    }
    let last_changed = rows
        .iter()
        .rposition(|row| row.conflict_index.is_some() || row.tone != MergeSideLineTone::Unchanged)
        .map(|index| index + 1)
        .unwrap_or(0);
    let keep = (last_changed + MERGE_COLLAPSE_CONTEXT_ROWS).min(rows.len());
    (rows.len().saturating_sub(keep) >= MERGE_COLLAPSE_MIN_UNCHANGED_ROWS)
        .then_some(keep)
        .unwrap_or(rows.len())
}

fn merge_clamp_scroll_offset(offset_y: f32, content_height: f32, viewport_height: f32) -> f32 {
    offset_y.clamp(0.0, (content_height - viewport_height).max(0.0))
}

fn merge_search_scroll_target(result_row_y: f32, viewport_height: f32, content_height: f32) -> f32 {
    let row_center = result_row_y + MERGE_CODE_ROW_HEIGHT * 0.5;
    merge_clamp_scroll_offset(
        row_center - viewport_height * 0.5,
        content_height,
        viewport_height,
    )
}

fn merge_side_offset_changed_by_user(
    accepts_scroll_input: bool,
    requested_scroll_y: f32,
    actual_scroll_y: f32,
) -> bool {
    accepts_scroll_input && (actual_scroll_y - requested_scroll_y).abs() > f32::EPSILON
}

fn merge_result_content_height(app: &MergeToolApp) -> f32 {
    let display_rows = if app.manual_result_override {
        app.manual_result_lines.len().max(1)
    } else {
        let styles = app
            .result_display_rows
            .iter()
            .map(|row| {
                let active = row.conflict_index.is_some_and(|conflict_index| {
                    (conflict_index == app.local_conflict_cursor
                        || conflict_index == app.remote_conflict_cursor)
                        && !app.document.conflict_fully_resolved(conflict_index)
                });
                (row.tone, active)
            })
            .collect::<Vec<_>>();
        let visible_rows = merge_visible_result_len(&styles, app.collapse_unchanged);
        visible_rows + usize::from(visible_rows < styles.len())
    };
    display_rows.max(1) as f32 * MERGE_CODE_ROW_HEIGHT
}

fn merge_visible_result_len(styles: &[(MergeSideLineTone, bool)], collapse: bool) -> usize {
    if !collapse {
        return styles.len();
    }
    let last_changed = styles
        .iter()
        .rposition(|(tone, active)| *active || *tone != MergeSideLineTone::Unchanged)
        .map(|index| index + 1)
        .unwrap_or(0);
    let keep = (last_changed + MERGE_COLLAPSE_CONTEXT_ROWS).min(styles.len());
    (styles.len().saturating_sub(keep) >= MERGE_COLLAPSE_MIN_UNCHANGED_ROWS)
        .then_some(keep)
        .unwrap_or(styles.len())
}

fn merge_collapsed_result_row(
    ui: &mut Ui,
    hidden_rows: usize,
    language: MergeLanguage,
    palette: MergePalette,
) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), MERGE_CODE_ROW_HEIGHT),
        Sense::hover(),
    );
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::same(2),
        color_with_opacity(palette.panel_soft, 0.7),
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        format!("… {hidden_rows} {} …", mt(language, "unchanged_lines")),
        FontId::monospace(MERGE_CODE_FONT_SIZE),
        palette.muted,
    );
}

fn merge_overview_target(
    ui: &mut Ui,
    overview: Rect,
    app: &MergeToolApp,
    scroll_y: f32,
    result_viewport_height: f32,
    result_content_height: f32,
    palette: MergePalette,
) -> Option<f32> {
    // The overview must mirror displayed rows, not source rows. In collapsed mode each hidden
    // tail becomes one marker row, otherwise the map advertises a scroll range the editors do not
    // have and scrolling eventually snaps back to the first row.
    let columns = [
        merge_visible_side_overview_tones(&app.local_display_rows, app.collapse_unchanged),
        merge_visible_result_overview_tones(app),
        merge_visible_side_overview_tones(&app.remote_display_rows, app.collapse_unchanged),
    ];
    let max_rows = columns.iter().map(Vec::len).max().unwrap_or(0);
    if max_rows == 0 || overview.height() < 16.0 {
        return None;
    }
    // The map never grows taller than the actual code viewport.  A short document should get a
    // short map, while a long document keeps a map as tall as the visible code area.
    let result_rows = columns[1].len().max(1);
    let fallback_content_height = result_rows as f32 * MERGE_CODE_ROW_HEIGHT;
    let content_height = result_content_height
        .max(fallback_content_height)
        .max(MERGE_CODE_ROW_HEIGHT);
    let viewport_height = if result_viewport_height > 0.0 {
        result_viewport_height
    } else {
        overview.height()
    }
    .min(content_height)
    .max(MERGE_CODE_ROW_HEIGHT);
    // Keep the minimap track independent from the currently materialized virtual rows. Otherwise
    // clicking a different region can alter the track itself and invalidate the pointer ratio.
    let map_height = content_height.min(overview.height()).max(8.0);
    let track = Rect::from_min_size(
        Pos2::new(overview.left() + 1.0, overview.top()),
        Vec2::new((overview.width() - 2.0).max(1.0), map_height),
    );
    let viewport = merge_overview_viewport_rect(track, scroll_y, viewport_height, content_height);
    ui.painter().rect_filled(
        viewport.intersect(track),
        egui::CornerRadius::same(2),
        color_with_opacity(palette.accent, 0.54),
    );
    for (column_index, tones) in columns.iter().enumerate() {
        let total_rows = tones.len();
        let mut run_start = 0;
        while run_start < total_rows {
            let tone = tones[run_start];
            let mut run_end = run_start + 1;
            while run_end < total_rows && tones[run_end] == tone {
                run_end += 1;
            }
            let Some(color) = merge_overview_tone_color(tone, palette) else {
                run_start = run_end;
                continue;
            };
            let left = track.left() + column_index as f32 * (MERGE_OVERVIEW_COLUMN_WIDTH + 2.0);
            let projected_start =
                merge_overview_result_row(app, column_index, run_start as f32, result_rows);
            let projected_end =
                merge_overview_result_row(app, column_index, run_end as f32, result_rows);
            let top = egui::remap(
                projected_start,
                0.0..=result_rows as f32,
                track.top()..=track.bottom(),
            );
            let bottom = egui::remap(
                projected_end,
                0.0..=result_rows as f32,
                track.top()..=track.bottom(),
            );
            // A deletion maps a side span onto a zero-height result boundary. Keep it visible as
            // a one-pixel marker without changing the shared document coordinate system.
            let marker_top = top.min(track.bottom() - 1.0);
            let marker_bottom = bottom.max(marker_top + 1.0).min(track.bottom());
            ui.painter().rect_filled(
                Rect::from_min_max(
                    Pos2::new(left, marker_top),
                    Pos2::new(left + MERGE_OVERVIEW_COLUMN_WIDTH, marker_bottom),
                ),
                0.0,
                color,
            );
            run_start = run_end;
        }
    }
    let response = ui.interact(
        track.expand2(Vec2::new(3.0, 0.0)),
        ui.make_persistent_id(("merge_overview", max_rows, overview.left() as i32)),
        Sense::click_and_drag(),
    );
    if !response.clicked() && !response.dragged() {
        return None;
    }
    let pointer = response.interact_pointer_pos()?;
    Some(merge_overview_scroll_target(
        track,
        pointer.y,
        result_rows,
        viewport_height,
        content_height,
    ))
}

fn merge_overview_scroll_target(
    track: Rect,
    pointer_y: f32,
    result_rows: usize,
    viewport_height: f32,
    content_height: f32,
) -> f32 {
    let content_height = content_height.max(MERGE_CODE_ROW_HEIGHT);
    let viewport_height = viewport_height.clamp(MERGE_CODE_ROW_HEIGHT, content_height);
    let max_scroll = (content_height - viewport_height).max(0.0);
    let ratio = ((pointer_y - track.top()) / track.height()).clamp(0.0, 1.0);
    let result_row = ratio * result_rows.max(1) as f32;
    (result_row * MERGE_CODE_ROW_HEIGHT - viewport_height * 0.5).clamp(0.0, max_scroll)
}

fn merge_overview_result_row(
    app: &MergeToolApp,
    column_index: usize,
    source_row: f32,
    result_rows: usize,
) -> f32 {
    let projected = if app.collapse_unchanged {
        source_row
    } else {
        match column_index {
            0 => merge_mapped_scroll_row(
                app.cached_scroll_anchors(MergeSide::Local),
                source_row,
                false,
            ),
            2 => merge_mapped_scroll_row(
                app.cached_scroll_anchors(MergeSide::Remote),
                source_row,
                false,
            ),
            _ => source_row,
        }
    };
    projected.clamp(0.0, result_rows.max(1) as f32)
}

fn merge_overview_viewport_rect(
    track: Rect,
    scroll_y: f32,
    viewport_height: f32,
    content_height: f32,
) -> Rect {
    let content_height = content_height.max(MERGE_CODE_ROW_HEIGHT);
    let viewport_height = viewport_height.clamp(MERGE_CODE_ROW_HEIGHT, content_height);
    let viewport_ratio = (viewport_height / content_height).clamp(0.04, 1.0);
    let thumb_height = (track.height() * viewport_ratio)
        .clamp(3.0, track.height().max(3.0))
        .min(track.height());
    let max_scroll = (content_height - viewport_height).max(0.0);
    let visible_center_ratio = if max_scroll > 0.0 {
        ((scroll_y.clamp(0.0, max_scroll) + viewport_height * 0.5) / content_height).clamp(0.0, 1.0)
    } else {
        0.5
    };
    let centered_top = track.top() + track.height() * visible_center_ratio - thumb_height * 0.5;
    let top = centered_top.clamp(track.top(), track.bottom() - thumb_height);
    Rect::from_min_size(
        Pos2::new(track.left(), top),
        Vec2::new(track.width(), thumb_height),
    )
}

fn merge_result_overview_tones(app: &MergeToolApp) -> Vec<MergeSideLineTone> {
    if app.manual_result_override {
        return vec![MergeSideLineTone::Unchanged; app.manual_result_lines.len()];
    }
    app.result_display_rows.iter().map(|row| row.tone).collect()
}

fn merge_visible_side_overview_tones(
    rows: &[CachedMergeSideDisplayRow],
    collapse: bool,
) -> Vec<MergeSideLineTone> {
    let visible_rows = merge_visible_tail_len(rows, collapse);
    let mut tones = rows
        .iter()
        .take(visible_rows)
        .map(|row| row.tone)
        .collect::<Vec<_>>();
    if visible_rows < rows.len() {
        // The editor draws one collapsed-tail marker after the retained context.
        tones.push(MergeSideLineTone::Unchanged);
    }
    tones
}

fn merge_visible_result_overview_tones(app: &MergeToolApp) -> Vec<MergeSideLineTone> {
    let result_tones = merge_result_overview_tones(app);
    let visible_rows = merge_visible_result_len(
        &result_tones
            .iter()
            .copied()
            .map(|tone| (tone, false))
            .collect::<Vec<_>>(),
        app.collapse_unchanged,
    );
    let mut tones = result_tones
        .iter()
        .copied()
        .take(visible_rows)
        .collect::<Vec<_>>();
    if visible_rows < result_tones.len() {
        tones.push(MergeSideLineTone::Unchanged);
    }
    tones
}

fn merge_overview_tone_color(tone: MergeSideLineTone, palette: MergePalette) -> Option<Color32> {
    match tone {
        MergeSideLineTone::Added => Some(palette.added_text),
        MergeSideLineTone::BaseOnly => Some(palette.base_only_text),
        MergeSideLineTone::Deleted
        | MergeSideLineTone::Replaced
        | MergeSideLineTone::LocalDeletedRemoteEdited
        | MergeSideLineTone::LocalEditedRemoteDeleted => Some(palette.conflict_text),
        MergeSideLineTone::Unchanged => None,
    }
}

fn result_header(ui: &mut Ui, app: &mut MergeToolApp, palette: MergePalette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::hover());
    let title_w = 118.0;
    ui.painter().text(
        Pos2::new(rect.left() + 4.0, rect.center().y),
        Align2::LEFT_CENTER,
        mt(app.language, "result"),
        FontId::proportional(13.0),
        palette.text,
    );
    let path_rect = Rect::from_min_max(
        Pos2::new(rect.left() + title_w, rect.top()),
        rect.right_bottom(),
    );
    ui.painter().with_clip_rect(path_rect).text(
        path_rect.left_center(),
        Align2::LEFT_CENTER,
        app.args.output.display().to_string(),
        FontId::monospace(12.0),
        palette.muted,
    );
}

fn merge_result_nav_spacer(ui: &mut Ui) {
    ui.allocate_exact_size(
        Vec2::new(ui.available_width(), MERGE_NAV_BUTTON_SIZE),
        Sense::hover(),
    );
}

fn side_header(ui: &mut Ui, title: &str, path: &Path, palette: MergePalette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::hover());
    let title_w = 118.0;
    ui.painter().text(
        Pos2::new(rect.left() + 4.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(13.0),
        palette.text,
    );
    let path_rect = Rect::from_min_max(
        Pos2::new(rect.left() + title_w, rect.top()),
        rect.right_top() + Vec2::new(0.0, rect.height()),
    );
    ui.painter().with_clip_rect(path_rect).text(
        path_rect.left_center(),
        Align2::LEFT_CENTER,
        path.display().to_string(),
        FontId::monospace(12.0),
        palette.muted,
    );
}

fn side_conflict_nav(
    ui: &mut Ui,
    app: &mut MergeToolApp,
    side: MergeSide,
    palette: MergePalette,
) -> Option<MergeLineActionTarget> {
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        Vec2::new(width, MERGE_NAV_BUTTON_SIZE),
        Layout::left_to_right(Align::Center),
        |ui| {
            let targets = if app.manual_result_override {
                Vec::new()
            } else {
                merge_navigation_targets(&app.document, side)
            };
            let current_target = match side {
                MergeSide::Local => app.local_navigation_target,
                MergeSide::Remote => app.remote_navigation_target,
            };
            let conflict_cursor = match side {
                MergeSide::Local => app.local_conflict_cursor,
                MergeSide::Remote => app.remote_conflict_cursor,
            };
            let mut position = merge_navigation_position(&targets, current_target)
                .or_else(|| {
                    merge_navigation_position(
                        &targets,
                        Some(MergeLineActionTarget::Conflict(conflict_cursor)),
                    )
                })
                .unwrap_or(0);
            let enabled = !targets.is_empty();
            let mut navigation_requested = false;
            if nav_icon_button(ui, enabled, NavDirection::Previous, palette) {
                navigation_requested = true;
                position = previous_navigation_position(position, targets.len());
            }
            if nav_icon_button(ui, enabled, NavDirection::Next, palette) {
                navigation_requested = true;
                position = next_navigation_position(position, targets.len());
            }
            let target = targets.get(position).copied();
            match side {
                MergeSide::Local => app.local_navigation_target = target,
                MergeSide::Remote => app.remote_navigation_target = target,
            }
            if let Some(MergeLineActionTarget::Conflict(index)) = target {
                match side {
                    MergeSide::Local => app.local_conflict_cursor = index,
                    MergeSide::Remote => app.remote_conflict_cursor = index,
                }
            }
            let display_position = usize::from(target.is_some()) * (position + 1);
            ui.label(
                RichText::new(format!("{} / {}", display_position, targets.len()))
                    .color(palette.muted),
            );
            navigation_requested.then_some(target).flatten()
        },
    )
    .inner
}

fn nav_icon_button(
    ui: &mut Ui,
    enabled: bool,
    direction: NavDirection,
    palette: MergePalette,
) -> bool {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(MERGE_NAV_BUTTON_SIZE), sense);
    let fill = if !enabled {
        ui.visuals().widgets.noninteractive.bg_fill
    } else if response.hovered() {
        ui.visuals().widgets.hovered.bg_fill
    } else {
        ui.visuals().widgets.inactive.bg_fill
    };
    let color = if enabled {
        palette.muted
    } else {
        palette.muted.gamma_multiply(0.45)
    };

    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(3), fill);
    paint_nav_chevron(ui, rect, direction, color);
    enabled && response.clicked()
}

fn paint_nav_chevron(ui: &mut Ui, rect: Rect, direction: NavDirection, color: Color32) {
    let center = rect.center();
    let half_width = 4.5;
    let half_height = 2.8;
    let stroke = egui::Stroke::new(1.5, color);
    let (left, middle, right) = match direction {
        NavDirection::Previous => (
            Pos2::new(center.x - half_width, center.y + half_height),
            Pos2::new(center.x, center.y - half_height),
            Pos2::new(center.x + half_width, center.y + half_height),
        ),
        NavDirection::Next => (
            Pos2::new(center.x - half_width, center.y - half_height),
            Pos2::new(center.x, center.y + half_height),
            Pos2::new(center.x + half_width, center.y - half_height),
        ),
    };
    ui.painter().line_segment([left, middle], stroke);
    ui.painter().line_segment([middle, right], stroke);
}

fn merge_code_row(
    ui: &mut Ui,
    line_number: Option<usize>,
    side: MergeSide,
    text: &str,
    highlighted_line: Option<&HighlightedLine>,
    reference_text: Option<&str>,
    conflict_index: Option<usize>,
    side_resolved: bool,
    tone: MergeSideLineTone,
    show_conflict_actions: bool,
    action_target: Option<MergeLineActionTarget>,
    background: Option<(Color32, usize)>,
    highlight_mode: MergeHighlightMode,
    scroll_x: f32,
    code_content_width: f32,
    palette: MergePalette,
    pending_action: &mut Option<(MergeLineActionTarget, MergeLineAction)>,
) -> Rect {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), MERGE_CODE_ROW_HEIGHT),
        Sense::hover(),
    );
    if let Some((fill, row_count)) = background {
        let fill_rect = Rect::from_min_max(
            rect.min,
            Pos2::new(
                rect.right(),
                rect.bottom() + MERGE_CODE_ROW_HEIGHT * row_count.saturating_sub(1) as f32,
            ),
        );
        ui.painter()
            .rect_filled(fill_rect, egui::CornerRadius::ZERO, fill);
    }
    let can_show_actions = show_conflict_actions
        && action_target.is_some()
        && match action_target {
            Some(MergeLineActionTarget::Conflict(_)) => !side_resolved,
            Some(MergeLineActionTarget::BaseOnlyGroup(_)) => true,
            None => false,
        };
    if can_show_actions {
        let action_target = action_target.expect("checked action target");
        let action_rects = conflict_action_rects(rect, side);
        let drop_response = ui.put(action_rects.drop, egui::Button::new("X"));
        if drop_response.clicked() {
            *pending_action = Some((action_target, MergeLineAction::Drop));
        }
        let arrow = match side {
            MergeSide::Local => ">>",
            MergeSide::Remote => "<<",
        };
        let take_response = ui.put(action_rects.take, egui::Button::new(arrow));
        if take_response.clicked() {
            *pending_action = Some((action_target, MergeLineAction::Take));
        }
    }
    if let Some(line_number) = line_number {
        ui.painter().text(
            Pos2::new(rect.left() + 58.0, rect.center().y),
            Align2::LEFT_CENTER,
            format!("{line_number:>4}"),
            FontId::monospace(MERGE_CODE_FONT_SIZE),
            palette.muted,
        );
    }
    let (text_clip_rect, text_rect) = merge_scrolled_code_text_rects(
        rect,
        MERGE_SIDE_CODE_GUTTER_WIDTH,
        scroll_x,
        code_content_width,
    );
    let text_color = match tone {
        MergeSideLineTone::Added => palette.added_text,
        MergeSideLineTone::BaseOnly => palette.base_only_text,
        MergeSideLineTone::Deleted
        | MergeSideLineTone::Replaced
        | MergeSideLineTone::LocalDeletedRemoteEdited
        | MergeSideLineTone::LocalEditedRemoteDeleted
            if conflict_index.is_some() && !side_resolved =>
        {
            palette.conflict_text
        }
        _ => palette.text,
    };
    if highlight_mode == MergeHighlightMode::Words
        && tone != MergeSideLineTone::Unchanged
        && !text.is_empty()
    {
        paint_word_highlight_text(
            ui,
            text_rect,
            text_clip_rect,
            text,
            reference_text.unwrap_or(""),
            text_color,
            highlighted_line,
            palette,
        );
    } else {
        paint_merge_syntax_text(
            ui,
            text_rect,
            text_clip_rect,
            text,
            highlighted_line,
            text_color,
            palette,
        );
    }
    rect
}

fn paint_word_highlight_text(
    ui: &Ui,
    text_rect: Rect,
    text_clip_rect: Rect,
    text: &str,
    reference: &str,
    text_color: Color32,
    highlighted_line: Option<&HighlightedLine>,
    palette: MergePalette,
) {
    paint_word_highlight_backgrounds(ui, text_rect, text_clip_rect, text, reference, text_color);
    paint_merge_syntax_text(
        ui,
        text_rect,
        text_clip_rect,
        text,
        highlighted_line,
        text_color,
        palette,
    );
}

fn paint_merge_syntax_text(
    ui: &Ui,
    text_rect: Rect,
    text_clip_rect: Rect,
    text: &str,
    highlighted_line: Option<&HighlightedLine>,
    base_color: Color32,
    palette: MergePalette,
) {
    let painter = ui.painter().with_clip_rect(text_clip_rect);
    let galley = painter.layout_job(merge_syntax_layout_job(
        text,
        highlighted_line,
        base_color,
        palette,
        MERGE_CODE_FONT_SIZE,
    ));
    painter.galley(
        Pos2::new(
            text_rect.left(),
            text_rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        base_color,
    );
}

fn merge_syntax_layout_job(
    text: &str,
    highlighted_line: Option<&HighlightedLine>,
    base_color: Color32,
    palette: MergePalette,
    font_size: f32,
) -> LayoutJob {
    let font_id = FontId::monospace(font_size);
    let format = |color| TextFormat {
        font_id: font_id.clone(),
        color,
        ..TextFormat::default()
    };
    let mut job = LayoutJob::default();
    let mut cursor = 0usize;
    if let Some(highlighted_line) = highlighted_line {
        for span in &highlighted_line.spans {
            let start = span.start.min(text.len());
            let end = span.end.min(text.len());
            if start < cursor
                || start >= end
                || !text.is_char_boundary(start)
                || !text.is_char_boundary(end)
                || merge_syntax_span_splits_identifier(text, start, end)
            {
                continue;
            }
            if cursor < start {
                job.append(&text[cursor..start], 0.0, format(base_color));
            }
            job.append(
                &text[start..end],
                0.0,
                format(merge_syntax_color(span.role, palette)),
            );
            cursor = end;
        }
    }
    if cursor < text.len() {
        job.append(&text[cursor..], 0.0, format(base_color));
    }
    job
}

fn merge_syntax_span_splits_identifier(text: &str, start: usize, end: usize) -> bool {
    let boundary_splits_identifier = |boundary: usize| {
        if boundary == 0 || boundary >= text.len() {
            return false;
        }
        let previous = text[..boundary].chars().next_back();
        let next = text[boundary..].chars().next();
        previous.is_some_and(merge_identifier_character)
            && next.is_some_and(merge_identifier_character)
    };
    boundary_splits_identifier(start) || boundary_splits_identifier(end)
}

fn merge_identifier_character(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '$')
}

fn merge_syntax_color(role: SyntaxRole, palette: MergePalette) -> Color32 {
    let mode = if palette.bg.r() < 128 {
        crate::theme::ThemeMode::Dark
    } else {
        crate::theme::ThemeMode::Light
    };
    crate::theme::syntax_color_for_mode(role, mode)
}

fn paint_word_highlight_backgrounds(
    ui: &Ui,
    text_rect: Rect,
    text_clip_rect: Rect,
    text: &str,
    reference: &str,
    text_color: Color32,
) {
    let font = FontId::monospace(MERGE_CODE_FONT_SIZE);
    for range in merge_word_highlight_ranges(text, reference) {
        let prefix_width = merge_text_width(ui, &text[..range.start], &font, text_color);
        let token_width = merge_text_width(ui, &text[range.clone()], &font, text_color);
        let highlight = Rect::from_min_size(
            Pos2::new(text_rect.left() + prefix_width, text_rect.top() + 2.0),
            Vec2::new(token_width.max(2.0), text_rect.height() - 4.0),
        );
        ui.painter().with_clip_rect(text_clip_rect).rect_filled(
            highlight,
            egui::CornerRadius::same(2),
            color_with_opacity(text_color, 0.26),
        );
    }
}

fn merge_text_width(ui: &Ui, text: &str, font: &FontId, color: Color32) -> f32 {
    ui.fonts(|fonts| {
        fonts
            .layout_no_wrap(text.to_owned(), font.clone(), color)
            .rect
            .width()
    })
}

fn merge_code_row_fill(
    tone: MergeSideLineTone,
    unresolved_conflict: bool,
    active_conflict: bool,
    highlight_mode: MergeHighlightMode,
    palette: MergePalette,
) -> Option<Color32> {
    if unresolved_conflict {
        // One pair of take/drop controls applies to the whole side block. Painting retained lines
        // with a different BaseOnly/unchanged background made one operation look like unrelated
        // edits and could appear as if the opposite side's tone leaked into this pane.
        let fill = if active_conflict {
            palette.active_conflict_fill
        } else {
            palette.conflict_fill
        };
        return Some(merge_highlight_fill(
            MergeSideLineTone::Replaced,
            fill,
            active_conflict,
            highlight_mode,
        ));
    }
    // Word mode still needs a quiet line-level cue for every changed block. The stronger
    // token highlights alone made non-conflicting insertions read as unchanged whitespace.
    let show_word_mode_change = highlight_mode == MergeHighlightMode::Words;
    let fill = match tone {
        MergeSideLineTone::Added if show_word_mode_change => palette.added_fill,
        MergeSideLineTone::BaseOnly => palette.base_only_fill,
        MergeSideLineTone::Deleted
        | MergeSideLineTone::Replaced
        | MergeSideLineTone::LocalDeletedRemoteEdited
        | MergeSideLineTone::LocalEditedRemoteDeleted
            if show_word_mode_change =>
        {
            if active_conflict {
                palette.active_conflict_fill
            } else {
                palette.conflict_fill
            }
        }
        _ => return None,
    };

    Some(merge_highlight_fill(
        tone,
        fill,
        active_conflict,
        highlight_mode,
    ))
}

fn merge_side_row_fill(
    row: &CachedMergeSideDisplayRow,
    cursor: usize,
    highlight_mode: MergeHighlightMode,
    palette: MergePalette,
) -> Option<Color32> {
    merge_code_row_fill(
        row.tone,
        row.conflict_index.is_some() && !row.side_resolved,
        row.conflict_index == Some(cursor) && !row.side_resolved,
        highlight_mode,
        palette,
    )
}

fn merge_side_background_run(
    rows: &[CachedMergeSideDisplayRow],
    index: usize,
    visible_start: usize,
    visible_end: usize,
    cursor: usize,
    highlight_mode: MergeHighlightMode,
    palette: MergePalette,
) -> Option<(Color32, usize)> {
    let row = rows.get(index)?;
    let fill = merge_side_row_fill(row, cursor, highlight_mode, palette)?;
    let same_run = |other: &CachedMergeSideDisplayRow| {
        other.conflict_index == row.conflict_index
            && merge_side_row_fill(other, cursor, highlight_mode, palette) == Some(fill)
    };
    let is_start = index == visible_start
        || rows
            .get(index.saturating_sub(1))
            .is_none_or(|previous| !same_run(previous));
    if !is_start {
        return None;
    }

    let mut count = 1;
    while index + count < visible_end && rows.get(index + count).is_some_and(same_run) {
        count += 1;
    }
    Some((fill, count))
}

fn merge_word_highlight_ranges(text: &str, reference: &str) -> Vec<Range<usize>> {
    let text_tokens = merge_word_tokens(text);
    if text_tokens.is_empty() {
        return Vec::new();
    }
    let reference_tokens = merge_word_tokens(reference);
    if reference_tokens.is_empty() {
        return text_tokens;
    }

    const MAX_EXACT_TOKENS: usize = 128;
    if text_tokens.len() > MAX_EXACT_TOKENS || reference_tokens.len() > MAX_EXACT_TOKENS {
        return merge_word_fallback_ranges(text, reference, text_tokens);
    }

    let mut lcs = vec![vec![0_usize; reference_tokens.len() + 1]; text_tokens.len() + 1];
    for text_index in (0..text_tokens.len()).rev() {
        for reference_index in (0..reference_tokens.len()).rev() {
            lcs[text_index][reference_index] = if text[text_tokens[text_index].clone()]
                == reference[reference_tokens[reference_index].clone()]
            {
                lcs[text_index + 1][reference_index + 1] + 1
            } else {
                lcs[text_index + 1][reference_index].max(lcs[text_index][reference_index + 1])
            };
        }
    }

    let mut changed = Vec::new();
    let (mut text_index, mut reference_index) = (0, 0);
    while text_index < text_tokens.len() && reference_index < reference_tokens.len() {
        if text[text_tokens[text_index].clone()]
            == reference[reference_tokens[reference_index].clone()]
        {
            text_index += 1;
            reference_index += 1;
        } else if lcs[text_index + 1][reference_index] >= lcs[text_index][reference_index + 1] {
            changed.push(text_tokens[text_index].clone());
            text_index += 1;
        } else {
            reference_index += 1;
        }
    }
    changed.extend(text_tokens.into_iter().skip(text_index));
    changed
}

fn merge_word_tokens(text: &str) -> Vec<Range<usize>> {
    let mut tokens = Vec::new();
    let mut token_start = None;
    for (index, character) in text.char_indices() {
        if character.is_whitespace() {
            if let Some(start) = token_start.take() {
                tokens.push(start..index);
            }
        } else if token_start.is_none() {
            token_start = Some(index);
        }
    }
    if let Some(start) = token_start {
        tokens.push(start..text.len());
    }
    tokens
}

fn merge_word_fallback_ranges(
    text: &str,
    reference: &str,
    text_tokens: Vec<Range<usize>>,
) -> Vec<Range<usize>> {
    let text_chars = text.chars().collect::<Vec<_>>();
    let reference_chars = reference.chars().collect::<Vec<_>>();
    let mut prefix = 0;
    while prefix < text_chars.len()
        && prefix < reference_chars.len()
        && text_chars[prefix] == reference_chars[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < text_chars.len().saturating_sub(prefix)
        && suffix < reference_chars.len().saturating_sub(prefix)
        && text_chars[text_chars.len() - 1 - suffix]
            == reference_chars[reference_chars.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let changed_start = text_chars[..prefix]
        .iter()
        .map(|character| character.len_utf8())
        .sum::<usize>();
    let changed_end = text.len()
        - text_chars[text_chars.len().saturating_sub(suffix)..]
            .iter()
            .map(|character| character.len_utf8())
            .sum::<usize>();
    text_tokens
        .into_iter()
        .filter(|token| token.start < changed_end && token.end > changed_start)
        .collect()
}

fn paint_base_only_gap_marker_rect(ui: &Ui, marker_rect: Rect, palette: MergePalette) {
    ui.painter().rect_filled(
        marker_rect,
        egui::CornerRadius::same(1),
        color_with_opacity(palette.base_only_fill, 0.9),
    );
}

fn base_only_gap_marker_rect(row_rect: Rect) -> Rect {
    let center_y = row_rect.center().y;
    let half_height = MERGE_BASE_ONLY_MARKER_HEIGHT * 0.5;
    Rect::from_min_max(
        Pos2::new(row_rect.left() + 58.0, center_y - half_height),
        Pos2::new(row_rect.right() - 8.0, center_y + half_height),
    )
}

fn paint_base_only_side_overlays(
    ui: &mut Ui,
    cache: &MergeGeometryCache,
    side: MergeSide,
    geometry: &MergePanelGeometry,
    palette: MergePalette,
    pending_action: &mut Option<(MergeLineActionTarget, MergeLineAction)>,
) {
    for cached in cache
        .base_only_groups
        .iter()
        .filter(|cached| cached.group.missing_side == side)
    {
        let Some(marker_rect) =
            geometry.boundary_marker_rect(cached.side_boundary_row, MERGE_BASE_ONLY_MARKER_HEIGHT)
        else {
            continue;
        };
        paint_base_only_gap_marker_rect(ui, marker_rect, palette);

        let action_rect = Rect::from_center_size(
            marker_rect.center(),
            Vec2::new(marker_rect.width(), MERGE_CODE_ROW_HEIGHT),
        );
        let action_rects = conflict_action_rects(action_rect, side);
        let action_target = MergeLineActionTarget::BaseOnlyGroup(cached.group.line_index);
        let drop_response = ui.put(action_rects.drop, egui::Button::new("X"));
        if drop_response.clicked() {
            *pending_action = Some((action_target, MergeLineAction::Drop));
        }
        let arrow = match side {
            MergeSide::Local => ">>",
            MergeSide::Remote => "<<",
        };
        let take_response = ui.put(action_rects.take, egui::Button::new(arrow));
        if take_response.clicked() {
            *pending_action = Some((action_target, MergeLineAction::Take));
        }
    }
}

fn merge_side_display_rows(
    document: &MergeDocument,
    side: MergeSide,
) -> Vec<MergeSideDisplayRow<'_>> {
    let mut rows = Vec::new();
    let mut line_index = 0;
    let mut side_line_number = 1;

    while line_index < document.lines.len() {
        if let Some(conflict) = document
            .conflicts()
            .iter()
            .find(|conflict| conflict.line_indices.first().copied() == Some(line_index))
        {
            push_conflict_side_display_rows(
                &mut rows,
                conflict,
                side,
                document.conflict_side_resolved(conflict.index, side),
                &mut side_line_number,
            );
            line_index = conflict
                .line_indices
                .last()
                .map_or(line_index + 1, |last| last + 1);
            continue;
        }

        let line = &document.lines[line_index];
        let raw_missing_side = line.base_only_missing_side_raw();
        if line.base_only_resolved && raw_missing_side == Some(side) {
            line_index += 1;
            continue;
        }

        let missing_side = line.base_only_missing_side();
        if missing_side == Some(side) {
            line_index += base_only_gap_group_len(document, line_index, side).max(1);
            continue;
        }

        let base_only_display = line.is_base_only_display();
        let side_text = match side {
            MergeSide::Local => line.local.as_deref(),
            MergeSide::Remote => line.remote.as_deref(),
        };
        let text = if base_only_display {
            side_text.unwrap_or("")
        } else {
            side_text.unwrap_or(line.result.as_str())
        };
        let line_number = (side_text.is_some()
            || (line.kind != MergeLineKind::Conflict && !base_only_display))
            .then(|| {
                let number = side_line_number;
                side_line_number += 1;
                number
            });
        rows.push(MergeSideDisplayRow {
            text,
            reference_text: line.base.as_deref(),
            line_number,
            conflict_index: line.conflict_index,
            side_resolved: line.side_resolved(side),
            tone: MergeSideLineTone::Unchanged,
            show_conflict_actions: false,
            action_target: None,
        });
        line_index += 1;
    }

    rows
}

fn cached_merge_side_display_rows(
    document: &MergeDocument,
    side: MergeSide,
) -> Vec<CachedMergeSideDisplayRow> {
    merge_side_display_rows(document, side)
        .into_iter()
        .map(|row| CachedMergeSideDisplayRow {
            text: row.text.to_owned(),
            reference_text: row.reference_text.map(str::to_owned),
            line_number: row.line_number,
            conflict_index: row.conflict_index,
            side_resolved: row.side_resolved,
            tone: row.tone,
            show_conflict_actions: row.show_conflict_actions,
            action_target: row.action_target,
        })
        .collect()
}

fn cached_merge_result_display_rows(
    rows: &[MergeResultDisplayRow<'_>],
) -> Vec<CachedMergeResultDisplayRow> {
    rows.iter()
        .map(|row| CachedMergeResultDisplayRow {
            reference_text: row.reference_text.map(str::to_owned),
            conflict_index: row.conflict_index,
            tone: row.tone,
        })
        .collect()
}

fn merge_cached_conflict_spans(
    conflict_indices: impl IntoIterator<Item = Option<usize>>,
) -> HashMap<usize, (usize, usize)> {
    let mut spans = HashMap::<usize, (usize, usize)>::new();
    for (row_index, conflict_index) in conflict_indices.into_iter().enumerate() {
        let Some(conflict_index) = conflict_index else {
            continue;
        };
        spans
            .entry(conflict_index)
            .and_modify(|(_, count)| *count += 1)
            .or_insert((row_index, 1));
    }
    spans
}

fn merge_cached_result_rows_for_document_lines(
    document: &MergeDocument,
    conflict_spans: &HashMap<usize, (usize, usize)>,
) -> Vec<Option<usize>> {
    let conflict_starts = document
        .conflicts()
        .iter()
        .filter_map(|conflict| {
            conflict
                .line_indices
                .first()
                .copied()
                .map(|line_index| (line_index, conflict))
        })
        .collect::<HashMap<_, _>>();
    let mut rows = vec![None; document.lines.len()];
    let mut display_row = 0;
    let mut line_index = 0;
    while line_index < document.lines.len() {
        if let Some(conflict) = conflict_starts.get(&line_index) {
            display_row += conflict_spans
                .get(&conflict.index)
                .map_or(0, |(_, count)| *count);
            line_index = conflict
                .line_indices
                .last()
                .map_or(line_index + 1, |last| last + 1);
            continue;
        }

        rows[line_index] = Some(display_row);
        let line = &document.lines[line_index];
        display_row += if line.is_base_only_display() {
            1
        } else {
            line.result_lines().len()
        };
        line_index += 1;
    }
    rows
}

fn merge_cached_side_rows_for_document_lines(
    document: &MergeDocument,
    side: MergeSide,
    conflict_spans: &HashMap<usize, (usize, usize)>,
) -> Vec<Option<usize>> {
    let conflict_starts = document
        .conflicts()
        .iter()
        .filter_map(|conflict| {
            conflict
                .line_indices
                .first()
                .copied()
                .map(|line_index| (line_index, conflict))
        })
        .collect::<HashMap<_, _>>();
    let mut rows = vec![None; document.lines.len()];
    let mut display_row = 0;
    let mut line_index = 0;
    while line_index < document.lines.len() {
        if let Some(conflict) = conflict_starts.get(&line_index) {
            let (first, count) = conflict_spans
                .get(&conflict.index)
                .copied()
                .unwrap_or((display_row, 0));
            for conflict_line in &conflict.line_indices {
                if let Some(row) = rows.get_mut(*conflict_line) {
                    *row = Some(first);
                }
            }
            display_row += count;
            line_index = conflict
                .line_indices
                .last()
                .map_or(line_index + 1, |last| last + 1);
            continue;
        }

        let line = &document.lines[line_index];
        let raw_missing_side = line.base_only_missing_side_raw();
        if line.base_only_resolved && raw_missing_side == Some(side) {
            line_index += 1;
            continue;
        }
        if line.base_only_missing_side() == Some(side) {
            let group_len = base_only_gap_group_len(document, line_index, side).max(1);
            for group_line in line_index..(line_index + group_len).min(rows.len()) {
                rows[group_line] = Some(display_row);
            }
            line_index += group_len;
            continue;
        }

        rows[line_index] = Some(display_row);
        display_row += 1;
        line_index += 1;
    }
    rows
}

fn merge_cached_conflict_tone(
    result_rows: &[CachedMergeResultDisplayRow],
    conflict_index: usize,
) -> MergeSideLineTone {
    let mut has_base_only = false;
    let mut has_added = false;
    for row in result_rows
        .iter()
        .filter(|row| row.conflict_index == Some(conflict_index))
    {
        match row.tone {
            MergeSideLineTone::Replaced
            | MergeSideLineTone::Deleted
            | MergeSideLineTone::LocalDeletedRemoteEdited
            | MergeSideLineTone::LocalEditedRemoteDeleted => {
                return MergeSideLineTone::Replaced;
            }
            MergeSideLineTone::BaseOnly => has_base_only = true,
            MergeSideLineTone::Added => has_added = true,
            MergeSideLineTone::Unchanged => {}
        }
    }
    if has_base_only {
        MergeSideLineTone::BaseOnly
    } else if has_added {
        MergeSideLineTone::Added
    } else {
        MergeSideLineTone::Unchanged
    }
}

fn merge_geometry_cache(
    document: &MergeDocument,
    result_rows: &[CachedMergeResultDisplayRow],
    local_rows: &[CachedMergeSideDisplayRow],
    remote_rows: &[CachedMergeSideDisplayRow],
) -> MergeGeometryCache {
    let result_spans =
        merge_cached_conflict_spans(result_rows.iter().map(|row| row.conflict_index));
    let local_spans = merge_cached_conflict_spans(local_rows.iter().map(|row| row.conflict_index));
    let remote_spans =
        merge_cached_conflict_spans(remote_rows.iter().map(|row| row.conflict_index));
    let result_line_rows = merge_cached_result_rows_for_document_lines(document, &result_spans);
    let local_line_rows =
        merge_cached_side_rows_for_document_lines(document, MergeSide::Local, &local_spans);
    let remote_line_rows =
        merge_cached_side_rows_for_document_lines(document, MergeSide::Remote, &remote_spans);

    let conflicts = document
        .conflicts()
        .iter()
        .map(|conflict| {
            (
                conflict.index,
                CachedConflictGeometry {
                    result_span: result_spans.get(&conflict.index).copied(),
                    result_boundary_row: conflict.line_indices.first().copied().map(|line_index| {
                        merge_result_display_boundary_before_line(document, line_index)
                    }),
                    local_span: local_spans.get(&conflict.index).copied(),
                    remote_span: remote_spans.get(&conflict.index).copied(),
                    tone: merge_cached_conflict_tone(result_rows, conflict.index),
                },
            )
        })
        .collect();
    let base_only_groups = base_only_display_groups(document)
        .into_iter()
        .filter_map(|group| {
            let result_row = result_line_rows.get(group.line_index).copied().flatten()?;
            let local_row = local_line_rows.get(group.line_index).copied().flatten();
            let remote_row = remote_line_rows.get(group.line_index).copied().flatten();
            let side_boundary_row = match group.missing_side {
                MergeSide::Local => local_row,
                MergeSide::Remote => remote_row,
            }?;
            Some(CachedBaseOnlyGeometry {
                group,
                result_row,
                side_boundary_row,
                local_row,
                remote_row,
            })
        })
        .collect();

    MergeGeometryCache {
        conflicts,
        base_only_groups,
    }
}

fn merge_cached_scroll_anchors(
    document: &MergeDocument,
    side: MergeSide,
    result_rows: &[MergeResultDisplayRow<'_>],
    side_rows: &[CachedMergeSideDisplayRow],
) -> Vec<(f32, f32)> {
    let mut result_spans = HashMap::<usize, (usize, usize)>::new();
    for (row_index, row) in result_rows.iter().enumerate() {
        let Some(conflict_index) = row.conflict_index else {
            continue;
        };
        result_spans
            .entry(conflict_index)
            .and_modify(|(_, count)| *count += 1)
            .or_insert((row_index, 1));
    }

    let mut side_spans = HashMap::<usize, (usize, usize)>::new();
    for (row_index, row) in side_rows.iter().enumerate() {
        let Some(conflict_index) = row.conflict_index else {
            continue;
        };
        side_spans
            .entry(conflict_index)
            .and_modify(|(_, count)| *count += 1)
            .or_insert((row_index, 1));
    }

    let conflict_starts = document
        .conflicts()
        .iter()
        .filter_map(|conflict| {
            conflict
                .line_indices
                .first()
                .copied()
                .map(|line_index| (line_index, conflict))
        })
        .collect::<HashMap<_, _>>();
    let mut document_line = 0;
    let mut result_row = 0;
    let mut side_row = 0;
    let mut anchors = vec![(0.0, 0.0)];
    while document_line < document.lines.len() {
        if let Some(conflict) = conflict_starts.get(&document_line) {
            let result_count = result_spans
                .get(&conflict.index)
                .map_or(0, |(_, count)| *count);
            let side_count = side_spans
                .get(&conflict.index)
                .map_or(0, |(_, count)| *count);
            anchors.push((result_row as f32, side_row as f32));
            result_row += result_count;
            side_row += side_count;
            anchors.push((result_row as f32, side_row as f32));
            document_line = conflict
                .line_indices
                .last()
                .map_or(document_line + 1, |last| last + 1);
            continue;
        }

        let line = &document.lines[document_line];
        let result_count = if line.is_base_only_display() {
            1
        } else {
            line.result_lines().len()
        };
        let side_count = usize::from(
            !(line.base_only_resolved && line.base_only_missing_side_raw() == Some(side))
                && line.base_only_missing_side() != Some(side),
        );
        if result_count != side_count {
            anchors.push((result_row as f32, side_row as f32));
        }
        result_row += result_count;
        side_row += side_count;
        if result_count != side_count {
            anchors.push((result_row as f32, side_row as f32));
        }
        document_line += 1;
    }

    anchors.push((result_rows.len() as f32, side_rows.len() as f32));
    anchors.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then_with(|| left.1.total_cmp(&right.1))
    });
    anchors.dedup();
    anchors
}

fn base_only_gap_group_len(document: &MergeDocument, line_index: usize, side: MergeSide) -> usize {
    document.lines[line_index..]
        .iter()
        .take_while(|line| line.base_only_missing_side() == Some(side))
        .count()
}

fn merge_side_display_row_visual_height(row: &MergeSideDisplayRow<'_>) -> usize {
    let _ = row;
    1
}

fn push_conflict_side_display_rows<'a>(
    rows: &mut Vec<MergeSideDisplayRow<'a>>,
    conflict: &'a ConflictBlock,
    side: MergeSide,
    side_resolved: bool,
    side_line_number: &mut usize,
) {
    let side_lines = match side {
        MergeSide::Local => conflict.local.as_slice(),
        MergeSide::Remote => conflict.remote.as_slice(),
    };
    let compare_sides = conflict_prefers_side_comparison(conflict);
    let diff_reference_lines = if compare_sides {
        match side {
            MergeSide::Local => conflict.remote.as_slice(),
            MergeSide::Remote => conflict.local.as_slice(),
        }
    } else {
        conflict.base.as_slice()
    };
    // Word-level emphasis describes each side's change from the common base, not the
    // opposing side's unrelated wording. A pure insertion has no base text at all.
    let word_reference_lines = conflict.base.as_slice();
    let diff_rows = if compare_sides {
        normalize_side_comparison_diff_rows(
            merge_diff_base_to_side(diff_reference_lines, side_lines),
            conflict,
        )
    } else {
        merge_diff_base_to_side(&conflict.base, side_lines)
    };
    let mut show_conflict_actions = true;

    for (row_index, diff_row) in diff_rows.into_iter().enumerate() {
        let (text, line_number, tone) = match diff_row {
            MergeSideDiffRow::Equal(text) => {
                let number = *side_line_number;
                *side_line_number += 1;
                (text, Some(number), MergeSideLineTone::Unchanged)
            }
            MergeSideDiffRow::Deleted(text) => (
                "",
                None,
                side_diff_tone_for_missing_reference(conflict, compare_sides, text),
            ),
            MergeSideDiffRow::Added(text) => {
                let number = *side_line_number;
                *side_line_number += 1;
                (
                    text,
                    Some(number),
                    side_diff_tone_for_side_text(conflict, compare_sides, text),
                )
            }
            MergeSideDiffRow::Replaced(text) => {
                let number = *side_line_number;
                *side_line_number += 1;
                (text, Some(number), MergeSideLineTone::Replaced)
            }
        };
        rows.push(MergeSideDisplayRow {
            text,
            reference_text: (!conflict.base.is_empty())
                .then(|| word_reference_lines.get(row_index).map(String::as_str))
                .flatten(),
            line_number,
            conflict_index: Some(conflict.index),
            side_resolved,
            // Keep the alignment rows after a decision so all three panes remain vertically
            // synchronized, but stop painting them as pending changes. Otherwise deleted-line
            // placeholders survive as opaque gray blocks after an AI suggestion is applied.
            tone: if side_resolved {
                MergeSideLineTone::Unchanged
            } else {
                tone
            },
            show_conflict_actions,
            action_target: show_conflict_actions
                .then_some(MergeLineActionTarget::Conflict(conflict.index)),
        });
        show_conflict_actions = false;
    }
}

fn conflict_prefers_side_comparison(conflict: &ConflictBlock) -> bool {
    conflict.local != conflict.base
        && conflict.remote != conflict.base
        && conflict.local != conflict.remote
}

fn normalize_side_comparison_diff_rows<'a>(
    rows: Vec<MergeSideDiffRow<'a>>,
    conflict: &ConflictBlock,
) -> Vec<MergeSideDiffRow<'a>> {
    rows.into_iter()
        .filter_map(|row| match row {
            MergeSideDiffRow::Added(text) if !merge_base_contains_line(conflict, text) => {
                Some(MergeSideDiffRow::Replaced(text))
            }
            MergeSideDiffRow::Deleted(text) if !merge_base_contains_line(conflict, text) => None,
            other => Some(other),
        })
        .collect()
}

fn merge_base_contains_line(conflict: &ConflictBlock, text: &str) -> bool {
    conflict.base.iter().any(|base| base == text)
}

fn side_diff_tone_for_missing_reference(
    conflict: &ConflictBlock,
    compare_sides: bool,
    text: &str,
) -> MergeSideLineTone {
    if compare_sides && merge_base_contains_line(conflict, text) {
        MergeSideLineTone::BaseOnly
    } else {
        MergeSideLineTone::Deleted
    }
}

fn side_diff_tone_for_side_text(
    conflict: &ConflictBlock,
    compare_sides: bool,
    text: &str,
) -> MergeSideLineTone {
    if compare_sides && merge_base_contains_line(conflict, text) {
        MergeSideLineTone::BaseOnly
    } else {
        MergeSideLineTone::Added
    }
}

fn merge_diff_base_to_side<'a>(
    base: &'a [String],
    side: &'a [String],
) -> Vec<MergeSideDiffRow<'a>> {
    let mut lcs = vec![vec![0; side.len() + 1]; base.len() + 1];
    for base_index in (0..base.len()).rev() {
        for side_index in (0..side.len()).rev() {
            lcs[base_index][side_index] = if base[base_index] == side[side_index] {
                lcs[base_index + 1][side_index + 1] + 1
            } else {
                lcs[base_index + 1][side_index].max(lcs[base_index][side_index + 1])
            };
        }
    }

    let mut rows = Vec::new();
    let mut base_index = 0;
    let mut side_index = 0;
    while base_index < base.len() && side_index < side.len() {
        if base[base_index] == side[side_index] {
            rows.push(MergeSideDiffRow::Equal(side[side_index].as_str()));
            base_index += 1;
            side_index += 1;
        } else if lcs[base_index + 1][side_index] >= lcs[base_index][side_index + 1] {
            rows.push(MergeSideDiffRow::Deleted(base[base_index].as_str()));
            base_index += 1;
        } else {
            rows.push(MergeSideDiffRow::Added(side[side_index].as_str()));
            side_index += 1;
        }
    }
    while base_index < base.len() {
        rows.push(MergeSideDiffRow::Deleted(base[base_index].as_str()));
        base_index += 1;
    }
    while side_index < side.len() {
        rows.push(MergeSideDiffRow::Added(side[side_index].as_str()));
        side_index += 1;
    }

    collapse_replacement_rows(rows)
}

fn collapse_replacement_rows<'a>(rows: Vec<MergeSideDiffRow<'a>>) -> Vec<MergeSideDiffRow<'a>> {
    let mut collapsed = Vec::new();
    let mut index = 0;

    while index < rows.len() {
        if !matches!(rows[index], MergeSideDiffRow::Deleted(_)) {
            collapsed.push(rows[index]);
            index += 1;
            continue;
        }

        let delete_start = index;
        while index < rows.len() && matches!(rows[index], MergeSideDiffRow::Deleted(_)) {
            index += 1;
        }
        let add_start = index;
        while index < rows.len() && matches!(rows[index], MergeSideDiffRow::Added(_)) {
            index += 1;
        }

        if add_start == index {
            collapsed.extend_from_slice(&rows[delete_start..add_start]);
            continue;
        }

        let deleted = &rows[delete_start..add_start];
        let added = &rows[add_start..index];
        let replace_count = deleted.len().min(added.len());
        for row in added.iter().take(replace_count) {
            if let MergeSideDiffRow::Added(text) = row {
                collapsed.push(MergeSideDiffRow::Replaced(text));
            }
        }
        if deleted.len() > replace_count {
            collapsed.extend_from_slice(&deleted[replace_count..]);
        }
        if added.len() > replace_count {
            collapsed.extend_from_slice(&added[replace_count..]);
        }
    }

    collapsed
}

fn merge_editable_result_row(
    ui: &mut Ui,
    index: usize,
    text: &mut String,
    highlighted_line: Option<&HighlightedLine>,
    tone: MergeSideLineTone,
    background: Option<(Color32, usize)>,
    highlight_mode: MergeHighlightMode,
    reference_text: Option<&str>,
    ai_suggested_edit: bool,
    scroll_x: f32,
    code_content_width: f32,
    palette: MergePalette,
) -> (Rect, Option<String>) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), MERGE_CODE_ROW_HEIGHT),
        Sense::hover(),
    );
    if let Some((fill, row_count)) = background {
        // Paint one solid span before any row text. Per-row fills can leave a one-pixel
        // antialiasing seam between adjacent conflict rows on fractional display scales.
        let fill_rect = Rect::from_min_max(
            rect.min,
            Pos2::new(
                rect.right(),
                rect.bottom() + MERGE_CODE_ROW_HEIGHT * row_count.saturating_sub(1) as f32,
            ),
        );
        ui.painter()
            .rect_filled(fill_rect, egui::CornerRadius::ZERO, fill);
    }
    if ai_suggested_edit {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::ZERO,
            color_with_opacity(palette.accent, 0.14),
        );
        ui.painter().rect_filled(
            Rect::from_min_max(rect.min, Pos2::new(rect.left() + 3.0, rect.bottom())),
            egui::CornerRadius::ZERO,
            palette.accent,
        );
    }
    paint_result_side_status_badges(ui, rect, tone, palette);
    ui.painter().text(
        Pos2::new(rect.left() + 32.0, rect.center().y),
        Align2::LEFT_CENTER,
        format!("{:>4}", index + 1),
        FontId::monospace(MERGE_CODE_FONT_SIZE),
        palette.muted,
    );
    let (text_clip_rect, text_rect) = merge_scrolled_code_text_rects(
        rect,
        MERGE_RESULT_CODE_GUTTER_WIDTH,
        scroll_x,
        code_content_width,
    );
    let text_color = match tone {
        MergeSideLineTone::Added => palette.added_text,
        MergeSideLineTone::BaseOnly => palette.base_only_text,
        MergeSideLineTone::Deleted
        | MergeSideLineTone::Replaced
        | MergeSideLineTone::LocalDeletedRemoteEdited
        | MergeSideLineTone::LocalEditedRemoteDeleted => palette.conflict_text,
        MergeSideLineTone::Unchanged => palette.text,
    };
    if highlight_mode == MergeHighlightMode::Words
        && tone != MergeSideLineTone::Unchanged
        && !text.is_empty()
    {
        paint_word_highlight_backgrounds(
            ui,
            text_rect,
            text_clip_rect,
            text,
            reference_text.unwrap_or(""),
            text_color,
        );
    }
    let before = text.clone();
    let mut layouter = |ui: &Ui, value: &str, wrap_width: f32| {
        let mut job = merge_syntax_layout_job(
            value,
            highlighted_line,
            text_color,
            palette,
            MERGE_CODE_FONT_SIZE,
        );
        job.wrap.max_width = wrap_width;
        ui.fonts(|fonts| fonts.layout_job(job))
    };
    // The editor is intentionally as wide as the longest line so all panes share one horizontal
    // offset. Put it in a non-allocating child Ui: adding the wide TextEdit directly to the row Ui
    // expands that Ui, causing every following row and the panel frame to cross pane boundaries.
    let mut editor_ui = ui.new_child(egui::UiBuilder::new().max_rect(text_rect));
    editor_ui.shrink_clip_rect(text_clip_rect);
    let changed = editor_ui
        .put(
            text_rect,
            egui::TextEdit::singleline(text)
                .id_salt(("merge_result_line", index))
                .frame(false)
                // Match the word-highlight coordinate system. TextEdit otherwise inserts 4px.
                .margin(egui::Margin::ZERO)
                .font(FontId::monospace(MERGE_CODE_FONT_SIZE))
                .text_color(text_color)
                .layouter(&mut layouter)
                .desired_width(text_rect.width()),
        )
        .changed()
        .then_some(before);
    (rect, changed)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MergeResultSideStatus {
    Deleted,
    Edited,
}

fn paint_result_side_status_badges(
    ui: &Ui,
    rect: Rect,
    tone: MergeSideLineTone,
    palette: MergePalette,
) {
    let Some((local_status, remote_status)) = result_side_status_pair(tone) else {
        return;
    };
    for (offset, status) in [(2.0, local_status), (17.0, remote_status)] {
        let badge = Rect::from_min_size(
            Pos2::new(rect.left() + offset, rect.top() + 2.0),
            Vec2::new(13.0, rect.height() - 4.0),
        );
        let (fill, foreground, symbol) = match status {
            MergeResultSideStatus::Deleted => (palette.base_only_fill, palette.base_only_text, "−"),
            MergeResultSideStatus::Edited => (palette.conflict_fill, palette.conflict_text, "~"),
        };
        // Draw a one-pixel semantic outline so the edit badge is still visible on a conflict row.
        ui.painter()
            .rect_filled(badge, egui::CornerRadius::same(3), foreground);
        ui.painter()
            .rect_filled(badge.shrink(1.0), egui::CornerRadius::same(2), fill);
        ui.painter().text(
            badge.center(),
            Align2::CENTER_CENTER,
            symbol,
            FontId::monospace(10.0),
            foreground,
        );
    }
}

fn result_side_status_pair(
    tone: MergeSideLineTone,
) -> Option<(MergeResultSideStatus, MergeResultSideStatus)> {
    match tone {
        MergeSideLineTone::LocalDeletedRemoteEdited => Some((
            MergeResultSideStatus::Deleted,
            MergeResultSideStatus::Edited,
        )),
        MergeSideLineTone::LocalEditedRemoteDeleted => Some((
            MergeResultSideStatus::Edited,
            MergeResultSideStatus::Deleted,
        )),
        _ => None,
    }
}

fn merge_result_row_fill(
    tone: MergeSideLineTone,
    active_conflict: bool,
    palette: MergePalette,
) -> Color32 {
    match tone {
        MergeSideLineTone::Added => palette.added_fill,
        MergeSideLineTone::BaseOnly => palette.base_only_fill,
        MergeSideLineTone::Deleted
        | MergeSideLineTone::Replaced
        | MergeSideLineTone::LocalDeletedRemoteEdited
        | MergeSideLineTone::LocalEditedRemoteDeleted => {
            if active_conflict {
                palette.active_conflict_fill
            } else {
                palette.conflict_fill
            }
        }
        MergeSideLineTone::Unchanged => palette.result_fill,
    }
}

fn merge_highlight_fill(
    tone: MergeSideLineTone,
    fill: Color32,
    active_conflict: bool,
    highlight_mode: MergeHighlightMode,
) -> Color32 {
    match highlight_mode {
        MergeHighlightMode::Lines | MergeHighlightMode::Words
            if tone == MergeSideLineTone::BaseOnly =>
        {
            // Base-only represents a deletion decision. It stays a full grey block in word
            // mode so the actionable deletion does not fade into surrounding context.
            fill
        }
        MergeHighlightMode::Lines => fill,
        MergeHighlightMode::Words => {
            let opacity = if active_conflict {
                MERGE_WORD_ACTIVE_BLOCK_OPACITY
            } else {
                MERGE_WORD_BLOCK_OPACITY
            };
            if opacity >= 1.0 {
                fill
            } else {
                color_with_opacity(fill, opacity)
            }
        }
    }
}

fn merge_result_background_run(
    styles: &[(MergeSideLineTone, bool)],
    index: usize,
    visible_start: usize,
    visible_end: usize,
    highlight_mode: MergeHighlightMode,
    palette: MergePalette,
) -> Option<(Color32, usize)> {
    let &(tone, active) = styles.get(index)?;
    let fill = merge_result_row_fill(tone, active, palette);
    let fill = merge_highlight_fill(tone, fill, active, highlight_mode);
    let is_start = index == visible_start
        || styles
            .get(index.saturating_sub(1))
            .is_none_or(|&(previous_tone, previous_active)| {
                merge_highlight_fill(
                    previous_tone,
                    merge_result_row_fill(previous_tone, previous_active, palette),
                    previous_active,
                    highlight_mode,
                ) != fill
            });
    if !is_start {
        return None;
    }

    let mut count = 1;
    while index + count < visible_end {
        let (next_tone, next_active) = styles[index + count];
        let next_fill = merge_highlight_fill(
            next_tone,
            merge_result_row_fill(next_tone, next_active, palette),
            next_active,
            highlight_mode,
        );
        if next_fill != fill {
            break;
        }
        count += 1;
    }
    Some((fill, count))
}

#[cfg(test)]
fn merge_result_display_lines(document: &MergeDocument) -> Vec<&str> {
    merge_result_display_rows(document)
        .into_iter()
        .map(|row| row.text)
        .collect()
}

fn merge_result_display_rows(document: &MergeDocument) -> Vec<MergeResultDisplayRow<'_>> {
    let mut rows = Vec::new();
    let mut line_index = 0;

    while line_index < document.lines.len() {
        if let Some(conflict) = document
            .conflicts()
            .iter()
            .find(|conflict| conflict.line_indices.first().copied() == Some(line_index))
        {
            push_conflict_result_display_rows(&mut rows, document, conflict);
            line_index = conflict
                .line_indices
                .last()
                .map_or(line_index + 1, |last| last + 1);
            continue;
        }

        let line = &document.lines[line_index];
        if line.is_base_only_display() {
            rows.push(MergeResultDisplayRow {
                text: line.result.as_str(),
                reference_text: line.base.as_deref(),
                conflict_index: None,
                tone: MergeSideLineTone::BaseOnly,
            });
        } else {
            for text in line.result_lines() {
                rows.push(MergeResultDisplayRow {
                    text,
                    reference_text: line.base.as_deref(),
                    conflict_index: None,
                    tone: MergeSideLineTone::Unchanged,
                });
            }
        }
        line_index += 1;
    }

    rows
}

fn push_conflict_result_display_rows<'a>(
    rows: &mut Vec<MergeResultDisplayRow<'a>>,
    document: &'a MergeDocument,
    conflict: &'a ConflictBlock,
) {
    let selected = conflict
        .line_indices
        .iter()
        .filter_map(|line_index| document.lines.get(*line_index))
        .flat_map(MergeLine::result_lines)
        .collect::<Vec<_>>();
    if !selected.is_empty() {
        for (index, text) in selected.into_iter().enumerate() {
            rows.push(MergeResultDisplayRow {
                text,
                reference_text: conflict.base.get(index).map(String::as_str),
                conflict_index: Some(conflict.index),
                tone: MergeSideLineTone::Unchanged,
            });
        }
        return;
    }

    if document.conflict_fully_resolved(conflict.index) {
        return;
    }

    if conflict.base.is_empty() {
        // An insertion conflict lives on the boundary between two result rows. Keeping a
        // blank display row here shifts everything below it out of alignment, so the painter
        // draws a narrow marker at that boundary from the real row geometry instead.
        return;
    }

    let tones = merge_base_result_tones(conflict);
    for (index, (text, tone)) in conflict.base.iter().zip(tones).enumerate() {
        rows.push(MergeResultDisplayRow {
            text,
            reference_text: conflict
                .local
                .get(index)
                .or_else(|| conflict.remote.get(index))
                .map(String::as_str),
            conflict_index: Some(conflict.index),
            tone,
        });
    }
}

fn merge_base_result_tones(conflict: &ConflictBlock) -> Vec<MergeSideLineTone> {
    let local_states = merge_base_line_states(&conflict.base, &conflict.local);
    let remote_states = merge_base_line_states(&conflict.base, &conflict.remote);
    let has_local_only_base =
        local_states
            .iter()
            .zip(remote_states.iter())
            .any(|(local, remote)| {
                *local == MergeBaseLineState::Kept && *remote == MergeBaseLineState::Deleted
            });
    let has_remote_only_base =
        local_states
            .iter()
            .zip(remote_states.iter())
            .any(|(local, remote)| {
                *local == MergeBaseLineState::Deleted && *remote == MergeBaseLineState::Kept
            });
    let opposing_base_deletions = has_local_only_base && has_remote_only_base;
    // A delete-vs-edit run is one overlapping conflict even if only its first line changed on the
    // edited side. Keeping the unchanged body gray splits a changed `if` from its retained `return`
    // and closing brace. Propagate replacement tone forward through that continuous run, but do not
    // recolor independent one-sided deletions that appear before a later replacement in the same
    // broad conflict block.
    let mut replacement_run_tone = None;
    local_states
        .iter()
        .zip(remote_states.iter())
        .map(|(local, remote)| match (*local, *remote) {
            (MergeBaseLineState::Kept, MergeBaseLineState::Kept) => {
                replacement_run_tone = None;
                MergeSideLineTone::Unchanged
            }
            (MergeBaseLineState::Deleted, MergeBaseLineState::Replaced) => {
                replacement_run_tone = Some(MergeSideLineTone::LocalDeletedRemoteEdited);
                MergeSideLineTone::LocalDeletedRemoteEdited
            }
            (MergeBaseLineState::Replaced, MergeBaseLineState::Deleted) => {
                replacement_run_tone = Some(MergeSideLineTone::LocalEditedRemoteDeleted);
                MergeSideLineTone::LocalEditedRemoteDeleted
            }
            (MergeBaseLineState::Kept, MergeBaseLineState::Deleted)
            | (MergeBaseLineState::Deleted, MergeBaseLineState::Kept)
                if replacement_run_tone.is_some() =>
            {
                replacement_run_tone.expect("replacement run tone exists")
            }
            (MergeBaseLineState::Kept, MergeBaseLineState::Deleted)
            | (MergeBaseLineState::Deleted, MergeBaseLineState::Kept)
                if opposing_base_deletions =>
            {
                MergeSideLineTone::Replaced
            }
            (MergeBaseLineState::Kept, MergeBaseLineState::Deleted)
            | (MergeBaseLineState::Deleted, MergeBaseLineState::Kept) => {
                MergeSideLineTone::BaseOnly
            }
            _ => {
                replacement_run_tone = Some(MergeSideLineTone::Replaced);
                MergeSideLineTone::Replaced
            }
        })
        .collect()
}

fn merge_base_line_states(base: &[String], side: &[String]) -> Vec<MergeBaseLineState> {
    let mut states = vec![MergeBaseLineState::Deleted; base.len()];
    // Plain LCS is ambiguous for source code because braces, blank lines and statements such as
    // `return "review";` repeat frequently. It can match a later identical statement to an
    // earlier base line across a changed block, making a real replacement look unchanged in the
    // result preview. Patience diff anchors unique surrounding lines first and therefore keeps
    // equal rows attached to their local edit block.
    for operation in capture_diff_slices(Algorithm::Patience, base, side) {
        let (tag, base_range, side_range) = operation.as_tag_tuple();
        match tag {
            DiffTag::Equal => states[base_range].fill(MergeBaseLineState::Kept),
            DiffTag::Replace => {
                mark_replaced_base_lines(&mut states, base, side, base_range, side_range)
            }
            DiffTag::Delete | DiffTag::Insert => {}
        }
    }
    states
}

fn mark_replaced_base_lines(
    states: &mut [MergeBaseLineState],
    base: &[String],
    side: &[String],
    base_range: Range<usize>,
    side_range: Range<usize>,
) {
    if base_range.is_empty() || side_range.is_empty() {
        return;
    }
    if side_range.len() >= base_range.len() {
        states[base_range].fill(MergeBaseLineState::Replaced);
        return;
    }
    if base_range.len().saturating_mul(side_range.len()) > 4_096 {
        states[base_range].fill(MergeBaseLineState::Replaced);
        return;
    }

    let mut candidates = base_range
        .clone()
        .flat_map(|base_index| {
            side_range.clone().map(move |side_index| {
                (
                    merge_line_similarity(&base[base_index], &side[side_index]),
                    base_index,
                    side_index,
                )
            })
        })
        .filter(|(similarity, _, _)| *similarity >= 0.35)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.0.total_cmp(&left.0));

    let mut paired_base = HashSet::new();
    let mut paired_side = HashSet::new();
    for (_, base_index, side_index) in candidates {
        if paired_base.contains(&base_index) || paired_side.contains(&side_index) {
            continue;
        }
        paired_base.insert(base_index);
        paired_side.insert(side_index);
        states[base_index] = MergeBaseLineState::Replaced;
    }

    // If none of the lines has a meaningful textual relationship, the replace hunk is still an
    // overlapping edit. Treating it as a pure deletion would recreate the misleading gray state.
    if paired_side.is_empty() {
        states[base_range].fill(MergeBaseLineState::Replaced);
    }
}

fn merge_line_similarity(left: &str, right: &str) -> f32 {
    let left = left.trim().chars().take(256).collect::<Vec<_>>();
    let right = right.trim().chars().take(256).collect::<Vec<_>>();
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let mut previous = vec![0_usize; right.len() + 1];
    for left_char in &left {
        let mut current = vec![0_usize; right.len() + 1];
        for (right_index, right_char) in right.iter().enumerate() {
            current[right_index + 1] = if left_char == right_char {
                previous[right_index] + 1
            } else {
                current[right_index].max(previous[right_index + 1])
            };
        }
        previous = current;
    }
    previous[right.len()] as f32 / left.len().max(right.len()) as f32
}

fn paint_merge_block_connectors(
    ui: &Ui,
    document: &MergeDocument,
    cache: &MergeGeometryCache,
    local_geometry: &MergePanelGeometry,
    result_geometry: &MergePanelGeometry,
    remote_geometry: &MergePanelGeometry,
    columns: MergeConnectorColumns,
    local_conflict_cursor: usize,
    remote_conflict_cursor: usize,
    debug: MergeConnectorDebug,
    palette: MergePalette,
) {
    for conflict in document.conflicts() {
        let cached = cache
            .conflicts
            .get(&conflict.index)
            .copied()
            .unwrap_or_default();
        let tone = if conflict.base.is_empty() {
            MergeSideLineTone::Replaced
        } else {
            cached.tone
        };
        let result_rect = if conflict.base.is_empty() {
            cached.result_boundary_row.and_then(|row| {
                result_geometry.boundary_marker_rect(row, MERGE_BASE_ONLY_MARKER_HEIGHT)
            })
        } else {
            cached
                .result_span
                .and_then(|(first, count)| result_geometry.span_rect(first, count))
        };
        let Some(result_rect) = result_rect else {
            continue;
        };
        let unresolved = !document.conflict_fully_resolved(conflict.index);
        let result_fill = merge_result_row_fill(
            tone,
            unresolved
                && (conflict.index == local_conflict_cursor
                    || conflict.index == remote_conflict_cursor),
            palette,
        );
        if conflict.base.is_empty() {
            // Zero-width conflicts do not own a result row. Paint the marker directly on the
            // boundary so the line remains visible without shifting later rows downward.
            ui.painter().rect_filled(
                result_rect,
                egui::CornerRadius::ZERO,
                merge_connector_fill(tone, palette),
            );
        }
        paint_result_block_outline(ui, result_rect, tone, palette);

        if let Some(local_rect) = cached
            .local_span
            .and_then(|(first, count)| local_geometry.span_rect(first, count))
        {
            paint_side_block_bridge(
                ui,
                result_rect,
                local_rect,
                columns.result,
                columns.local,
                MergeSide::Local,
                tone,
                result_fill,
                if unresolved && conflict.index == local_conflict_cursor {
                    palette.active_conflict_fill
                } else {
                    palette.conflict_fill
                },
                palette,
            );
            paint_side_block_debug(
                ui,
                debug,
                "conflict",
                conflict.index,
                MergeSide::Local,
                result_rect,
                local_rect,
                columns.result,
                columns.local,
                tone,
            );
        }
        if let Some(remote_rect) = cached
            .remote_span
            .and_then(|(first, count)| remote_geometry.span_rect(first, count))
        {
            paint_side_block_bridge(
                ui,
                result_rect,
                remote_rect,
                columns.result,
                columns.remote,
                MergeSide::Remote,
                tone,
                result_fill,
                if unresolved && conflict.index == remote_conflict_cursor {
                    palette.active_conflict_fill
                } else {
                    palette.conflict_fill
                },
                palette,
            );
            paint_side_block_debug(
                ui,
                debug,
                "conflict",
                conflict.index,
                MergeSide::Remote,
                result_rect,
                remote_rect,
                columns.result,
                columns.remote,
                tone,
            );
        }
    }

    for cached in &cache.base_only_groups {
        let group = cached.group;
        let Some(result_rect) = result_geometry.span_rect(cached.result_row, group.line_count)
        else {
            continue;
        };
        let tone = MergeSideLineTone::BaseOnly;
        paint_result_block_outline(ui, result_rect, tone, palette);
        let side_geometry = match group.missing_side {
            MergeSide::Local => local_geometry,
            MergeSide::Remote => remote_geometry,
        };
        if let Some(side_rect) = side_geometry
            .boundary_marker_rect(cached.side_boundary_row, MERGE_BASE_ONLY_MARKER_HEIGHT)
        {
            let side_column = match group.missing_side {
                MergeSide::Local => columns.local,
                MergeSide::Remote => columns.remote,
            };
            paint_base_only_marker_bridge(
                ui,
                result_rect,
                side_rect,
                columns.result,
                side_column,
                group.missing_side,
                palette,
            );
            paint_side_block_debug(
                ui,
                debug,
                "base-only",
                group.line_index,
                group.missing_side,
                result_rect,
                side_rect,
                columns.result,
                side_column,
                tone,
            );
        }
    }
}

fn merge_connector_debug_mode() -> MergeConnectorDebug {
    if merge_build_config_bool("connector_log") {
        MergeConnectorDebug::Log
    } else if merge_build_config_bool("connector_guides") {
        MergeConnectorDebug::Guides
    } else {
        MergeConnectorDebug::Off
    }
}

fn merge_build_config_bool(key: &str) -> bool {
    MERGE_BUILD_CONFIG
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name.trim() == key).then_some(value.trim())
        })
        .is_some_and(|value| matches!(value, "true" | "1" | "yes" | "on"))
}

fn base_only_display_groups(document: &MergeDocument) -> Vec<BaseOnlyDisplayGroup> {
    let mut groups = Vec::new();
    let mut line_index = 0;
    while line_index < document.lines.len() {
        let Some(missing_side) = document.lines[line_index].base_only_missing_side() else {
            line_index += 1;
            continue;
        };
        let line_count = base_only_gap_group_len(document, line_index, missing_side).max(1);
        groups.push(BaseOnlyDisplayGroup {
            line_index,
            line_count,
            missing_side,
        });
        line_index += line_count;
    }
    groups
}

fn merge_block_connector_tone(
    document: &MergeDocument,
    conflict: &ConflictBlock,
) -> MergeSideLineTone {
    let mut has_base_only = false;
    let mut has_added = false;

    for row in merge_result_display_rows(document)
        .into_iter()
        .filter(|row| row.conflict_index == Some(conflict.index))
    {
        match row.tone {
            MergeSideLineTone::Replaced
            | MergeSideLineTone::Deleted
            | MergeSideLineTone::LocalDeletedRemoteEdited
            | MergeSideLineTone::LocalEditedRemoteDeleted => {
                return MergeSideLineTone::Replaced;
            }
            MergeSideLineTone::BaseOnly => has_base_only = true,
            MergeSideLineTone::Added => has_added = true,
            MergeSideLineTone::Unchanged => {}
        }
    }

    if has_base_only {
        MergeSideLineTone::BaseOnly
    } else if has_added {
        MergeSideLineTone::Added
    } else {
        MergeSideLineTone::Unchanged
    }
}

fn merge_block_result_rect_from_geometry(
    document: &MergeDocument,
    conflict: &ConflictBlock,
    geometry: &MergePanelGeometry,
) -> Option<Rect> {
    let (first, count) = merge_result_row_span_for_conflict(document, conflict)?;
    geometry.span_rect(first, count)
}

fn merge_block_side_rect_from_geometry(
    document: &MergeDocument,
    conflict: &ConflictBlock,
    side: MergeSide,
    geometry: &MergePanelGeometry,
) -> Option<Rect> {
    let (first, count) = merge_side_row_span_for_conflict(document, side, conflict)?;
    geometry.span_rect(first, count)
}

#[cfg(test)]
fn merge_result_scroll_y_for_conflict(
    document: &MergeDocument,
    conflict_index: usize,
) -> Option<f32> {
    let conflict = document
        .conflicts()
        .iter()
        .find(|conflict| conflict.index == conflict_index)?;
    let row_index = merge_result_row_span_for_conflict(document, conflict)
        .map(|(first, _)| first)
        .unwrap_or_else(|| {
            conflict
                .line_indices
                .first()
                .map(|first_line| merge_result_display_boundary_before_line(document, *first_line))
                .unwrap_or_else(|| merge_result_display_rows(document).len())
        });
    Some(row_index as f32 * MERGE_CODE_ROW_HEIGHT)
}

fn merge_result_scroll_y_for_conflict_in_view(
    document: &MergeDocument,
    conflict_index: usize,
    viewport_height: f32,
    content_height: f32,
) -> Option<f32> {
    merge_result_scroll_y_for_navigation_target_in_view(
        document,
        MergeLineActionTarget::Conflict(conflict_index),
        viewport_height,
        content_height,
    )
}

fn merge_result_scroll_y_for_navigation_target_in_view(
    document: &MergeDocument,
    target: MergeLineActionTarget,
    viewport_height: f32,
    content_height: f32,
) -> Option<f32> {
    let (row_index, row_count) = match target {
        MergeLineActionTarget::Conflict(conflict_index) => {
            let conflict = document
                .conflicts()
                .iter()
                .find(|conflict| conflict.index == conflict_index)?;
            merge_result_row_span_for_conflict(document, conflict).unwrap_or_else(|| {
                let boundary = conflict
                    .line_indices
                    .first()
                    .map(|first_line| {
                        merge_result_display_boundary_before_line(document, *first_line)
                    })
                    .unwrap_or_else(|| merge_result_display_rows(document).len());
                (boundary, 1)
            })
        }
        MergeLineActionTarget::BaseOnlyGroup(line_index) => {
            let group = base_only_display_groups(document)
                .into_iter()
                .find(|group| group.line_index == line_index)?;
            (
                merge_result_display_row_for_line(document, group.line_index)?,
                group.line_count,
            )
        }
    };
    let target_center = (row_index as f32 + row_count.max(1) as f32 * 0.5) * MERGE_CODE_ROW_HEIGHT;
    let max_scroll = (content_height - viewport_height).max(0.0);
    Some((target_center - viewport_height * 0.5).clamp(0.0, max_scroll))
}

#[allow(clippy::too_many_arguments)]
fn merge_next_shared_scroll_y(
    document: &MergeDocument,
    current_scroll_y: f32,
    local_scroll_y: Option<f32>,
    remote_scroll_y: Option<f32>,
    navigation_target: Option<MergeLineActionTarget>,
    viewport_height: f32,
    content_height: f32,
    collapse_unchanged: bool,
) -> f32 {
    if let Some(target) = navigation_target {
        return merge_result_scroll_y_for_navigation_target_in_view(
            document,
            target,
            viewport_height,
            content_height,
        )
        .unwrap_or(current_scroll_y);
    }
    if collapse_unchanged {
        return current_scroll_y;
    }
    // Keep remote-pane precedence from the previous renderer when both panes report a manual
    // offset in the same frame.
    remote_scroll_y
        .or(local_scroll_y)
        .unwrap_or(current_scroll_y)
}

fn merge_result_display_boundary_before_line(
    document: &MergeDocument,
    target_line_index: usize,
) -> usize {
    let mut display_row = 0;
    let mut line_index = 0;
    while line_index < document.lines.len() {
        if line_index == target_line_index {
            return display_row;
        }
        if let Some(conflict) = document
            .conflicts()
            .iter()
            .find(|conflict| conflict.line_indices.first().copied() == Some(line_index))
        {
            display_row += merge_result_row_span_for_conflict(document, conflict)
                .map_or(0, |(_, count)| count);
            line_index = conflict
                .line_indices
                .last()
                .map_or(line_index + 1, |last| last + 1);
            continue;
        }

        let line = &document.lines[line_index];
        display_row += if line.is_base_only_display() {
            1
        } else {
            line.result_lines().len()
        };
        line_index += 1;
    }
    display_row
}

fn merge_base_only_result_rect_from_geometry(
    document: &MergeDocument,
    group: BaseOnlyDisplayGroup,
    geometry: &MergePanelGeometry,
) -> Option<Rect> {
    let first = merge_result_display_row_for_line(document, group.line_index)?;
    geometry.span_rect(first, group.line_count)
}

fn merge_base_only_side_rect_from_geometry(
    document: &MergeDocument,
    group: BaseOnlyDisplayGroup,
    geometry: &MergePanelGeometry,
) -> Option<Rect> {
    let row_index =
        merge_side_display_row_for_line(document, group.missing_side, group.line_index)?;
    geometry.boundary_marker_rect(row_index, MERGE_BASE_ONLY_MARKER_HEIGHT)
}

fn merge_block_result_rect(
    result_panel: Rect,
    document: &MergeDocument,
    conflict: &ConflictBlock,
    scroll_y: f32,
) -> Option<Rect> {
    let (display_row, display_count) = merge_result_row_span_for_conflict(document, conflict)?;
    let clip = merge_scroll_clip_rect(result_panel);
    let top = merge_scroll_content_top(result_panel) + display_row as f32 * MERGE_CODE_ROW_HEIGHT
        - scroll_y;
    let bottom = top + display_count as f32 * MERGE_CODE_ROW_HEIGHT;
    if bottom <= clip.top() || top >= clip.bottom() {
        return None;
    }
    Some(Rect::from_min_max(
        Pos2::new(clip.left(), top.max(clip.top())),
        Pos2::new(clip.right(), bottom.min(clip.bottom())),
    ))
}

fn merge_result_row_span_for_conflict(
    document: &MergeDocument,
    conflict: &ConflictBlock,
) -> Option<(usize, usize)> {
    let rows = merge_result_display_rows(document);
    let first = rows
        .iter()
        .position(|row| row.conflict_index == Some(conflict.index))?;
    let count = rows
        .iter()
        .skip(first)
        .take_while(|row| row.conflict_index == Some(conflict.index))
        .count();
    (count > 0).then_some((first, count))
}

fn merge_block_side_rect(
    side_panel: Rect,
    document: &MergeDocument,
    conflict: &ConflictBlock,
    side: MergeSide,
    scroll_y: f32,
) -> Option<Rect> {
    let (first, count) = merge_side_row_span_for_conflict(document, side, conflict)?;

    let clip = merge_scroll_clip_rect(side_panel);
    let top =
        merge_scroll_content_top(side_panel) + first as f32 * MERGE_CODE_ROW_HEIGHT - scroll_y;
    let bottom = top + count as f32 * MERGE_CODE_ROW_HEIGHT;
    if bottom <= clip.top() || top >= clip.bottom() {
        return None;
    }
    Some(Rect::from_min_max(
        Pos2::new(side_panel.left() + 6.0, top.max(clip.top())),
        Pos2::new(side_panel.right() - 6.0, bottom.min(clip.bottom())),
    ))
}

fn merge_base_only_result_rect(
    result_panel: Rect,
    document: &MergeDocument,
    group: BaseOnlyDisplayGroup,
    scroll_y: f32,
) -> Option<Rect> {
    let display_row = merge_result_display_row_for_line(document, group.line_index)?;
    let clip = merge_scroll_clip_rect(result_panel);
    let top = merge_scroll_content_top(result_panel) + display_row as f32 * MERGE_CODE_ROW_HEIGHT
        - scroll_y;
    let bottom = top + group.line_count as f32 * MERGE_CODE_ROW_HEIGHT;
    if bottom <= clip.top() || top >= clip.bottom() {
        return None;
    }
    Some(Rect::from_min_max(
        Pos2::new(clip.left(), top.max(clip.top())),
        Pos2::new(clip.right(), bottom.min(clip.bottom())),
    ))
}

fn merge_base_only_side_rect(
    side_panel: Rect,
    document: &MergeDocument,
    group: BaseOnlyDisplayGroup,
    scroll_y: f32,
) -> Option<Rect> {
    let boundary_row =
        merge_side_display_row_for_line(document, group.missing_side, group.line_index)?;
    let clip = merge_scroll_clip_rect(side_panel);
    let top = merge_scroll_content_top(side_panel) + boundary_row as f32 * MERGE_CODE_ROW_HEIGHT
        - scroll_y;
    let bottom = top;
    let row_rect = Rect::from_min_max(Pos2::new(clip.left(), top), Pos2::new(clip.right(), bottom));
    let marker_rect = base_only_gap_marker_rect(row_rect);
    if marker_rect.bottom() <= clip.top() || marker_rect.top() >= clip.bottom() {
        return None;
    }
    Some(marker_rect)
}

fn merge_side_scroll_y_for_result_scroll(
    document: &MergeDocument,
    side: MergeSide,
    result_scroll_y: f32,
) -> f32 {
    let result_row = result_scroll_y / MERGE_CODE_ROW_HEIGHT;
    merge_mapped_scroll_row(&merge_scroll_anchors(document, side), result_row, true)
        * MERGE_CODE_ROW_HEIGHT
}

fn merge_result_scroll_y_for_side_scroll(
    document: &MergeDocument,
    side: MergeSide,
    side_scroll_y: f32,
) -> f32 {
    let side_row = side_scroll_y / MERGE_CODE_ROW_HEIGHT;
    merge_mapped_scroll_row(&merge_scroll_anchors(document, side), side_row, false)
        * MERGE_CODE_ROW_HEIGHT
}

fn merge_mapped_scroll_row(anchors: &[(f32, f32)], source_row: f32, result_to_side: bool) -> f32 {
    if anchors.is_empty() {
        return source_row.max(0.0);
    }
    let project = |anchor: (f32, f32)| {
        if result_to_side {
            (anchor.0, anchor.1)
        } else {
            (anchor.1, anchor.0)
        }
    };
    let mut points = anchors.iter().copied().map(project).collect::<Vec<_>>();
    points.sort_by(|a, b| a.0.total_cmp(&b.0));

    if source_row <= points[0].0 {
        return (points[0].1 + source_row - points[0].0).max(0.0);
    }

    for pair in points.windows(2) {
        let (source_a, target_a) = pair[0];
        let (source_b, target_b) = pair[1];
        if source_row <= source_b {
            let source_span = (source_b - source_a).max(1.0);
            let t = ((source_row - source_a) / source_span).clamp(0.0, 1.0);
            return target_a + (target_b - target_a) * t;
        }
    }

    let (source_last, target_last) = points[points.len() - 1];
    (target_last + source_row - source_last).max(0.0)
}

fn merge_scroll_anchors(document: &MergeDocument, side: MergeSide) -> Vec<(f32, f32)> {
    let mut anchors = Vec::new();
    let mut line_index = 0;
    while line_index < document.lines.len() {
        if let Some(conflict) = document
            .conflicts()
            .iter()
            .find(|conflict| conflict.line_indices.first().copied() == Some(line_index))
        {
            if let (Some((result_row, _)), Some((side_row, _))) = (
                merge_result_row_span_for_conflict(document, conflict),
                merge_side_row_span_for_conflict(document, side, conflict),
            ) {
                anchors.push((result_row as f32, side_row as f32));
            }
            line_index = conflict
                .line_indices
                .last()
                .map_or(line_index + 1, |last| last + 1);
            continue;
        }

        if let (Some(result_row), Some(side_row)) = (
            merge_result_display_row_for_line(document, line_index),
            merge_side_display_row_for_line(document, side, line_index),
        ) {
            anchors.push((result_row as f32, side_row as f32));
        }
        line_index += 1;
    }
    anchors
}

fn merge_side_row_span_for_conflict(
    document: &MergeDocument,
    side: MergeSide,
    conflict: &ConflictBlock,
) -> Option<(usize, usize)> {
    let rows = merge_side_display_rows(document, side);
    let mut visual_row = 0;
    let mut first = None;
    let mut count = 0;
    for row in &rows {
        let visual_height = merge_side_display_row_visual_height(row);
        if row.conflict_index == Some(conflict.index) {
            first.get_or_insert(visual_row);
            count += visual_height;
        } else if first.is_some() {
            break;
        }
        visual_row += visual_height;
    }
    first.and_then(|first| (count > 0).then_some((first, count)))
}

fn merge_side_display_row_for_line(
    document: &MergeDocument,
    side: MergeSide,
    target_line_index: usize,
) -> Option<usize> {
    let mut display_row = 0;
    let mut line_index = 0;
    while line_index < document.lines.len() {
        if let Some(conflict) = document
            .conflicts()
            .iter()
            .find(|conflict| conflict.line_indices.first().copied() == Some(line_index))
        {
            if conflict.line_indices.contains(&target_line_index) {
                return Some(display_row);
            }
            display_row += merge_side_row_span_for_conflict(document, side, conflict)
                .map_or(0, |(_, count)| count);
            line_index = conflict
                .line_indices
                .last()
                .map_or(line_index + 1, |last| last + 1);
            continue;
        }

        let line = &document.lines[line_index];
        let raw_missing_side = line.base_only_missing_side_raw();
        if line.base_only_resolved && raw_missing_side == Some(side) {
            if line_index == target_line_index {
                return None;
            }
            line_index += 1;
            continue;
        }

        let missing_side = line.base_only_missing_side();
        if missing_side == Some(side) {
            let group_len = base_only_gap_group_len(document, line_index, side).max(1);
            if (line_index..line_index + group_len).contains(&target_line_index) {
                return Some(display_row);
            }
            line_index += group_len;
            continue;
        }

        if line_index == target_line_index {
            return Some(display_row);
        }
        display_row += 1;
        line_index += 1;
    }
    None
}

fn merge_result_display_row_for_line(
    document: &MergeDocument,
    target_line_index: usize,
) -> Option<usize> {
    let mut display_row = 0;
    let mut line_index = 0;
    while line_index < document.lines.len() {
        if let Some(conflict) = document
            .conflicts()
            .iter()
            .find(|conflict| conflict.line_indices.first().copied() == Some(line_index))
        {
            if conflict.line_indices.contains(&target_line_index) {
                return None;
            }
            display_row += merge_result_row_span_for_conflict(document, conflict)
                .map_or(0, |(_, count)| count);
            line_index = conflict
                .line_indices
                .last()
                .map_or(line_index + 1, |last| last + 1);
            continue;
        }

        if line_index == target_line_index {
            return Some(display_row);
        }
        let line = &document.lines[line_index];
        display_row += if line.is_base_only_display() {
            1
        } else {
            line.result_lines().len()
        };
        line_index += 1;
    }
    None
}

fn merge_scroll_content_top(panel: Rect) -> f32 {
    panel.top() + 6.0 + 24.0 + MERGE_NAV_BUTTON_SIZE + 8.0
}

fn merge_scroll_clip_rect(panel: Rect) -> Rect {
    Rect::from_min_max(
        Pos2::new(panel.left() + 6.0, merge_scroll_content_top(panel)),
        Pos2::new(panel.right() - 6.0, panel.bottom() - 6.0),
    )
}

fn paint_result_block_outline(ui: &Ui, rect: Rect, tone: MergeSideLineTone, palette: MergePalette) {
    if !should_paint_result_block_outline(tone) {
        return;
    }
    let stroke = egui::Stroke::new(1.2, merge_connector_color(tone, palette));
    ui.painter()
        .line_segment([rect.left_top(), rect.right_top()], stroke);
    ui.painter()
        .line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
}

fn should_paint_result_block_outline(tone: MergeSideLineTone) -> bool {
    matches!(tone, MergeSideLineTone::Added)
}

fn paint_side_block_bridge(
    ui: &Ui,
    result_rect: Rect,
    side_rect: Rect,
    result_column: Rect,
    side_column: Rect,
    side: MergeSide,
    tone: MergeSideLineTone,
    result_endpoint_fill: Color32,
    side_endpoint_fill: Color32,
    palette: MergePalette,
) {
    let (result_inner_x, result_edge_x, side_edge_x, side_inner_x) =
        connector_bridge_x_positions(result_rect, side_rect, result_column, side_column, side);
    paint_connector_endpoint_extension(
        ui,
        result_inner_x,
        result_edge_x,
        result_rect.top(),
        result_rect.bottom(),
        result_endpoint_fill,
    );
    paint_connector_endpoint_extension(
        ui,
        side_edge_x,
        side_inner_x,
        side_rect.top(),
        side_rect.bottom(),
        side_endpoint_fill,
    );
    ui.painter().add(egui::Shape::convex_polygon(
        connector_gap_points(result_rect, side_rect, result_column, side_column, side),
        merge_connector_fill(tone, palette),
        egui::Stroke::NONE,
    ));
}

fn paint_connector_endpoint_extension(
    ui: &Ui,
    first_x: f32,
    second_x: f32,
    top: f32,
    bottom: f32,
    fill: Color32,
) {
    ui.painter().rect_filled(
        Rect::from_min_max(
            Pos2::new(first_x.min(second_x), top),
            Pos2::new(first_x.max(second_x), bottom),
        ),
        egui::CornerRadius::ZERO,
        fill,
    );
}

fn connector_bridge_x_positions(
    result_rect: Rect,
    side_rect: Rect,
    _result_column: Rect,
    side_column: Rect,
    side: MergeSide,
) -> (f32, f32, f32, f32) {
    match side {
        MergeSide::Local => (
            result_rect.left(),
            side_column.right(),
            side_rect.right(),
            side_rect.right(),
        ),
        MergeSide::Remote => (
            result_rect.right(),
            side_column.left(),
            side_rect.left(),
            side_rect.left(),
        ),
    }
}

fn connector_gap_points(
    result_rect: Rect,
    side_rect: Rect,
    result_column: Rect,
    side_column: Rect,
    side: MergeSide,
) -> Vec<Pos2> {
    let (_, result_edge_x, side_edge_x, _) =
        connector_bridge_x_positions(result_rect, side_rect, result_column, side_column, side);
    vec![
        Pos2::new(result_edge_x, result_rect.top()),
        Pos2::new(side_edge_x, side_rect.top()),
        Pos2::new(side_edge_x, side_rect.bottom()),
        Pos2::new(result_edge_x, result_rect.bottom()),
    ]
}

fn paint_base_only_marker_bridge(
    ui: &Ui,
    result_rect: Rect,
    marker_rect: Rect,
    result_column: Rect,
    side_column: Rect,
    side: MergeSide,
    palette: MergePalette,
) {
    let endpoint_fill = palette.base_only_fill;
    let (result_inner_x, result_edge_x, side_edge_x, side_inner_x) =
        connector_bridge_x_positions(result_rect, marker_rect, result_column, side_column, side);
    paint_connector_endpoint_extension(
        ui,
        result_inner_x,
        result_edge_x,
        result_rect.top(),
        result_rect.bottom(),
        endpoint_fill,
    );
    paint_connector_endpoint_extension(
        ui,
        side_edge_x,
        side_inner_x,
        marker_rect.top(),
        marker_rect.bottom(),
        endpoint_fill,
    );
    ui.painter().add(egui::Shape::convex_polygon(
        connector_gap_points(result_rect, marker_rect, result_column, side_column, side),
        merge_connector_fill(MergeSideLineTone::BaseOnly, palette),
        egui::Stroke::NONE,
    ));
}

fn paint_side_block_debug(
    ui: &Ui,
    mode: MergeConnectorDebug,
    kind: &str,
    index: usize,
    side: MergeSide,
    result_rect: Rect,
    side_rect: Rect,
    result_column: Rect,
    side_column: Rect,
    tone: MergeSideLineTone,
) {
    if mode == MergeConnectorDebug::Off {
        return;
    }

    let painter = ui.painter();
    let side_color = Color32::from_rgb(245, 158, 11);
    let result_color = Color32::from_rgb(37, 99, 235);
    let side_stroke = egui::Stroke::new(2.0, side_color);
    let result_stroke = egui::Stroke::new(2.0, result_color);
    painter.rect_stroke(
        side_rect,
        egui::CornerRadius::ZERO,
        side_stroke,
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        result_rect,
        egui::CornerRadius::ZERO,
        result_stroke,
        egui::StrokeKind::Inside,
    );

    let (result_inner_x, result_edge_x, side_edge_x, side_inner_x) =
        connector_bridge_x_positions(result_rect, side_rect, result_column, side_column, side);
    let guide_stroke = egui::Stroke::new(2.0, Color32::from_rgb(220, 38, 38));
    painter.line_segment(
        [
            Pos2::new(result_edge_x, result_rect.top()),
            Pos2::new(side_edge_x, side_rect.top()),
        ],
        guide_stroke,
    );
    painter.line_segment(
        [
            Pos2::new(result_edge_x, result_rect.bottom()),
            Pos2::new(side_edge_x, side_rect.bottom()),
        ],
        guide_stroke,
    );
    for (x, color) in [
        (result_inner_x, Color32::from_rgb(37, 99, 235)),
        (result_edge_x, Color32::from_rgb(16, 185, 129)),
        (side_edge_x, Color32::from_rgb(245, 158, 11)),
        (side_inner_x, Color32::from_rgb(147, 51, 234)),
    ] {
        painter.line_segment(
            [
                Pos2::new(x, result_rect.top()),
                Pos2::new(x, result_rect.bottom()),
            ],
            egui::Stroke::new(1.5, color),
        );
    }

    if mode == MergeConnectorDebug::Log {
        let count = MERGE_CONNECTOR_DEBUG_LOG_COUNT.fetch_add(1, Ordering::Relaxed);
        if count < 200 {
            eprintln!(
                "merge-connector {kind}#{index} side={side:?} tone={tone:?} \
                 side=({:.1},{:.1}) result=({:.1},{:.1}) \
                 x=result_inner:{:.1} result_edge:{:.1} side_edge:{:.1} side_inner:{:.1} \
                 delta_top={:.1} delta_bottom={:.1}",
                side_rect.top(),
                side_rect.bottom(),
                result_rect.top(),
                result_rect.bottom(),
                result_inner_x,
                result_edge_x,
                side_edge_x,
                side_inner_x,
                result_rect.top() - side_rect.top(),
                result_rect.bottom() - side_rect.bottom(),
            );
        }
    }
}

fn merge_connector_color(tone: MergeSideLineTone, palette: MergePalette) -> Color32 {
    match tone {
        MergeSideLineTone::Added => palette.connector,
        MergeSideLineTone::BaseOnly => palette.base_only_text,
        MergeSideLineTone::Deleted
        | MergeSideLineTone::Replaced
        | MergeSideLineTone::LocalDeletedRemoteEdited
        | MergeSideLineTone::LocalEditedRemoteDeleted => palette.conflict_text,
        MergeSideLineTone::Unchanged => palette.connector,
    }
}

fn merge_connector_fill(tone: MergeSideLineTone, palette: MergePalette) -> Color32 {
    let fill = match tone {
        MergeSideLineTone::Added => palette.added_fill,
        MergeSideLineTone::BaseOnly => palette.base_only_connector_fill,
        MergeSideLineTone::Deleted
        | MergeSideLineTone::Replaced
        | MergeSideLineTone::LocalDeletedRemoteEdited
        | MergeSideLineTone::LocalEditedRemoteDeleted => palette.conflict_fill,
        MergeSideLineTone::Unchanged => return Color32::TRANSPARENT,
    };
    color_with_opacity(fill, 0.9)
}

fn color_with_opacity(color: Color32, opacity: f32) -> Color32 {
    let alpha = (color.a() as f32 * opacity).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

fn conflict_action_rects(rect: Rect, side: MergeSide) -> ConflictActionRects {
    let top = rect.top();
    let drop_size = Vec2::new(18.0, MERGE_CODE_ROW_HEIGHT);
    let take_size = Vec2::new(28.0, MERGE_CODE_ROW_HEIGHT);
    let first_left = rect.left() + 4.0;
    let second_left = first_left + take_size.x + 2.0;
    match side {
        MergeSide::Local => ConflictActionRects {
            drop: Rect::from_min_size(Pos2::new(first_left, top), drop_size),
            take: Rect::from_min_size(Pos2::new(first_left + drop_size.x + 2.0, top), take_size),
        },
        MergeSide::Remote => ConflictActionRects {
            take: Rect::from_min_size(Pos2::new(first_left, top), take_size),
            drop: Rect::from_min_size(Pos2::new(second_left, top), drop_size),
        },
    }
}

fn merge_navigation_targets(
    document: &MergeDocument,
    side: MergeSide,
) -> Vec<MergeLineActionTarget> {
    let mut targets = document
        .conflicts()
        .iter()
        .filter(|conflict| !document.conflict_side_resolved(conflict.index, side))
        .filter_map(|conflict| {
            conflict
                .line_indices
                .first()
                .copied()
                .map(|line_index| (line_index, MergeLineActionTarget::Conflict(conflict.index)))
        })
        .collect::<Vec<_>>();
    targets.extend(
        base_only_display_groups(document)
            .into_iter()
            .filter(|group| group.missing_side == side)
            .map(|group| {
                (
                    group.line_index,
                    MergeLineActionTarget::BaseOnlyGroup(group.line_index),
                )
            }),
    );
    targets.sort_by_key(|(line_index, _)| *line_index);
    targets.into_iter().map(|(_, target)| target).collect()
}

fn merge_navigation_position(
    targets: &[MergeLineActionTarget],
    current: Option<MergeLineActionTarget>,
) -> Option<usize> {
    current.and_then(|current| targets.iter().position(|target| *target == current))
}

fn previous_navigation_position(current: usize, target_count: usize) -> usize {
    if target_count == 0 {
        0
    } else {
        (current + target_count - 1) % target_count
    }
}

fn next_navigation_position(current: usize, target_count: usize) -> usize {
    if target_count == 0 {
        0
    } else {
        (current + 1) % target_count
    }
}

fn merge_panel_frame(ui: &mut Ui, palette: MergePalette, body: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(palette.panel)
        .shadow(palette.shadow)
        .corner_radius(egui::CornerRadius::same(MERGE_PANEL_RADIUS))
        .inner_margin(egui::Margin::symmetric(6, 6))
        .show(ui, body);
}

fn apply_merge_theme(ctx: &egui::Context, theme: MergeTheme) {
    let palette = merge_palette(theme);
    let mut visuals = match theme {
        MergeTheme::Dark => egui::Visuals::dark(),
        MergeTheme::Light => egui::Visuals::light(),
    };
    visuals.panel_fill = palette.bg;
    visuals.window_fill = palette.panel;
    visuals.window_stroke = egui::Stroke::NONE;
    visuals.window_shadow = palette.shadow;
    visuals.popup_shadow = palette.shadow;
    visuals.extreme_bg_color = palette.panel_soft;
    visuals.faint_bg_color = palette.panel_soft;
    visuals.override_text_color = Some(palette.text);
    visuals.selection.bg_fill = palette.accent;
    visuals.selection.stroke = egui::Stroke::NONE;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.open.bg_stroke = egui::Stroke::NONE;
    ctx.set_visuals(visuals);
}

fn merge_palette(theme: MergeTheme) -> MergePalette {
    match theme {
        MergeTheme::Dark => MergePalette {
            bg: Color32::from_rgb(24, 27, 31),
            panel: Color32::from_rgb(29, 32, 36),
            panel_soft: Color32::from_rgb(49, 43, 43),
            text: Color32::from_rgb(222, 229, 238),
            muted: Color32::from_rgb(130, 143, 160),
            accent: Color32::from_rgb(57, 120, 220),
            conflict_fill: Color32::from_rgb(70, 47, 43),
            active_conflict_fill: Color32::from_rgb(104, 57, 48),
            conflict_text: Color32::from_rgb(255, 190, 170),
            added_fill: Color32::from_rgb(42, 68, 52),
            added_text: Color32::from_rgb(150, 226, 170),
            base_only_fill: Color32::from_rgb(81, 85, 92),
            base_only_connector_fill: Color32::from_rgb(81, 85, 92),
            base_only_text: Color32::from_rgb(192, 200, 210),
            connector: Color32::from_rgb(92, 145, 98),
            result_fill: Color32::from_rgb(31, 34, 38),
            shadow: eframe::epaint::Shadow {
                offset: [3, 4],
                blur: 12,
                spread: 0,
                color: Color32::from_rgba_unmultiplied(0, 0, 0, 90),
            },
        },
        MergeTheme::Light => MergePalette {
            bg: Color32::from_rgb(239, 242, 246),
            panel: Color32::from_rgb(253, 254, 255),
            panel_soft: Color32::from_rgb(248, 225, 219),
            text: Color32::from_rgb(32, 39, 50),
            muted: Color32::from_rgb(105, 116, 132),
            accent: Color32::from_rgb(57, 120, 220),
            conflict_fill: Color32::from_rgb(255, 224, 216),
            active_conflict_fill: Color32::from_rgb(255, 194, 178),
            conflict_text: Color32::from_rgb(154, 52, 42),
            added_fill: Color32::from_rgb(215, 246, 224),
            added_text: Color32::from_rgb(32, 128, 72),
            base_only_fill: Color32::from_rgb(214, 221, 230),
            base_only_connector_fill: Color32::from_rgb(214, 221, 230),
            base_only_text: Color32::from_rgb(92, 102, 116),
            connector: Color32::from_rgb(92, 145, 98),
            result_fill: Color32::from_rgb(255, 255, 255),
            shadow: eframe::epaint::Shadow {
                offset: [3, 4],
                blur: 12,
                spread: 0,
                color: Color32::from_rgba_unmultiplied(44, 56, 72, 44),
            },
        },
    }
}

pub fn merge_theme_label(language: MergeLanguage, theme: MergeTheme) -> &'static str {
    match theme {
        MergeTheme::Dark => mt(language, "dark"),
        MergeTheme::Light => mt(language, "light"),
    }
}

pub fn merge_language_label(language: MergeLanguage) -> &'static str {
    match language {
        MergeLanguage::Chinese => "中文",
        MergeLanguage::English => "EN",
    }
}

fn merge_ignore_mode_label(language: MergeLanguage, mode: MergeIgnoreMode) -> &'static str {
    match (language, mode) {
        (MergeLanguage::Chinese, MergeIgnoreMode::None) => "不忽略",
        (MergeLanguage::Chinese, MergeIgnoreMode::TrimWhitespace) => "忽略首尾空白",
        (MergeLanguage::Chinese, MergeIgnoreMode::IgnoreWhitespace) => "忽略全部空白",
        (_, MergeIgnoreMode::None) => "Do not ignore",
        (_, MergeIgnoreMode::TrimWhitespace) => "Trim whitespaces",
        (_, MergeIgnoreMode::IgnoreWhitespace) => "Ignore whitespaces",
    }
}

fn merge_highlight_mode_label(language: MergeLanguage, mode: MergeHighlightMode) -> &'static str {
    match (language, mode) {
        (MergeLanguage::Chinese, MergeHighlightMode::Lines) => "高亮行",
        (MergeLanguage::Chinese, MergeHighlightMode::Words) => "高亮词",
        (_, MergeHighlightMode::Lines) => "Highlight lines",
        (_, MergeHighlightMode::Words) => "Highlight words",
    }
}

fn mt(language: MergeLanguage, key: &str) -> &'static str {
    match (language, key) {
        (MergeLanguage::Chinese, "title") => "合并修订",
        (MergeLanguage::Chinese, "conflicts") => "个冲突",
        (MergeLanguage::Chinese, "auto_applied") => "非冲突内容已自动合并",
        (MergeLanguage::Chinese, "no_changes") => "无其他变更。",
        (MergeLanguage::Chinese, "conflict_count") => "个冲突。",
        (MergeLanguage::Chinese, "local") => "左边",
        (MergeLanguage::Chinese, "remote") => "右边",
        (MergeLanguage::Chinese, "result") => "中间",
        (MergeLanguage::Chinese, "search_placeholder") => "搜索代码",
        (MergeLanguage::Chinese, "accept_left") => "使用左边",
        (MergeLanguage::Chinese, "accept_right") => "使用右边",
        (MergeLanguage::Chinese, "apply") => "应用",
        (MergeLanguage::Chinese, "cancel") => "取消",
        (MergeLanguage::Chinese, "light") => "白天",
        (MergeLanguage::Chinese, "dark") => "黑夜",
        (MergeLanguage::Chinese, "analyzing_merge") => "正在分析合并内容...",
        (MergeLanguage::Chinese, "ai_analyze") => "使用 AI 分析冲突",
        (MergeLanguage::Chinese, "ai_analyzing") => "AI 正在分析冲突",
        (MergeLanguage::Chinese, "ai_completed_prefix") => "AI 分析完成：",
        (MergeLanguage::Chinese, "ai_completed_suffix") => "条建议",
        (MergeLanguage::Chinese, "ai_changes_suffix") => "项实际改动",
        (MergeLanguage::Chinese, "ai_actual_changes") => "将执行改动",
        (MergeLanguage::Chinese, "ai_target_result") => "冲突区结果",
        (MergeLanguage::Chinese, "ai_middle_edit") => "中间代码",
        (MergeLanguage::Chinese, "ai_no_suggestions") => "AI 分析完成：暂无可用建议",
        (MergeLanguage::Chinese, "ai_no_reason") => "未提供说明。",
        (MergeLanguage::Chinese, "ai_choose_local") => "建议采用左边",
        (MergeLanguage::Chinese, "ai_choose_remote") => "建议采用右边",
        (MergeLanguage::Chinese, "ai_manual") => "建议在中间手动处理",
        (MergeLanguage::Chinese, "ai_apply_suggestion") => "确定采用建议",
        (MergeLanguage::Chinese, "ai_apply_all") => "应用所有建议",
        (MergeLanguage::Chinese, "ai_apply_all_hint") => {
            "一次应用所有可执行建议；需人工处理的建议保留。可一次撤销，不自动保存文件；若建议失效或修改冲突，整批不应用。"
        }
        (MergeLanguage::Chinese, "ai_apply_hint") => "仅将建议应用到合并结果；不会自动保存文件",
        (MergeLanguage::Chinese, "ai_ignore") => "忽略此建议",
        (MergeLanguage::Chinese, "ai_sources_unavailable") => "AI 分析需要三个版本的已加载内容",
        (MergeLanguage::Chinese, "ai_analysis_stopped") => "AI 分析任务已停止",
        (MergeLanguage::Chinese, "analysis_stopped") => "合并分析已停止",
        (MergeLanguage::Chinese, "loading_title") => "准备合并编辑器",
        (MergeLanguage::Chinese, "loading_reading") => "读取三个版本",
        (MergeLanguage::Chinese, "loading_comparing") => "比较三方差异",
        (MergeLanguage::Chinese, "loading_preparing") => "准备编辑器",
        (MergeLanguage::Chinese, "loading_done") => "已完成",
        (MergeLanguage::Chinese, "loading_active") => "处理中",
        (MergeLanguage::Chinese, "loading_waiting") => "等待",
        (MergeLanguage::Chinese, "applying") => "应用中...",
        (MergeLanguage::Chinese, "write_failed") => "写入失败",
        (MergeLanguage::Chinese, "write_stopped") => "写入已停止",
        (MergeLanguage::Chinese, "resolve_all_conflicts") => {
            "\u{8bf7}\u{5148}\u{89e3}\u{51b3}\u{6240}\u{6709}\u{51b2}\u{7a81}"
        }
        (MergeLanguage::Chinese, "result_placeholder") => {
            "\u{8bf7}\u{8f93}\u{5165}\u{5408}\u{5e76}\u{7ed3}\u{679c}"
        }
        (MergeLanguage::Chinese, "cancel_merge_title") => "\u{53d6}\u{6d88}\u{5408}\u{5e76}",
        (MergeLanguage::Chinese, "cancel_merge_message") => {
            "\u{5408}\u{5e76}\u{7ed3}\u{679c}\u{4e2d}\u{6709}\u{672a}\u{4fdd}\u{5b58}\u{7684}\u{66f4}\u{6539}\u{3002}\u{8981}\u{4e22}\u{5f03}\u{66f4}\u{6539}\u{5e76}\u{53d6}\u{6d88}\u{5408}\u{5e76}\u{5417}\u{ff1f}"
        }
        (MergeLanguage::Chinese, "cancel_merge_discard") => {
            "\u{4e22}\u{5f03}\u{66f4}\u{6539}\u{5e76}\u{53d6}\u{6d88}\u{5408}\u{5e76}"
        }
        (MergeLanguage::Chinese, "cancel_merge_continue") => "\u{7ee7}\u{7eed}\u{5408}\u{5e76}",
        (MergeLanguage::Chinese, "edit_result") => "\u{7f16}\u{8f91}\u{7ed3}\u{679c}",
        (MergeLanguage::Chinese, "editing_result") => "\u{6b63}\u{5728}\u{7f16}\u{8f91}",
        (MergeLanguage::Chinese, "collapse_unchanged") => "收起未变块",
        (MergeLanguage::Chinese, "expand_unchanged") => "展开未变块",
        (MergeLanguage::Chinese, "unchanged_lines") => "行未变更",
        (MergeLanguage::Chinese, "manual_result_hint") => {
            "\u{624b}\u{52a8}\u{7f16}\u{8f91}\u{540e}\u{5c06}\u{4ee5}\u{4e2d}\u{95f4}\u{7ed3}\u{679c}\u{4e3a}\u{51c6}"
        }
        (_, "title") => "Merge Revisions",
        (_, "conflicts") => "conflict(s)",
        (_, "auto_applied") => "Non-conflicting changes auto-applied",
        (_, "no_changes") => "No changes.",
        (_, "conflict_count") => "conflict(s).",
        (_, "local") => "Left",
        (_, "remote") => "Right",
        (_, "result") => "Middle",
        (_, "search_placeholder") => "Search code",
        (_, "accept_left") => "Use Left",
        (_, "accept_right") => "Use Right",
        (_, "apply") => "Apply",
        (_, "cancel") => "Cancel",
        (_, "light") => "Light",
        (_, "dark") => "Dark",
        (_, "analyzing_merge") => "Analyzing merge content...",
        (_, "ai_analyze") => "Analyze conflicts with AI",
        (_, "ai_analyzing") => "AI is analyzing conflicts",
        (_, "ai_completed_prefix") => "AI analysis complete:",
        (_, "ai_completed_suffix") => "suggestion(s)",
        (_, "ai_changes_suffix") => "actual change(s)",
        (_, "ai_actual_changes") => "Changes to apply",
        (_, "ai_target_result") => "Conflict result",
        (_, "ai_middle_edit") => "Middle code",
        (_, "ai_no_suggestions") => "AI analysis complete: no suggestions",
        (_, "ai_no_reason") => "No explanation provided.",
        (_, "ai_choose_local") => "Recommend Left",
        (_, "ai_choose_remote") => "Recommend Right",
        (_, "ai_manual") => "Edit in Middle manually",
        (_, "ai_apply_suggestion") => "Apply suggestion",
        (_, "ai_apply_all") => "Apply all suggestions",
        (_, "ai_apply_all_hint") => {
            "Apply all actionable suggestions in one undoable step; manual-only advice remains. Does not save the file. Stale or conflicting edits reject the entire batch."
        }
        (_, "ai_apply_hint") => {
            "Updates the merge result only; it does not save the file automatically."
        }
        (_, "ai_ignore") => "Ignore this suggestion",
        (_, "ai_sources_unavailable") => "AI analysis needs the loaded three-way file contents",
        (_, "ai_analysis_stopped") => "AI analysis stopped",
        (_, "analysis_stopped") => "Merge analysis stopped",
        (_, "loading_title") => "Preparing merge editor",
        (_, "loading_reading") => "Read three versions",
        (_, "loading_comparing") => "Compare three-way changes",
        (_, "loading_preparing") => "Prepare editor",
        (_, "loading_done") => "Done",
        (_, "loading_active") => "Working",
        (_, "loading_waiting") => "Waiting",
        (_, "applying") => "Applying...",
        (_, "write_failed") => "Failed to write",
        (_, "write_stopped") => "Write stopped",
        (_, "resolve_all_conflicts") => "Resolve all conflicts before applying",
        (_, "result_placeholder") => "Enter merge result",
        (_, "cancel_merge_title") => "Cancel Merge",
        (_, "cancel_merge_message") => {
            "There are unsaved changes in the result file. Discard changes and cancel merge anyway?"
        }
        (_, "cancel_merge_discard") => "Discard Changes and Cancel Merge",
        (_, "cancel_merge_continue") => "Continue Merge",
        (_, "edit_result") => "Edit Result",
        (_, "editing_result") => "Editing",
        (_, "collapse_unchanged") => "Collapse unchanged",
        (_, "expand_unchanged") => "Expand unchanged",
        (_, "unchanged_lines") => "unchanged lines",
        (_, "manual_result_hint") => {
            "Manual edits resolve the remaining conflicts from this result."
        }
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static MERGE_LOAD_TEST_ID: AtomicUsize = AtomicUsize::new(0);
    static MERGE_AI_CONTEXT_TEST_ID: AtomicUsize = AtomicUsize::new(0);

    fn test_merge_args() -> MergeArgs {
        MergeArgs {
            base: PathBuf::from("base.txt"),
            local: PathBuf::from("local.txt"),
            remote: PathBuf::from("remote.txt"),
            output: PathBuf::from("merged.txt"),
            repo_root: None,
            stage: false,
            theme: MergeTheme::Light,
            language: MergeLanguage::Chinese,
            ai_model_name: None,
        }
    }

    fn run_context_test_git(repo: &Path, args: &[&str]) -> String {
        let mut command = Command::new("git");
        #[cfg(target_os = "windows")]
        command.creation_flags(MERGE_WINDOWS_CREATE_NO_WINDOW);
        let output = command
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("unable to run git {}: {error}", args.join(" ")));
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    fn live_merge_stage_text(repo: &Path, stage: usize, relative_path: &str) -> String {
        let mut command = Command::new("git");
        #[cfg(target_os = "windows")]
        command.creation_flags(MERGE_WINDOWS_CREATE_NO_WINDOW);
        let output = command
            .arg("-C")
            .arg(repo)
            .args(["show", &format!(":{stage}:{relative_path}")])
            .output()
            .unwrap_or_else(|error| {
                panic!("unable to read stage {stage} for {relative_path}: {error}")
            });
        assert!(
            output.status.success(),
            "unable to read stage {stage} for {relative_path}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap_or_else(|error| {
            panic!("stage {stage} for {relative_path} is not UTF-8: {error}")
        })
    }

    /// Manual live regression for the configured model. It reads Git's unmerged index stages and
    /// calls the same context collector, tool schema, HTTP request, parser, and safety guard as the
    /// Merge window, without opening or controlling any UI. API keys are never printed.
    #[test]
    #[ignore = "requires a configured AI model and explicit GIT_AGENT_LIVE_AI_REPO"]
    fn live_merge_ai_self_test_current_index() {
        let repo = PathBuf::from(
            env::var("GIT_AGENT_LIVE_AI_REPO")
                .expect("set GIT_AGENT_LIVE_AI_REPO to an intentionally conflicted repository"),
        );
        let configured_files = env::var("GIT_AGENT_LIVE_AI_FILES").unwrap_or_default();
        let files = if configured_files.trim().is_empty() {
            run_context_test_git(&repo, &["diff", "--name-only", "--diff-filter=U"])
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        } else {
            configured_files
                .split(';')
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        assert!(
            !files.is_empty(),
            "repository has no selected unmerged files"
        );
        let requested_model = env::var("GIT_AGENT_LIVE_AI_MODEL").ok();
        let config = crate::app::load_merge_ai_model_config(requested_model.as_deref())
            .expect("unable to load the configured AI model");
        println!(
            "LIVE_AI_CONFIG\tname={}\tformat={:?}\tbase_url={}\tmodel_id={}",
            config.name,
            config.api_format,
            merge_ai_url_for_log(&config.base_url),
            config.model_id,
        );

        let temp_root = env::temp_dir().join(format!(
            "git-agent-live-ai-{}-{}",
            std::process::id(),
            MERGE_AI_CONTEXT_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&temp_root).expect("unable to create live AI temporary directory");

        for (file_index, relative_path) in files.iter().enumerate() {
            let base = live_merge_stage_text(&repo, 1, relative_path);
            let local = live_merge_stage_text(&repo, 2, relative_path);
            let remote = live_merge_stage_text(&repo, 3, relative_path);
            let document = three_way_merge(&base, &local, &remote);
            let sources = MergeSourceText {
                base: base.clone(),
                local: local.clone(),
                remote: remote.clone(),
            };
            let file_temp = temp_root.join(file_index.to_string());
            fs::create_dir_all(&file_temp).expect("unable to create file temporary directory");
            let base_path = file_temp.join("base.ts");
            let local_path = file_temp.join("left.ts");
            let remote_path = file_temp.join("right.ts");
            fs::write(&base_path, base).expect("unable to write base stage");
            fs::write(&local_path, local).expect("unable to write left stage");
            fs::write(&remote_path, remote).expect("unable to write right stage");
            let args = MergeArgs {
                base: base_path,
                local: local_path,
                remote: remote_path,
                output: repo.join(relative_path),
                repo_root: Some(repo.clone()),
                stage: false,
                theme: MergeTheme::Light,
                language: MergeLanguage::Chinese,
                ai_model_name: requested_model.clone(),
            };
            let context =
                collect_merge_ai_context(&args, &sources, &document).unwrap_or_else(|error| {
                    panic!("context collection failed for {relative_path}: {error}")
                });
            println!(
                "LIVE_AI_CONTEXT\tfile={}\tconflicts={}\tdeletions={}\thistory_chars={}\tstate_chars={}\treferences_chars={}\trelated_chars={}",
                relative_path,
                document.conflicts().len(),
                base_only_display_groups(&document).len(),
                context.history.chars().count(),
                context.repository_state.chars().count(),
                context.symbol_references.chars().count(),
                context.related_files.chars().count(),
            );
            for conflict in document.conflicts() {
                println!(
                    "LIVE_AI_TARGET\tfile={}\ttarget=Conflict({})\tbase={}\tleft={}\tright={}",
                    relative_path,
                    conflict.index,
                    serde_json::to_string(&conflict.base).expect("encode base conflict"),
                    serde_json::to_string(&conflict.local).expect("encode left conflict"),
                    serde_json::to_string(&conflict.remote).expect("encode right conflict"),
                );
            }
            for group in base_only_display_groups(&document) {
                let lines = &document.lines[group.line_index..group.line_index + group.line_count];
                println!(
                    "LIVE_AI_TARGET\tfile={}\ttarget=BaseOnlyGroup({})\tmissing_side={:?}\tbase={}\tleft={}\tright={}",
                    relative_path,
                    group.line_index,
                    group.missing_side,
                    serde_json::to_string(
                        &lines
                            .iter()
                            .filter_map(|line| line.base.as_deref())
                            .collect::<Vec<_>>()
                    )
                    .expect("encode base deletion"),
                    serde_json::to_string(
                        &lines
                            .iter()
                            .filter_map(|line| line.local.as_deref())
                            .collect::<Vec<_>>()
                    )
                    .expect("encode left deletion"),
                    serde_json::to_string(
                        &lines
                            .iter()
                            .filter_map(|line| line.remote.as_deref())
                            .collect::<Vec<_>>()
                    )
                    .expect("encode right deletion"),
                );
            }
            let expected_targets =
                document.conflicts().len() + base_only_display_groups(&document).len();
            let suggestions =
                request_merge_ai_suggestions(&config, &args, &sources, &document, &context)
                    .unwrap_or_else(|error| {
                        panic!("AI request failed for {relative_path}: {error}")
                    });
            println!(
                "LIVE_AI_SUMMARY\tfile={}\texpected_targets={}\taccepted_suggestions={}",
                relative_path,
                expected_targets,
                suggestions.len(),
            );
            for suggestion in &suggestions {
                let middle_edits = suggestion
                    .middle_edits
                    .iter()
                    .map(|edit| {
                        serde_json::json!({
                            "expected_text": edit.expected_text,
                            "replacement_text": edit.replacement_text,
                        })
                    })
                    .collect::<Vec<_>>();
                println!(
                    "LIVE_AI_SUGGESTION\tfile={}\ttarget={:?}\tchoice={:?}\tmanual_result={}\tmiddle_edits={}\treason_zh={}\treason_en={}",
                    relative_path,
                    suggestion.target,
                    suggestion.choice,
                    serde_json::to_string(&suggestion.manual_result).expect("encode manual result"),
                    serde_json::to_string(&middle_edits).expect("encode middle edits"),
                    suggestion.reason_zh.replace(['\r', '\n'], " "),
                    suggestion.reason_en.replace(['\r', '\n'], " "),
                );
            }
            assert_eq!(
                suggestions.len(),
                expected_targets,
                "the model must return one accepted suggestion per merge target for {relative_path}"
            );
        }

        let _ = fs::remove_dir_all(temp_root);
    }

    /// Live regression for the failure mode where one pane retains an import even though the
    /// complete Middle draft no longer contains any use of its binding. This stays in memory and
    /// does not alter the repository or its intentionally conflicted index.
    #[test]
    #[ignore = "requires a configured AI model and explicit GIT_AGENT_LIVE_AI_REPO"]
    fn live_merge_ai_removed_reference_prefers_deletion() {
        let repo = PathBuf::from(
            env::var("GIT_AGENT_LIVE_AI_REPO")
                .expect("set GIT_AGENT_LIVE_AI_REPO to the merge test repository"),
        );
        let requested_model = env::var("GIT_AGENT_LIVE_AI_MODEL").ok();
        let config = crate::app::load_merge_ai_model_config(requested_model.as_deref())
            .expect("unable to load the configured AI model");
        let base = r#"import { legacyDiscount } from "./legacyDiscount";
import { calculateModernPrice } from "./modernPrice";

export function quote(total: number): number {
  const legacy = legacyDiscount(total);
  return calculateModernPrice(total) + legacy;
}
"#;
        let local = r#"import { calculateModernPrice } from "./modernPrice";

export function quote(total: number): number {
  return calculateModernPrice(total);
}
"#;
        let remote = r#"import { legacyDiscount } from "./legacyDiscount";
import { calculateModernPrice } from "./modernPrice";

export function quote(total: number): number {
  const auditedTotal = Math.max(total, 0);
  return calculateModernPrice(auditedTotal);
}
"#;
        let document = three_way_merge(base, local, remote);
        let sources = MergeSourceText {
            base: base.to_owned(),
            local: local.to_owned(),
            remote: remote.to_owned(),
        };
        let deletion = base_only_display_groups(&document)
            .into_iter()
            .find(|group| {
                document.lines[group.line_index..group.line_index + group.line_count]
                    .iter()
                    .any(|line| {
                        line.base
                            .as_deref()
                            .is_some_and(|line| line.contains("legacyDiscount"))
                    })
            })
            .expect("fixture must expose the stale import as a deletion decision");
        assert_eq!(deletion.missing_side, MergeSide::Local);
        assert_eq!(
            count_identifier_occurrences(&document.result_text(), "legacyDiscount"),
            0,
            "the Middle draft should already contain no stale import or usage"
        );

        let temp_root = env::temp_dir().join(format!(
            "git-agent-live-ai-removed-reference-{}-{}",
            std::process::id(),
            MERGE_AI_CONTEXT_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&temp_root).expect("unable to create live AI temporary directory");
        let base_path = temp_root.join("base.ts");
        let local_path = temp_root.join("left.ts");
        let remote_path = temp_root.join("right.ts");
        fs::write(&base_path, base).expect("unable to write base fixture");
        fs::write(&local_path, local).expect("unable to write left fixture");
        fs::write(&remote_path, remote).expect("unable to write right fixture");
        let args = MergeArgs {
            base: base_path,
            local: local_path,
            remote: remote_path,
            output: repo.join("src/checkout/deletedImportProbe.ts"),
            repo_root: Some(repo),
            stage: false,
            theme: MergeTheme::Light,
            language: MergeLanguage::Chinese,
            ai_model_name: requested_model,
        };
        let context = collect_merge_ai_context(&args, &sources, &document)
            .expect("unable to collect live AI context");
        let suggestions =
            request_merge_ai_suggestions(&config, &args, &sources, &document, &context)
                .expect("live AI request failed");
        for suggestion in &suggestions {
            println!(
                "LIVE_AI_REMOVED_REFERENCE\ttarget={:?}\tchoice={:?}\treason_zh={}\treason_en={}",
                suggestion.target,
                suggestion.choice,
                suggestion.reason_zh.replace(['\r', '\n'], " "),
                suggestion.reason_en.replace(['\r', '\n'], " "),
            );
        }
        let suggestion = suggestions
            .iter()
            .find(|suggestion| {
                suggestion.target == MergeLineActionTarget::BaseOnlyGroup(deletion.line_index)
            })
            .expect("model must return the stale-import deletion suggestion");
        assert_eq!(
            suggestion.choice,
            MergeAiChoice::Local,
            "the Left deletion must win because Middle has no legacyDiscount usage"
        );
        let _ = fs::remove_dir_all(temp_root);
    }

    /// Live regression for a stale child event binding after the changed pane moved the same
    /// handler to the enclosing trigger. The target repository is context-only and remains
    /// untouched; all three merge inputs live in a temporary directory.
    #[test]
    #[ignore = "requires a configured AI model and explicit GIT_AGENT_LIVE_AI_REPO"]
    fn live_merge_ai_moved_event_binding_prefers_deletion() {
        let repo = PathBuf::from(
            env::var("GIT_AGENT_LIVE_AI_REPO")
                .expect("set GIT_AGENT_LIVE_AI_REPO to the merge test repository"),
        );
        let requested_model = env::var("GIT_AGENT_LIVE_AI_MODEL").ok();
        let config = crate::app::load_merge_ai_model_config(requested_model.as_deref())
            .expect("unable to load the configured AI model");
        let (sources, document) = moved_vue_event_binding_fixture();
        let deletions = base_only_display_groups(&document);
        assert_eq!(deletions.len(), 2);
        assert!(
            deletions
                .iter()
                .all(|deletion| deletion.missing_side == MergeSide::Local)
        );

        let temp_root = env::temp_dir().join(format!(
            "git-agent-live-ai-moved-event-{}-{}",
            std::process::id(),
            MERGE_AI_CONTEXT_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&temp_root).expect("unable to create live AI temporary directory");
        let base_path = temp_root.join("base.vue");
        let local_path = temp_root.join("left.vue");
        let remote_path = temp_root.join("right.vue");
        fs::write(&base_path, &sources.base).expect("unable to write base fixture");
        fs::write(&local_path, &sources.local).expect("unable to write left fixture");
        fs::write(&remote_path, &sources.remote).expect("unable to write right fixture");
        let args = MergeArgs {
            base: base_path,
            local: local_path,
            remote: remote_path,
            output: repo.join("src/components/dropdown/PSelect.vue"),
            repo_root: Some(repo),
            stage: false,
            theme: MergeTheme::Light,
            language: MergeLanguage::Chinese,
            ai_model_name: requested_model,
        };
        let context = collect_merge_ai_context(&args, &sources, &document)
            .expect("unable to collect live AI context");
        let suggestions =
            request_merge_ai_suggestions(&config, &args, &sources, &document, &context)
                .expect("live AI request failed");
        for suggestion in &suggestions {
            println!(
                "LIVE_AI_MOVED_EVENT\ttarget={:?}\tchoice={:?}\treason_zh={}\treason_en={}",
                suggestion.target,
                suggestion.choice,
                suggestion.reason_zh.replace(['\r', '\n'], " "),
                suggestion.reason_en.replace(['\r', '\n'], " "),
            );
        }
        for deletion in deletions {
            let target = MergeLineActionTarget::BaseOnlyGroup(deletion.line_index);
            let suggestion = suggestions
                .iter()
                .find(|suggestion| suggestion.target == target)
                .unwrap_or_else(|| panic!("model must return suggestion for {target:?}"));
            assert_eq!(
                suggestion.choice,
                MergeAiChoice::Local,
                "the changed Left pane moved the click handler to the enclosing trigger"
            );
        }
        let _ = fs::remove_dir_all(temp_root);
    }

    fn navigation_matrix_document() -> MergeDocument {
        three_way_merge(
            "header\nconflict-a = base\nstable-a\ndelete-from-local\nstable-b\nconflict-b = base\nstable-c\ndelete-from-remote\nstable-d\nconflict-c = base\nfooter\n",
            "header\nconflict-a = local\nstable-a\nstable-b\nconflict-b = local\nstable-c\ndelete-from-remote\nstable-d\nconflict-c = local\nfooter\n",
            "header\nconflict-a = remote\nstable-a\ndelete-from-local\nstable-b\nconflict-b = remote\nstable-c\nstable-d\nconflict-c = remote\nfooter\n",
        )
    }

    #[test]
    fn ai_suggestion_json_is_strictly_bound_to_current_conflicts() {
        let valid_targets = HashSet::from([
            MergeLineActionTarget::Conflict(0),
            MergeLineActionTarget::Conflict(2),
        ]);
        let suggestions = parse_merge_ai_suggestions(
            r#"{"suggestions":[
                {"conflict_index":0,"choice":"left","reason":"keeps local validation"},
                {"conflict_index":2,"choice":"manual","reason":"both changes are needed","merge_order_zh":"先执行左边校验，再执行右边回退","merge_order_en":"Run Left validation before the Right fallback"},
                {"conflict_index":3,"choice":"remote","reason":"not current"}
            ]}"#,
            &valid_targets,
            MergeLanguage::English,
        )
        .unwrap();

        assert_eq!(suggestions.len(), 2);
        assert_eq!(suggestions[0].choice, MergeAiChoice::Local);
        assert_eq!(suggestions[1].choice, MergeAiChoice::Manual);
        assert!(
            suggestions[1]
                .reason_zh
                .contains("合并顺序：先执行左边校验")
        );
        assert!(
            suggestions[1]
                .reason_en
                .contains("Merge order: Run Left validation")
        );
        assert!(
            !merge_ai_response_has_exact_target_coverage(
                r#"{"suggestions":[
                    {"conflict_index":0,"choice":"left"},
                    {"conflict_index":2,"choice":"manual"},
                    {"conflict_index":3,"choice":"right"}
                ]}"#,
                &valid_targets,
            ),
            "an otherwise parseable extra target must trigger a coverage repair"
        );
    }

    #[test]
    fn ai_response_coverage_requires_each_current_target_exactly_once() {
        let valid_targets = HashSet::from([
            MergeLineActionTarget::Conflict(0),
            MergeLineActionTarget::BaseOnlyGroup(17),
        ]);
        let exact = r#"{"suggestions":[
            {"target_type":"conflict","target_index":0},
            {"target_type":"deletion","target_index":17}
        ]}"#;
        let missing = r#"{"suggestions":[
            {"target_type":"conflict","target_index":0}
        ]}"#;
        let duplicate = r#"{"suggestions":[
            {"target_type":"conflict","target_index":0},
            {"target_type":"conflict","target_index":0}
        ]}"#;
        let extra = r#"{"suggestions":[
            {"target_type":"conflict","target_index":0},
            {"target_type":"deletion","target_index":17},
            {"target_type":"conflict","target_index":1}
        ]}"#;

        assert!(merge_ai_response_has_exact_target_coverage(
            exact,
            &valid_targets
        ));
        assert!(!merge_ai_response_has_exact_target_coverage(
            missing,
            &valid_targets
        ));
        assert!(!merge_ai_response_has_exact_target_coverage(
            duplicate,
            &valid_targets
        ));
        assert!(!merge_ai_response_has_exact_target_coverage(
            extra,
            &valid_targets
        ));
    }

    #[test]
    fn manual_ai_suggestion_without_explicit_merge_order_is_rejected() {
        let valid_targets = HashSet::from([MergeLineActionTarget::Conflict(0)]);
        let suggestions = parse_merge_ai_suggestions(
            r#"{"suggestions":[{
                "target_type":"conflict",
                "target_index":0,
                "choice":"manual",
                "reason_zh":"保留两边",
                "reason_en":"Keep both sides"
            }]}"#,
            &valid_targets,
            MergeLanguage::Chinese,
        )
        .unwrap();

        assert!(suggestions.is_empty());
    }

    #[test]
    fn claude_response_ignores_thinking_blocks_and_reads_final_text() {
        let response = serde_json::json!({
            "type": "message",
            "stop_reason": "end_turn",
            "content": [
                { "type": "thinking", "thinking": "internal analysis" },
                { "type": "text", "text": "{\"suggestions\":[]}" }
            ],
            "usage": { "input_tokens": 120, "output_tokens": 30 }
        });

        assert_eq!(
            merge_ai_response_content(crate::app::MergeAiApiFormat::Claude, &response).unwrap(),
            "{\"suggestions\":[]}",
        );
        let structure =
            merge_ai_response_structure(crate::app::MergeAiApiFormat::Claude, &response);
        assert!(structure.contains("stop_reason=end_turn"));
        assert!(structure.contains("content_types=thinking,text"));
        assert!(structure.contains("input_tokens=120"));
        assert!(structure.contains("output_tokens=30"));
    }

    #[test]
    fn claude_compatible_endpoint_accepts_openai_response_envelope() {
        let response = serde_json::json!({
            "choices": [{
                "finish_reason": "stop",
                "message": {
                    "content": [{ "type": "text", "text": "{\"suggestions\":[]}" }]
                }
            }]
        });

        assert_eq!(
            merge_ai_response_content(crate::app::MergeAiApiFormat::Claude, &response).unwrap(),
            "{\"suggestions\":[]}",
        );
    }

    #[test]
    fn openai_function_call_returns_merge_suggestion_payload() {
        let response = serde_json::json!({
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "content": null,
                    "tool_calls": [{
                        "type": "function",
                        "function": {
                            "name": MERGE_AI_TOOL_NAME,
                            "arguments": "{\"suggestions\":[{\"target_type\":\"conflict\",\"target_index\":0,\"choice\":\"manual\",\"reason_zh\":\"左右两边都很重要\",\"reason_en\":\"both sides matter\",\"merge_order_zh\":\"先左后右\",\"merge_order_en\":\"Left before Right\"}]}"
                        }
                    }]
                }
            }]
        });

        let (payload, mode) =
            merge_ai_response_payload(crate::app::MergeAiApiFormat::OpenAiCompatible, &response)
                .unwrap();
        assert_eq!(mode, "openai_function");
        assert!(payload.contains("both sides matter"));
        let structure =
            merge_ai_response_structure(crate::app::MergeAiApiFormat::OpenAiCompatible, &response);
        assert!(structure.contains("finish_reason=tool_calls"));
        assert!(structure.contains("tool_names=submit_merge_suggestions"));
    }

    #[test]
    fn anthropic_tool_use_returns_merge_suggestion_payload() {
        let response = serde_json::json!({
            "type": "message",
            "stop_reason": "tool_use",
            "content": [{
                "type": "tool_use",
                "name": MERGE_AI_TOOL_NAME,
                "input": {
                    "suggestions": [{
                        "target_type": "deletion",
                        "target_index": 17,
                        "choice": "left",
                        "reason_zh": "左边有意删除了这段内容",
                        "reason_en": "the Left deletion is intentional"
                    }]
                }
            }]
        });

        let (payload, mode) =
            merge_ai_response_payload(crate::app::MergeAiApiFormat::Claude, &response).unwrap();
        assert_eq!(mode, "anthropic_tool_use");
        assert!(payload.contains("the Left deletion is intentional"));
        let structure =
            merge_ai_response_structure(crate::app::MergeAiApiFormat::Claude, &response);
        assert!(structure.contains("stop_reason=tool_use"));
        assert!(structure.contains("content_types=tool_use"));
        assert!(structure.contains("tool_names=submit_merge_suggestions"));
    }

    #[test]
    fn merge_suggestion_tool_schema_exposes_confirmable_middle_edits_without_file_writes() {
        let schema = merge_ai_suggestions_input_schema();
        assert_eq!(
            schema.pointer("/properties/suggestions/items/properties/choice/enum"),
            Some(&serde_json::json!(["left", "right", "manual"])),
        );
        assert!(schema.to_string().contains("reason_zh"));
        assert!(schema.to_string().contains("reason_en"));
        assert_eq!(
            schema.pointer("/properties/suggestions/items/properties/reason_zh/maxLength"),
            Some(&serde_json::json!(220)),
        );
        assert_eq!(
            schema.pointer("/properties/suggestions/items/properties/reason_en/maxLength"),
            Some(&serde_json::json!(320)),
        );
        assert!(schema.to_string().contains("merge_order_zh"));
        assert!(schema.to_string().contains("merge_order_en"));
        assert!(schema.to_string().contains("manual_result_provided"));
        assert!(schema.to_string().contains("manual_result"));
        assert!(schema.to_string().contains("middle_edits"));
        assert!(schema.to_string().contains("expected_text"));
        assert!(schema.to_string().contains("replacement_text"));
        assert!(!schema.to_string().contains("write_file"));
        assert!(!schema.to_string().contains("git_commit"));
    }

    #[test]
    fn manual_suggestion_parser_keeps_exact_target_result_and_middle_edits() {
        let valid_targets = HashSet::from([MergeLineActionTarget::Conflict(0)]);
        let suggestions = parse_merge_ai_suggestions(
            r#"{"suggestions":[{
                "target_type":"conflict",
                "target_index":0,
                "choice":"manual",
                "manual_result_provided":true,
                "manual_result":"const fields = ['left', 'right']",
                "middle_edits":[{
                    "expected_text":"const fieldCount = 1",
                    "replacement_text":"const fieldCount = fields.length"
                }],
                "reason_zh":"两边字段都需要保留并同步派生数量",
                "reason_en":"Keep both fields and synchronize the derived count",
                "merge_order_zh":"字段按左边在前、右边在后排列，然后计算数量",
                "merge_order_en":"Place Left before Right, then calculate the count"
            }]}"#,
            &valid_targets,
            MergeLanguage::Chinese,
        )
        .unwrap();

        assert_eq!(suggestions.len(), 1);
        assert_eq!(
            suggestions[0].manual_result.as_deref(),
            Some("const fields = ['left', 'right']")
        );
        assert_eq!(
            suggestions[0].middle_edits,
            vec![MergeAiMiddleEdit {
                expected_text: "const fieldCount = 1".to_owned(),
                replacement_text: "const fieldCount = fields.length".to_owned(),
            }]
        );
        assert!(suggestions[0].is_actionable());
        assert!(suggestions[0].reason_zh.contains("\n\n合并顺序："));
        assert!(suggestions[0].reason_en.contains("\n\nMerge order: "));
    }

    #[test]
    fn manual_suggestion_reports_every_executable_change() {
        let suggestion = MergeAiSuggestion {
            target: MergeLineActionTarget::Conflict(0),
            choice: MergeAiChoice::Manual,
            reason_zh: "组合冲突并同步派生值".to_owned(),
            reason_en: "Combine the conflict and update the derived value".to_owned(),
            manual_result: Some("const pipeline = ['left', 'right']".to_owned()),
            middle_edits: vec![MergeAiMiddleEdit {
                expected_text: "const count = 1".to_owned(),
                replacement_text: "const count = 2".to_owned(),
            }],
        };

        assert_eq!(suggestion.change_count(), 2);
        assert!(!merge_ai_needs_completeness_repair(&[suggestion.clone()]));

        let target_only = MergeAiSuggestion {
            middle_edits: Vec::new(),
            ..suggestion
        };
        assert!(merge_ai_needs_completeness_repair(&[target_only]));
    }

    #[test]
    fn manual_guard_corrects_mechanically_proven_array_count() {
        let base = "export const EXPECTED_COUNT = 4\nexport const PIPELINE = ['a', 'c', 'e', 'f']\nif (PIPELINE.length !== EXPECTED_COUNT) throw new Error()\n";
        let local = "export const EXPECTED_COUNT = 4\nexport const PIPELINE = ['a', 'b', 'c', 'e', 'f']\nif (PIPELINE.length !== EXPECTED_COUNT) throw new Error()\n";
        let remote = "export const EXPECTED_COUNT = 4\nexport const PIPELINE = ['a', 'c', 'd', 'e', 'f']\nif (PIPELINE.length !== EXPECTED_COUNT) throw new Error()\n";
        let document = three_way_merge(base, local, remote);
        let target = MergeLineActionTarget::Conflict(0);
        let guarded = guard_merge_ai_suggestions(
            &document,
            vec![MergeAiSuggestion {
                target,
                choice: MergeAiChoice::Manual,
                reason_zh: "合并两边".to_owned(),
                reason_en: "Merge both sides".to_owned(),
                manual_result: Some(
                    "export const PIPELINE = ['a', 'b', 'c', 'd', 'e', 'f']".to_owned(),
                ),
                middle_edits: vec![MergeAiMiddleEdit {
                    expected_text: "export const EXPECTED_COUNT = 4".to_owned(),
                    replacement_text: "export const EXPECTED_COUNT = 5".to_owned(),
                }],
            }],
        );

        assert_eq!(guarded.len(), 1);
        assert_eq!(guarded[0].middle_edits.len(), 1);
        assert_eq!(
            guarded[0].middle_edits[0].replacement_text,
            "export const EXPECTED_COUNT = 6"
        );
        assert!(guarded[0].reason_zh.contains("合并后的 6 项"));
        assert!(guarded[0].reason_en.contains("merged count of 6"));
        assert_eq!(
            explicit_array_assignment("const values = ['a,b', nested(1, 2), { value: 3 }]")
                .map(|(_, count)| count),
            Some(3)
        );
    }

    #[test]
    fn completeness_repair_requires_reasons_and_payload_to_agree() {
        let prompt =
            merge_ai_validation_repair_prompt("original context", "previous payload", true, true);

        assert!(prompt.contains("original context"));
        assert!(prompt.contains("previous payload"));
        assert!(prompt.contains("must never describe a concrete required code change"));
        assert!(prompt.contains("encode every exact non-target change in middle_edits"));
        assert!(prompt.contains("Recount every explicit array element"));
        assert!(prompt.contains("six listed elements require a count of 6, never 5"));
        assert!(prompt.contains("exactly one suggestion for each target"));
        assert!(prompt.contains("no suggestion for any other target"));
    }

    #[test]
    fn middle_edit_connector_uses_the_unique_visible_middle_line() {
        let mut geometry = MergePanelGeometry::default();
        geometry.record_row(
            0,
            Rect::from_min_max(Pos2::new(100.0, 10.0), Pos2::new(500.0, 30.0)),
        );
        geometry.record_row(
            1,
            Rect::from_min_max(Pos2::new(100.0, 30.0), Pos2::new(500.0, 50.0)),
        );
        let lines = vec![
            "export const EXPECTED_COUNT = 4".to_owned(),
            "export const pipeline = []".to_owned(),
        ];
        let target = MergeLineActionTarget::Conflict(0);
        let suggestions = HashMap::from([(
            target,
            MergeAiSuggestion {
                target,
                choice: MergeAiChoice::Manual,
                reason_zh: "同步数量".to_owned(),
                reason_en: "Synchronize the count".to_owned(),
                manual_result: Some("pipeline".to_owned()),
                middle_edits: vec![MergeAiMiddleEdit {
                    expected_text: "EXPECTED_COUNT = 4".to_owned(),
                    replacement_text: "EXPECTED_COUNT = 6".to_owned(),
                }],
            },
        )]);
        let middle_edit_rows = merge_ai_middle_edit_row_cache(&suggestions, &lines);

        assert_eq!(
            merge_ai_middle_edit_anchor(&geometry, &middle_edit_rows, "EXPECTED_COUNT = 4"),
            Some(Pos2::new(100.0, 20.0))
        );
        let duplicate_lines = vec!["same anchor".to_owned(), "same anchor".to_owned()];
        let duplicate_suggestion = MergeAiSuggestion {
            target,
            choice: MergeAiChoice::Manual,
            reason_zh: String::new(),
            reason_en: String::new(),
            manual_result: Some("pipeline".to_owned()),
            middle_edits: vec![MergeAiMiddleEdit {
                expected_text: "same".to_owned(),
                replacement_text: "replacement".to_owned(),
            }],
        };
        let duplicate_rows = merge_ai_middle_edit_row_cache(
            &HashMap::from([(target, duplicate_suggestion)]),
            &duplicate_lines,
        );
        assert_eq!(
            merge_ai_middle_edit_anchor(&geometry, &duplicate_rows, "same"),
            None
        );
        assert_eq!(merge_ai_code_preview("first\nsecond", 40), "first ↵ second");
        assert_eq!(merge_ai_code_preview("abcdef", 3), "abc…");

        assert_eq!(
            merge_ai_pending_middle_edit_rows(&suggestions, &middle_edit_rows),
            HashSet::from([0])
        );
    }

    #[test]
    fn pending_ai_middle_edit_rows_use_accent_highlight_and_marker() {
        let source = include_str!("merge_tool.rs");
        let row = source
            .split("fn merge_editable_result_row")
            .nth(1)
            .and_then(|tail| tail.split("fn paint_result_side_status_badges").next())
            .expect("editable Middle row implementation");

        assert!(row.contains("ai_suggested_edit"));
        assert!(row.contains("color_with_opacity(palette.accent, 0.14)"));
        assert!(row.contains("rect.left() + 3.0"));
        assert!(row.contains("palette.accent"));
    }

    #[test]
    fn ai_suggestion_keeps_both_languages_for_runtime_switching() {
        let suggestion = MergeAiSuggestion {
            target: MergeLineActionTarget::Conflict(0),
            choice: MergeAiChoice::Manual,
            reason_zh: "在中间手动组合左右两边的校验".to_owned(),
            reason_en: "Combine validation from Left and Right in the Middle".to_owned(),
            manual_result: None,
            middle_edits: Vec::new(),
        };

        assert_eq!(
            suggestion.reason(MergeLanguage::Chinese),
            "在中间手动组合左右两边的校验"
        );
        assert_eq!(
            suggestion.reason(MergeLanguage::English),
            "Combine validation from Left and Right in the Middle"
        );
    }

    #[test]
    fn ai_card_uses_shadow_without_border_and_connects_to_action_buttons() {
        let source = include_str!("merge_tool.rs");
        let overlays = source
            .split("fn merge_ai_suggestion_overlays")
            .nth(1)
            .and_then(|tail| tail.split("fn merge_ai_action_anchor").next())
            .expect("AI overlay implementation");

        assert!(overlays.contains(".stroke(egui::Stroke::NONE)"));
        assert!(overlays.contains(".shadow(palette.shadow)"));
        assert!(overlays.contains("paint_merge_ai_suggestion_connector"));
        assert!(overlays.contains("suggestion.reason(language)"));
        assert_eq!(overlays.matches("egui::Area::new").count(), 1);
        assert!(overlays.contains("MergeAiCardPlacement::Middle"));
        assert!(overlays.contains("vec![local, remote]"));
        assert!(overlays.contains("MergeAiCardPlacement::Side(MergeSide::Local)"));
        assert!(overlays.contains("MergeAiCardPlacement::Side(MergeSide::Remote)"));
    }

    #[test]
    fn ai_connector_targets_nearest_card_edge_after_dragging() {
        let card = Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(300.0, 220.0));
        assert_eq!(
            closest_rect_edge_point(card, Pos2::new(70.0, 150.0)),
            Pos2::new(100.0, 150.0)
        );
        assert_eq!(
            closest_rect_edge_point(card, Pos2::new(340.0, 180.0)),
            Pos2::new(300.0, 180.0)
        );
        assert_eq!(
            closest_rect_edge_point(card, Pos2::new(180.0, 70.0)),
            Pos2::new(180.0, 100.0)
        );
    }

    #[test]
    fn ai_card_can_move_to_any_visible_window_edge() {
        let viewport = Rect::from_min_max(Pos2::ZERO, Pos2::new(1_200.0, 800.0));
        let anchor = Pos2::new(500.0, 420.0);
        let allowed = merge_ai_overlay_allowed_offset(viewport, anchor, Vec2::new(252.0, 120.0));

        assert!(
            allowed.min.y < -400.0,
            "the card can move well above its row"
        );
        assert!(
            allowed.max.y > 200.0,
            "the card can also move below its row"
        );
        assert_eq!(allowed.min, Pos2::new(-492.0, -412.0));
        assert_eq!(allowed.max, Pos2::new(440.0, 252.0));
    }

    #[test]
    fn merge_requests_force_advisory_tool_calls_and_disable_claude_thinking() {
        let source = include_str!("merge_tool.rs");
        let request = source
            .split("fn request_merge_ai_suggestions")
            .nth(1)
            .and_then(|tail| tail.split("fn merge_ai_url_for_log").next())
            .expect("AI request implementation");

        assert!(request.contains("\"thinking\": { \"type\": \"disabled\" }"));
        assert!(request.contains("\"tools\""));
        assert!(request.contains("MERGE_AI_TOOL_NAME"));
        assert!(request.contains("\"tool_choice\""));
        assert!(!request.contains("\"response_format\""));
        assert!(request.contains("response.structure"));
    }

    #[test]
    fn ai_suggestion_json_accepts_current_deletion_targets() {
        let target = MergeLineActionTarget::BaseOnlyGroup(17);
        let suggestions = parse_merge_ai_suggestions(
            r#"{"suggestions":[
                {"target_type":"deletion","target_index":17,"choice":"local","reason":"本地分支有意删除旧逻辑"},
                {"target_type":"deletion","target_index":18,"choice":"remote","reason":"not current"}
            ]}"#,
            &HashSet::from([target]),
            MergeLanguage::Chinese,
        )
        .unwrap();

        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].target, target);
        assert_eq!(
            suggestions[0].reason(MergeLanguage::Chinese),
            "本地分支有意删除旧逻辑"
        );
        assert_eq!(
            suggestions[0].reason(MergeLanguage::English),
            "本地分支有意删除旧逻辑"
        );
    }

    #[test]
    fn merge_code_search_is_case_insensitive_and_wraps_navigation() {
        let lines = ["import Widget", "const value = 1", "render(widget)"];
        assert_eq!(merge_search_matches(lines, "WIDGET"), vec![0, 2]);
        assert_eq!(merge_next_search_index(1, 2, NavDirection::Next), 0);
        assert_eq!(merge_next_search_index(0, 2, NavDirection::Previous), 1);
        assert_eq!(merge_next_search_index(0, 0, NavDirection::Next), 0);
    }

    #[test]
    fn merge_search_jump_centers_and_clamps_the_selected_result_row() {
        let row_y = 50.0 * MERGE_CODE_ROW_HEIGHT;
        assert_eq!(
            merge_search_scroll_target(row_y, 200.0, 2_000.0),
            row_y + MERGE_CODE_ROW_HEIGHT * 0.5 - 100.0
        );
        assert_eq!(merge_search_scroll_target(0.0, 200.0, 2_000.0), 0.0);
        assert_eq!(merge_search_scroll_target(1_990.0, 200.0, 2_000.0), 1_800.0);
        assert_eq!(merge_search_scroll_target(400.0, 800.0, 600.0), 0.0);
    }

    #[test]
    fn merge_search_jump_is_applied_by_outer_shared_scroll_owner() {
        let source = include_str!("merge_tool.rs");
        let columns = source
            .split("fn merge_editor_columns")
            .nth(1)
            .and_then(|tail| tail.split("fn merge_horizontal_scroll_input").next())
            .expect("merge editor columns implementation");
        assert!(columns.contains("let search_result_y = result_output"));
        assert!(columns.contains(".or(remote_output.search_result_y)"));
        assert!(columns.contains(".or(local_output.search_result_y)"));
        assert!(columns.contains("merge_search_scroll_target("));

        for panel_name in ["fn merge_side_panel", "fn merge_result_panel"] {
            let panel = source
                .split(panel_name)
                .nth(1)
                .and_then(|tail| tail.split("fn ").next())
                .expect("merge panel implementation");
            assert!(panel.contains("search_result_y = Some("));
            assert!(!panel.contains("app.shared_scroll_y ="));
        }
    }

    #[test]
    fn ctrl_f_routes_search_to_the_hovered_merge_pane() {
        for pane in [
            MergeSearchPane::Left,
            MergeSearchPane::Middle,
            MergeSearchPane::Right,
        ] {
            let mut app = MergeToolApp::new(test_merge_args(), navigation_matrix_document());
            app.hovered_search_pane = Some(pane);
            app.collapse_unchanged = true;
            let modifiers = egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            };
            let ctx = egui::Context::default();
            ctx.begin_pass(egui::RawInput {
                modifiers,
                events: vec![egui::Event::Key {
                    key: egui::Key::F,
                    physical_key: Some(egui::Key::F),
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                ..Default::default()
            });

            app.handle_keyboard_shortcuts(&ctx);

            assert!(app.search.open);
            assert_eq!(app.search.pane, pane);
            assert!(app.search.request_focus);
            assert!(!app.collapse_unchanged);
            let _ = ctx.end_pass();
        }

        let implementation = include_str!("merge_tool.rs");
        assert!(implementation.contains("small_button(\"↑\")"));
        assert!(implementation.contains("small_button(\"↓\")"));
    }

    #[test]
    fn ai_prompt_includes_three_way_conflicts_history_and_related_files() {
        let document = three_way_merge("value = base\n", "value = local\n", "value = remote\n");
        let prompt = merge_ai_prompt(
            &test_merge_args(),
            &MergeSourceText {
                base: "value = base\n".to_owned(),
                local: "value = local\n".to_owned(),
                remote: "value = remote\n".to_owned(),
            },
            &document,
            &MergeAiContext {
                history: "abc123 change validation".to_owned(),
                repository_state: "M src/plugin.ts".to_owned(),
                related_files: "--- related file: settings.toml ---\nmode = strict\n".to_owned(),
                symbol_references: "src/app.ts:4:autoAnimationPlugin()".to_owned(),
            },
        );

        assert!(prompt.contains("CONFLICTS:"));
        assert!(prompt.contains("Simplified Chinese"));
        assert!(prompt.contains("reason_en"));
        assert!(prompt.contains("exactly one or two short sentences"));
        assert!(prompt.contains("Do not repeat code, merge order, or the full analysis"));
        assert!(merge_ai_system_prompt().contains("one or two short sentences"));
        assert!(merge_ai_system_prompt().contains("Do not expose the full reasoning chain"));
        assert!(prompt.contains("merge_order_zh"));
        assert!(prompt.contains("exact before/after or precedence order"));
        assert!(prompt.contains("analyze whether conditions overlap"));
        assert!(prompt.contains("which branch wins when both match"));
        assert!(prompt.contains("Never respond only that both sides should be kept"));
        assert!(prompt.contains("adds, removes, renames, or reorders array elements"));
        assert!(prompt.contains("object properties"));
        assert!(prompt.contains("method or function parameters and arguments"));
        assert!(prompt.contains("enum members"));
        assert!(prompt.contains("ordered operations"));
        assert!(prompt.contains("manual_result_provided=true"));
        assert!(prompt.contains("middle_edits"));
        assert!(prompt.contains("non-diff lines"));
        assert!(prompt.contains("occur exactly once inside one logical Middle line"));
        assert!(prompt.contains("左边/中间/右边"));
        assert!(prompt.contains("Left/Middle/Right"));
        assert!(!prompt.contains("LOCAL FILE EXCERPT"));
        assert!(!prompt.contains("REMOTE FILE EXCERPT"));
        assert!(prompt.contains("value = local"));
        assert!(prompt.contains("RESOLVED MIDDLE RESULT"));
        assert!(prompt.contains("TARGET-LOCAL NEIGHBORHOODS"));
        assert!(prompt.contains("unresolved target's provisional editor row"));
        assert!(prompt.contains("abc123 change validation"));
        assert!(prompt.contains("CURRENT REPOSITORY MERGE STATE"));
        assert!(prompt.contains("M src/plugin.ts"));
        assert!(prompt.contains("SYMBOL REFERENCES IN CURRENT WORKTREE"));
        assert!(prompt.contains("autoAnimationPlugin"));
        assert!(prompt.contains("related file: settings.toml"));
        assert!(prompt.contains("submit_merge_suggestions"));
        assert!(prompt.contains("advisory only"));
    }

    fn moved_vue_event_binding_fixture() -> (MergeSourceText, MergeDocument) {
        let filler = (0..1_400)
            .map(|index| format!("// stable filler {index:04}"))
            .collect::<Vec<_>>()
            .join("\n");
        let base = format!(
            "<script setup>\nfunction openDropdown() {{}}\n</script>\n{filler}\n<template>\n  <div class=\"p-select-trigger\">\n    <a-input\n      readonly\n      @click=\"openDropdown\"\n    />\n    <div\n      class=\"p-select-tags-wrapper\"\n      @click=\"openDropdown\"\n    />\n  </div>\n</template>\n"
        );
        let left = format!(
            "<script setup>\nfunction openDropdown() {{}}\n</script>\n{filler}\n<template>\n  <div\n    class=\"p-select-trigger\"\n    @click=\"openDropdown\"\n  >\n    <a-input\n      readonly\n    />\n    <div\n      class=\"p-select-tags-wrapper\"\n    />\n  </div>\n</template>\n"
        );
        let remote = base.clone();
        let sources = MergeSourceText {
            base: base.clone(),
            local: left.clone(),
            remote: remote.clone(),
        };
        (sources, three_way_merge(&base, &left, &remote))
    }

    #[test]
    fn ai_prompt_uses_target_local_context_for_large_file_event_relocation() {
        let (sources, document) = moved_vue_event_binding_fixture();
        let deletions = base_only_display_groups(&document);
        assert_eq!(deletions.len(), 2);
        assert!(
            sources.base.chars().count() > 20 * 1024,
            "fixture must place the template beyond the old prefix-only context"
        );

        let middle = merge_ai_editable_middle_text(&document);
        assert_eq!(middle.matches("@click=\"openDropdown\"").count(), 1);
        let prompt = merge_ai_prompt(
            &test_merge_args(),
            &sources,
            &document,
            &MergeAiContext::default(),
        );

        assert!(prompt.contains("TARGET-LOCAL NEIGHBORHOODS"));
        assert!(prompt.contains("@click=\"openDropdown\""));
        assert!(prompt.contains("TARGET ABSENT IN THIS PANE"));
        assert!(prompt.contains("UNRESOLVED TARGET OMITTED"));
        assert!(prompt.contains("class=\"p-select-trigger\""));
        assert!(prompt.contains("CROSS-TARGET RELATIONSHIP EVIDENCE"));
        assert!(prompt.contains("possible relocation or consolidation"));
        assert!(prompt.contains("broader or shared execution point"));
    }

    #[test]
    fn ai_context_identifier_scan_keeps_specific_import_names() {
        let document = three_way_merge(
            "import { autoAnimationPlugin } from './animation';\n",
            "\n",
            "import { autoAnimationPlugin } from './new-animation';\n",
        );
        let identifiers = merge_ai_candidate_identifiers(&document);
        assert!(identifiers.contains(&"autoAnimationPlugin".to_owned()));
        assert!(!identifiers.contains(&"import".to_owned()));
        assert_eq!(
            merge_ai_reference_paths(
                "src/plugin.ts:8:autoAnimationPlugin()\nsrc/app.ts:12:autoAnimationPlugin\n"
            ),
            vec!["src/plugin.ts".to_owned(), "src/app.ts".to_owned()]
        );
    }

    #[test]
    fn ai_context_reads_real_merge_tips_history_state_and_symbol_files() {
        let test_id = MERGE_AI_CONTEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        let repo = env::temp_dir().join(format!(
            "git-agent-merge-ai-context-{}-{test_id}",
            std::process::id()
        ));
        if repo.exists() {
            fs::remove_dir_all(&repo).unwrap();
        }
        fs::create_dir_all(repo.join("src")).unwrap();
        run_context_test_git(&repo, &["init"]);
        run_context_test_git(&repo, &["config", "user.name", "Git Agent Test"]);
        run_context_test_git(&repo, &["config", "user.email", "git-agent@example.test"]);

        let output = repo.join("src/plugin.ts");
        let registry = repo.join("src/registry.ts");
        let base = "import { autoAnimationPlugin } from './old-animation';\n// stable 01\n// stable 02\n// stable 03\n// stable 04\n// stable 05\n// stable 06\n// stable 07\n// stable 08\nexport const plugins = [autoAnimationPlugin];\nexport const stable = true;\n";
        let left = "// stable 01\n// stable 02\n// stable 03\n// stable 04\n// stable 05\n// stable 06\n// stable 07\n// stable 08\nexport const plugins = [];\nexport const stable = true;\n";
        let right = "import { autoAnimationPlugin } from './new-animation';\n// stable 01\n// stable 02\n// stable 03\n// stable 04\n// stable 05\n// stable 06\n// stable 07\n// stable 08\nexport const plugins = [];\nexport const stable = true;\n";
        fs::write(&output, base).unwrap();
        fs::write(
            &registry,
            "export const historicalPluginName = 'autoAnimationPlugin';\n",
        )
        .unwrap();
        run_context_test_git(&repo, &["add", "."]);
        run_context_test_git(&repo, &["commit", "-m", "base animation setup"]);
        let left_branch = run_context_test_git(&repo, &["branch", "--show-current"]);

        run_context_test_git(&repo, &["checkout", "-b", "right"]);
        fs::write(&output, right).unwrap();
        run_context_test_git(&repo, &["add", "."]);
        run_context_test_git(&repo, &["commit", "-m", "right: migrate animation import"]);

        run_context_test_git(&repo, &["checkout", &left_branch]);
        fs::write(&output, left).unwrap();
        run_context_test_git(&repo, &["add", "."]);
        run_context_test_git(
            &repo,
            &["commit", "-m", "left: remove animation plugin usage"],
        );
        let merge_output = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["merge", "right"])
            .output()
            .unwrap();
        assert!(
            !merge_output.status.success(),
            "fixture must produce a conflict"
        );

        let document = three_way_merge(base, left, right);
        assert_eq!(document.conflicts().len(), 1);
        let context = collect_merge_ai_context(
            &MergeArgs {
                output: output.clone(),
                repo_root: Some(repo.clone()),
                ..test_merge_args()
            },
            &MergeSourceText {
                base: base.to_owned(),
                local: left.to_owned(),
                remote: right.to_owned(),
            },
            &document,
        )
        .unwrap();

        assert!(context.history.contains("LEFT HEAD"));
        assert!(context.history.contains("RIGHT MERGE_HEAD"));
        assert!(
            context
                .history
                .contains("left: remove animation plugin usage")
        );
        assert!(context.history.contains("right: migrate animation import"));
        assert!(context.repository_state.contains("MERGE STATUS"));
        assert!(context.repository_state.contains("src/plugin.ts"));
        assert!(context.symbol_references.contains("autoAnimationPlugin"));
        assert!(context.symbol_references.contains("src/registry.ts"));
        assert!(
            context
                .related_files
                .contains("related file: src/registry.ts")
        );

        let guarded = guard_merge_ai_suggestions(
            &document,
            vec![MergeAiSuggestion {
                target: MergeLineActionTarget::Conflict(0),
                choice: MergeAiChoice::Remote,
                reason_zh: "右边仍有导入".to_owned(),
                reason_en: "The Right import remains".to_owned(),
                manual_result: None,
                middle_edits: Vec::new(),
            }],
        );
        assert_eq!(guarded[0].choice, MergeAiChoice::Local);

        run_context_test_git(&repo, &["merge", "--abort"]);
        fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn ai_guard_rejects_preserving_an_import_removed_with_all_usages() {
        let document = three_way_merge(
            "import { autoAnimationPlugin } from './old-animation';\nconst plugins = [autoAnimationPlugin];\n",
            "\n",
            "import { autoAnimationPlugin } from './new-animation';\n",
        );
        assert_eq!(document.conflicts().len(), 1);
        let target = MergeLineActionTarget::Conflict(0);
        let guarded = guard_merge_ai_suggestions(
            &document,
            vec![MergeAiSuggestion {
                target,
                choice: MergeAiChoice::Remote,
                reason_zh: "右边仍有导入".to_owned(),
                reason_en: "The Right import remains".to_owned(),
                manual_result: None,
                middle_edits: Vec::new(),
            }],
        );

        assert_eq!(guarded[0].choice, MergeAiChoice::Local);
        assert!(
            guarded[0]
                .reason_zh
                .contains("没有发现 autoAnimationPlugin 的实际使用")
        );
        assert!(guarded[0].reason_en.contains("no remaining usage"));
    }

    #[test]
    fn ai_guard_rejects_preserving_a_base_only_import_without_remaining_usage() {
        let document = three_way_merge(
            "import { legacyDiscount } from './legacyDiscount';\nimport { modernPrice } from './modernPrice';\n\nexport function quote(total: number) {\n  return modernPrice(total) + legacyDiscount(total);\n}\n",
            "import { modernPrice } from './modernPrice';\n\nexport function quote(total: number) {\n  return modernPrice(total);\n}\n",
            "import { legacyDiscount } from './legacyDiscount';\nimport { modernPrice } from './modernPrice';\n\nexport function quote(total: number) {\n  return modernPrice(Math.max(total, 0));\n}\n",
        );
        let group = base_only_display_groups(&document)
            .into_iter()
            .find(|group| {
                document.lines[group.line_index..group.line_index + group.line_count]
                    .iter()
                    .any(|line| {
                        line.base
                            .as_deref()
                            .is_some_and(|line| line.contains("legacyDiscount"))
                    })
            })
            .expect("stale import should be represented as a deletion target");
        assert_eq!(group.missing_side, MergeSide::Local);

        let guarded = guard_merge_ai_suggestions(
            &document,
            vec![MergeAiSuggestion {
                target: MergeLineActionTarget::BaseOnlyGroup(group.line_index),
                choice: MergeAiChoice::Remote,
                reason_zh: "右边仍有导入".to_owned(),
                reason_en: "The Right import remains".to_owned(),
                manual_result: None,
                middle_edits: Vec::new(),
            }],
        );

        assert_eq!(guarded[0].choice, MergeAiChoice::Local);
        assert!(
            guarded[0]
                .reason_zh
                .contains("没有发现 legacyDiscount 的实际使用")
        );
        assert!(guarded[0].reason_en.contains("unused import"));
    }

    #[test]
    fn ai_guard_keeps_an_import_when_the_merged_file_still_uses_it() {
        let document = three_way_merge(
            "import { autoAnimationPlugin } from './old-animation';\nconst plugins = [autoAnimationPlugin];\n",
            "const plugins = [autoAnimationPlugin];\n",
            "import { autoAnimationPlugin } from './new-animation';\nconst plugins = [autoAnimationPlugin];\n",
        );
        assert_eq!(document.conflicts().len(), 1);
        let target = MergeLineActionTarget::Conflict(0);
        let guarded = guard_merge_ai_suggestions(
            &document,
            vec![MergeAiSuggestion {
                target,
                choice: MergeAiChoice::Remote,
                reason_zh: "中间仍使用该符号".to_owned(),
                reason_en: "Middle still uses the symbol".to_owned(),
                manual_result: None,
                middle_edits: Vec::new(),
            }],
        );

        assert_eq!(guarded[0].choice, MergeAiChoice::Remote);
        assert_eq!(guarded[0].reason_en, "Middle still uses the symbol");
    }

    #[test]
    fn ai_guard_uses_manual_when_unused_import_shares_a_conflict_with_other_edits() {
        let document = three_way_merge(
            "import { autoAnimationPlugin } from './old-animation';\nexport const mode = 'base';\nconst plugins = [autoAnimationPlugin];\n",
            "export const mode = 'left';\n",
            "import { autoAnimationPlugin } from './new-animation';\nexport const mode = 'right';\n",
        );
        assert_eq!(document.conflicts().len(), 1);
        let target = MergeLineActionTarget::Conflict(0);
        let guarded = guard_merge_ai_suggestions(
            &document,
            vec![MergeAiSuggestion {
                target,
                choice: MergeAiChoice::Remote,
                reason_zh: "采用右边".to_owned(),
                reason_en: "Use Right".to_owned(),
                manual_result: None,
                middle_edits: Vec::new(),
            }],
        );

        assert_eq!(guarded[0].choice, MergeAiChoice::Manual);
        assert!(guarded[0].reason_zh.contains("还有其他改动"));
        assert!(
            guarded[0]
                .reason_en
                .contains("switching the whole conflict is unsafe")
        );
    }

    #[test]
    fn accepting_ai_suggestion_updates_only_the_suggested_conflict() {
        let document = navigation_matrix_document();
        let mut app = MergeToolApp::new(test_merge_args(), document);
        let target = MergeLineActionTarget::Conflict(0);
        app.ai_suggestions.insert(
            target,
            MergeAiSuggestion {
                target,
                choice: MergeAiChoice::Local,
                reason_zh: "左边行为是有意保留的".to_owned(),
                reason_en: "The Left behavior is intentional".to_owned(),
                manual_result: None,
                middle_edits: Vec::new(),
            },
        );

        app.apply_ai_suggestion(target);

        assert!(!app.ai_suggestions.contains_key(&target));
        assert!(app.result_text.contains("conflict-a = local"));
        assert!(app.document.conflict_side_resolved(0, MergeSide::Local));
        assert!(app.document.conflict_side_resolved(0, MergeSide::Remote));
        assert_eq!(app.undo_stack.len(), 1);
        assert_eq!(app.unresolved_conflict_count(), 2);

        assert!(app.undo());
        assert!(!app.document.conflict_side_resolved(0, MergeSide::Local));
        assert!(!app.document.conflict_side_resolved(0, MergeSide::Remote));
        assert_eq!(
            app.ai_suggestions
                .get(&target)
                .map(|suggestion| suggestion.reason_zh.as_str()),
            Some("左边行为是有意保留的")
        );

        assert!(app.redo());
        assert!(app.document.conflict_side_resolved(0, MergeSide::Local));
        assert!(app.document.conflict_side_resolved(0, MergeSide::Remote));
        assert!(!app.ai_suggestions.contains_key(&target));
    }

    fn test_ai_suggestion(
        target: MergeLineActionTarget,
        choice: MergeAiChoice,
    ) -> MergeAiSuggestion {
        MergeAiSuggestion {
            target,
            choice,
            reason_zh: "测试建议".to_owned(),
            reason_en: "Test suggestion".to_owned(),
            manual_result: None,
            middle_edits: Vec::new(),
        }
    }

    #[test]
    fn apply_all_ai_suggestions_is_one_edit_including_deletions_and_shared_middle_edits() {
        let mut app = MergeToolApp::new(test_merge_args(), navigation_matrix_document());
        let shared_edit = MergeAiMiddleEdit {
            expected_text: "stable-a".to_owned(),
            replacement_text: "stable-a-updated".to_owned(),
        };
        for (index, choice) in [
            MergeAiChoice::Local,
            MergeAiChoice::Remote,
            MergeAiChoice::Manual,
        ]
        .into_iter()
        .enumerate()
        {
            let target = MergeLineActionTarget::Conflict(index);
            let mut suggestion = test_ai_suggestion(target, choice);
            if index == 2 {
                suggestion.manual_result = Some("conflict-c = combined".to_owned());
            }
            suggestion.middle_edits.push(shared_edit.clone());
            app.ai_suggestions.insert(target, suggestion);
        }
        for group in base_only_display_groups(&app.document) {
            let target = MergeLineActionTarget::BaseOnlyGroup(group.line_index);
            let choice = match group.missing_side {
                MergeSide::Local => MergeAiChoice::Local,
                MergeSide::Remote => MergeAiChoice::Remote,
            };
            app.ai_suggestions
                .insert(target, test_ai_suggestion(target, choice));
        }
        let before = app.snapshot();
        let epoch = app.display_epoch;
        let highlight_revision = app.result_highlight_revision;
        app.ai_analysis_error = Some("old error".to_owned());

        app.apply_all_ai_suggestions();

        assert!(app.ai_suggestions.is_empty());
        assert!(app.ai_analysis_error.is_none());
        assert_eq!(app.unresolved_conflict_count(), 0);
        for text in [
            "conflict-a = local",
            "conflict-b = remote",
            "conflict-c = combined",
            "stable-a-updated",
        ] {
            assert!(app.result_text.contains(text), "{text}");
        }
        assert!(!app.result_text.contains("stable-a-updated-updated"));
        assert!(!app.result_text.contains("delete-from-"));
        assert_eq!(app.undo_stack.len(), 1);
        assert_eq!(app.display_epoch, epoch + 1);
        assert_eq!(app.result_highlight_revision, highlight_revision + 1);
        assert!(app.write_task.is_none());
        let after = app.snapshot();
        assert!(app.undo());
        assert_eq!(app.snapshot(), before);
        assert!(app.redo());
        assert_eq!(app.snapshot(), after);
    }

    #[test]
    fn apply_all_ai_suggestions_keeps_manual_only_advice_and_empty_batches_are_noops() {
        let mut app = MergeToolApp::new(test_merge_args(), navigation_matrix_document());
        let manual = MergeLineActionTarget::Conflict(1);
        app.ai_suggestions
            .insert(manual, test_ai_suggestion(manual, MergeAiChoice::Manual));
        assert_eq!(app.actionable_ai_suggestion_count(), 0);
        let before = app.snapshot();
        app.apply_all_ai_suggestions();
        assert_eq!(app.snapshot(), before);
        assert!(app.undo_stack.is_empty());

        let local = MergeLineActionTarget::Conflict(0);
        app.ai_suggestions
            .insert(local, test_ai_suggestion(local, MergeAiChoice::Local));
        assert_eq!(app.actionable_ai_suggestion_count(), 1);
        app.apply_all_ai_suggestions();
        assert_eq!(app.ai_suggestions.len(), 1);
        assert!(app.ai_suggestions.contains_key(&manual));
        assert_eq!(app.unresolved_conflict_count(), 2);
    }

    #[test]
    fn apply_all_ai_suggestions_rejects_stale_or_overlapping_edits_without_partial_changes() {
        for failure in [
            "missing_middle",
            "overlapping_middle",
            "missing_target",
            "resolved_target",
        ] {
            let mut app = MergeToolApp::new(test_merge_args(), navigation_matrix_document());
            for index in 0..2 {
                let target = MergeLineActionTarget::Conflict(index);
                let mut suggestion = test_ai_suggestion(target, MergeAiChoice::Local);
                suggestion.middle_edits.push(MergeAiMiddleEdit {
                    expected_text: if index == 0 { "stable-a" } else { "stable-b" }.to_owned(),
                    replacement_text: "updated".to_owned(),
                });
                app.ai_suggestions.insert(target, suggestion);
            }
            match failure {
                "missing_middle" => {
                    app.ai_suggestions
                        .get_mut(&MergeLineActionTarget::Conflict(1))
                        .unwrap()
                        .middle_edits[0]
                        .expected_text = "missing anchor".to_owned();
                }
                "overlapping_middle" => {
                    app.ai_suggestions
                        .get_mut(&MergeLineActionTarget::Conflict(1))
                        .unwrap()
                        .middle_edits[0]
                        .expected_text = "stable-a".to_owned();
                    app.ai_suggestions
                        .get_mut(&MergeLineActionTarget::Conflict(1))
                        .unwrap()
                        .middle_edits[0]
                        .replacement_text = "different replacement".to_owned();
                }
                "missing_target" => {
                    let target = MergeLineActionTarget::Conflict(99);
                    app.ai_suggestions
                        .insert(target, test_ai_suggestion(target, MergeAiChoice::Remote));
                }
                "resolved_target" => app.document.apply_conflict(1, MergeSide::Remote),
                _ => unreachable!(),
            }
            let before = app.snapshot();
            app.apply_all_ai_suggestions();
            assert_eq!(app.snapshot(), before, "{failure}");
            assert!(app.ai_analysis_error.is_some(), "{failure}");
            assert!(app.undo_stack.is_empty(), "{failure}");
        }
    }

    #[test]
    fn ai_middle_batch_edits_use_original_offsets_on_the_same_line() {
        let mut document = three_way_merge("alpha beta\n", "alpha beta\n", "alpha beta\n");
        let edits = [
            MergeAiMiddleEdit {
                expected_text: "alpha".to_owned(),
                replacement_text: "longer-alpha".to_owned(),
            },
            MergeAiMiddleEdit {
                expected_text: "beta".to_owned(),
                replacement_text: "B".to_owned(),
            },
        ];
        document.apply_ai_middle_edits(&edits).unwrap();
        assert_eq!(document.result_text(), "longer-alpha B\n");
    }

    #[test]
    fn apply_one_then_all_ai_suggestions_does_not_repeat_shared_middle_edits() {
        let mut app = MergeToolApp::new(test_merge_args(), navigation_matrix_document());
        for index in 0..2 {
            let target = MergeLineActionTarget::Conflict(index);
            let mut suggestion = test_ai_suggestion(target, MergeAiChoice::Local);
            suggestion.middle_edits.push(MergeAiMiddleEdit {
                expected_text: "stable-a".to_owned(),
                replacement_text: "stable-a-updated".to_owned(),
            });
            app.ai_suggestions.insert(target, suggestion);
        }
        let before = app.snapshot();
        app.apply_ai_suggestion(MergeLineActionTarget::Conflict(0));
        assert!(
            app.ai_suggestions[&MergeLineActionTarget::Conflict(1)]
                .middle_edits
                .is_empty()
        );
        app.apply_all_ai_suggestions();
        assert!(app.result_text.contains("stable-a-updated"));
        assert!(!app.result_text.contains("stable-a-updated-updated"));
        assert!(app.ai_suggestions.is_empty());
        assert!(app.undo());
        assert!(app.undo());
        assert_eq!(app.snapshot(), before);
    }

    #[test]
    fn single_and_bulk_ai_applications_share_the_busy_and_manual_edit_gate() {
        for blocked_by in ["analysis", "loading", "writing", "manual"] {
            let mut app = MergeToolApp::new(test_merge_args(), navigation_matrix_document());
            let target = MergeLineActionTarget::Conflict(0);
            app.ai_suggestions
                .insert(target, test_ai_suggestion(target, MergeAiChoice::Local));
            match blocked_by {
                "analysis" => app.ai_task = Some(mpsc::channel().1),
                "loading" => app.load_task = Some(mpsc::channel().1),
                "writing" => app.write_task = Some(mpsc::channel().1),
                "manual" => app.manual_result_override = true,
                _ => unreachable!(),
            }
            let before = app.snapshot();
            assert!(!app.ai_suggestion_actions_enabled());
            app.apply_all_ai_suggestions();
            app.apply_ai_suggestion(target);
            assert_eq!(app.snapshot(), before, "{blocked_by}");
            assert!(app.undo_stack.is_empty());
        }
    }

    #[test]
    fn ai_apply_all_entry_is_next_to_analysis_and_localized() {
        let source = include_str!("merge_tool.rs");
        let toolbar = source
            .split("fn merge_toolbar(")
            .nth(1)
            .unwrap()
            .split("fn merge_toolbar_ai_button(")
            .next()
            .unwrap();
        for required in [
            "ai_apply_all",
            "ai_apply_all_hint",
            "actionable_ai_suggestion_count()",
            "app.ai_suggestion_actions_enabled() && count > 0",
            "app.apply_all_ai_suggestions()",
        ] {
            assert!(toolbar.contains(required), "{required}");
        }
        assert!(
            toolbar.find("merge_toolbar_ai_button(").unwrap()
                < toolbar.find("ai_apply_all").unwrap()
        );
        assert_eq!(mt(MergeLanguage::Chinese, "ai_apply_all"), "应用所有建议");
        assert_eq!(
            mt(MergeLanguage::English, "ai_apply_all"),
            "Apply all suggestions"
        );
    }

    #[test]
    fn manual_ai_suggestion_updates_target_and_non_diff_middle_line_without_resolving_others() {
        let document = navigation_matrix_document();
        let mut app = MergeToolApp::new(test_merge_args(), document);
        let target = MergeLineActionTarget::Conflict(0);
        app.ai_suggestions.insert(
            target,
            MergeAiSuggestion {
                target,
                choice: MergeAiChoice::Manual,
                reason_zh: "组合冲突，并同步修改中间已有代码".to_owned(),
                reason_en: "Combine the conflict and update existing Middle code".to_owned(),
                manual_result: Some("conflict-a = combined".to_owned()),
                middle_edits: vec![MergeAiMiddleEdit {
                    expected_text: "stable-a".to_owned(),
                    replacement_text: "stable-a-updated".to_owned(),
                }],
            },
        );

        app.apply_ai_suggestion(target);

        assert!(!app.manual_result_override);
        assert!(!app.ai_suggestions.contains_key(&target));
        assert!(app.result_text.contains("conflict-a = combined"));
        assert!(app.result_text.contains("stable-a-updated"));
        assert_eq!(app.unresolved_conflict_count(), 2);
        assert_eq!(app.undo_stack.len(), 1);

        assert!(app.undo());
        assert!(!app.result_text.contains("conflict-a = combined"));
        assert!(app.result_text.contains("stable-a"));
        assert_eq!(app.unresolved_conflict_count(), 3);
    }

    #[test]
    fn stale_or_ambiguous_middle_edit_rejects_the_entire_ai_application() {
        let document = three_way_merge(
            "same\nvalue = 'base'\nsame\n",
            "same\nvalue = 'left'\nsame\n",
            "same\nvalue = 'right'\nsame\n",
        );
        let mut app = MergeToolApp::new(test_merge_args(), document);
        let target = MergeLineActionTarget::Conflict(0);
        let before = app.document.clone();
        app.ai_suggestions.insert(
            target,
            MergeAiSuggestion {
                target,
                choice: MergeAiChoice::Manual,
                reason_zh: "建议存在歧义锚点".to_owned(),
                reason_en: "The proposed anchor is ambiguous".to_owned(),
                manual_result: Some("value = 'combined'".to_owned()),
                middle_edits: vec![MergeAiMiddleEdit {
                    expected_text: "same".to_owned(),
                    replacement_text: "updated".to_owned(),
                }],
            },
        );

        app.apply_ai_suggestion(target);

        assert_eq!(app.document, before);
        assert!(app.ai_suggestions.contains_key(&target));
        assert!(
            app.ai_analysis_error
                .as_deref()
                .is_some_and(|error| error.contains("found 2"))
        );
        assert!(app.undo_stack.is_empty());
    }

    #[test]
    fn resolved_conflict_side_rows_do_not_leave_change_backgrounds() {
        let mut document = navigation_matrix_document();
        document.apply_conflict(0, MergeSide::Local);
        let palette = merge_palette(MergeTheme::Light);

        for side in [MergeSide::Local, MergeSide::Remote] {
            let rows = cached_merge_side_display_rows(&document, side);
            let conflict_rows = rows
                .iter()
                .filter(|row| row.conflict_index == Some(0))
                .collect::<Vec<_>>();

            assert!(!conflict_rows.is_empty());
            assert!(conflict_rows.iter().all(|row| row.side_resolved));
            assert!(conflict_rows.iter().all(|row| {
                row.tone == MergeSideLineTone::Unchanged
                    && merge_side_row_fill(row, 0, MergeHighlightMode::Lines, palette).is_none()
            }));
        }
    }

    #[test]
    fn ai_deletion_suggestion_resolves_the_base_only_group() {
        let document = navigation_matrix_document();
        let group = base_only_display_groups(&document)
            .into_iter()
            .find(|group| {
                document.lines[group.line_index].base.as_deref() == Some("delete-from-local")
            })
            .expect("local deletion group");
        assert_eq!(group.missing_side, MergeSide::Local);
        let target = MergeLineActionTarget::BaseOnlyGroup(group.line_index);
        let mut app = MergeToolApp::new(test_merge_args(), document);
        app.ai_suggestions.insert(
            target,
            MergeAiSuggestion {
                target,
                choice: MergeAiChoice::Local,
                reason_zh: "左边有意删除了过时行".to_owned(),
                reason_en: "The Left pane intentionally removed the obsolete line".to_owned(),
                manual_result: None,
                middle_edits: Vec::new(),
            },
        );

        app.apply_ai_suggestion(target);

        assert!(!app.ai_suggestions.contains_key(&target));
        assert!(
            !base_only_display_groups(&app.document)
                .iter()
                .any(|candidate| candidate.line_index == group.line_index)
        );
        assert!(!app.result_text.contains("delete-from-local"));
    }

    #[test]
    fn merge_loading_state_returns_immediately_and_releases_after_analysis() {
        let test_id = MERGE_LOAD_TEST_ID.fetch_add(1, Ordering::Relaxed);
        let temp_dir = env::temp_dir().join(format!(
            "git-agent-merge-load-{}-{test_id}",
            std::process::id()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        let base_path = temp_dir.join("base.txt");
        let local_path = temp_dir.join("local.txt");
        let remote_path = temp_dir.join("remote.txt");
        let output_path = temp_dir.join("merged.txt");
        fs::write(&base_path, "base\n").unwrap();
        fs::write(&local_path, "local\n").unwrap();
        fs::write(&remote_path, "remote\n").unwrap();
        let args = MergeArgs {
            base: base_path,
            local: local_path,
            remote: remote_path,
            output: output_path,
            ..test_merge_args()
        };

        let mut app = MergeToolApp::loading(args);
        assert!(app.load_task.is_some());
        assert!(app.sources.is_none());

        let ctx = egui::Context::default();
        for _ in 0..100 {
            app.poll_load_task(&ctx);
            if app.load_task.is_none() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert!(app.load_task.is_none());
        assert!(app.sources.is_some());
        assert_eq!(app.document.unresolved_conflict_count(), 1);
        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn loading_card_is_centered_bounded_and_stage_aware() {
        let available = Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 730.0));
        let card = merge_loading_card_rect(available);
        assert_eq!(card.center(), available.center());
        assert_eq!(card.size(), Vec2::new(560.0, 272.0));

        let compact_available = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 240.0));
        let compact_card = merge_loading_card_rect(compact_available);
        assert_eq!(compact_card.size(), Vec2::new(352.0, 192.0));
        assert!(compact_available.contains(compact_card.min));
        assert!(compact_available.contains(compact_card.max));

        assert_eq!(
            merge_loading_active_stage_label(
                MergeLanguage::Chinese,
                MergeLoadStage::PreparingEditor,
            ),
            "准备编辑器 · 处理中"
        );
    }

    #[test]
    fn loading_panel_uses_a_fixed_centered_card_and_segmented_progress() {
        let source = include_str!("merge_tool.rs");
        let panel = source
            .split("fn merge_loading_panel")
            .nth(1)
            .and_then(|tail| tail.split("fn merge_loading_card_rect").next())
            .unwrap();

        assert!(panel.contains("merge_loading_card_rect(available)"));
        assert!(panel.contains("UiBuilder::new().max_rect(card_rect)"));
        assert!(panel.contains("merge_loading_progress_track("));
        assert!(!panel.contains("ui.add_space(((ui.available_width() - card_width)"));
    }

    #[test]
    fn large_sparse_merge_prepares_without_quadratic_diff_matrix() {
        let base = (0..8_000)
            .map(|index| format!("package-{index:05}: 1.0.{index}"))
            .collect::<Vec<_>>();
        let mut local = base.clone();
        let mut remote = base.clone();
        for index in [480, 3_840, 7_780] {
            local[index] = format!("package-{index:05}: left-{index}");
            remote[index] = format!("package-{index:05}: right-{index}");
        }
        let sources = MergeSourceText {
            base: base.join("\n") + "\n",
            local: local.join("\n") + "\n",
            remote: remote.join("\n") + "\n",
        };

        let started_at = Instant::now();
        let document = three_way_merge(&sources.base, &sources.local, &sources.remote);
        let prepared = prepare_merge_document(&test_merge_args(), document, sources);
        let elapsed = started_at.elapsed();

        assert_eq!(prepared.document.unresolved_conflict_count(), 3);
        assert_eq!(prepared.local_display_rows.len(), 8_000);
        assert_eq!(prepared.remote_display_rows.len(), 8_000);
        assert!(
            elapsed < Duration::from_secs(15),
            "large sparse merge preparation took {elapsed:?}"
        );
    }

    #[test]
    fn merge_preparation_highlights_both_sides_and_the_displayed_result() {
        let base = "export const value = 1\n";
        let local = "export const value = 2\n";
        let remote = "export const value = 3\n";
        let sources = MergeSourceText {
            base: base.to_owned(),
            local: local.to_owned(),
            remote: remote.to_owned(),
        };
        let args = MergeArgs {
            output: PathBuf::from("src/view.ts"),
            ..test_merge_args()
        };
        let prepared = prepare_merge_document(&args, three_way_merge(base, local, remote), sources);

        for document in [
            prepared.syntax_highlights.local.as_ref(),
            prepared.syntax_highlights.remote.as_ref(),
            prepared.syntax_highlights.result.as_ref(),
        ] {
            assert!(document.is_some());
            assert!(document.unwrap().lines.iter().any(|line| {
                line.spans
                    .iter()
                    .any(|span| span.role == SyntaxRole::Keyword)
            }));
        }
    }

    #[test]
    fn side_syntax_highlight_never_applies_spans_to_different_text() {
        let source = "const expected = true\n";
        let document = three_way_merge(source, source, source);
        let mut row = cached_merge_side_display_rows(&document, MergeSide::Local)
            .into_iter()
            .next()
            .unwrap();
        let highlights = MergeSyntaxHighlights {
            local: crate::syntax::highlight_document(Path::new("."), "src/view.ts", source),
            local_source_lines: vec!["const expected = true".to_owned()],
            ..Default::default()
        };

        assert!(merge_side_highlighted_line(&highlights, MergeSide::Local, &row).is_some());
        row.text = "const visible = true".to_owned();
        assert!(merge_side_highlighted_line(&highlights, MergeSide::Local, &row).is_none());
    }

    #[test]
    fn side_syntax_highlight_recovers_a_unique_line_after_alignment_shift() {
        let source = "<script setup lang=\"ts\">\nconst props = withDefaults(defineProps<{}>(), {})\n</script>\n";
        let document = three_way_merge(source, source, source);
        let mut row = cached_merge_side_display_rows(&document, MergeSide::Local)
            .into_iter()
            .find(|row| row.text.starts_with("const props"))
            .unwrap();
        row.line_number = Some(25);
        let source_lines = source.lines().map(str::to_owned).collect::<Vec<_>>();
        let highlights = MergeSyntaxHighlights {
            local: crate::syntax::highlight_document(Path::new("."), "src/View.vue", source),
            local_unique_source_lines: merge_unique_source_line_indices(&source_lines),
            local_source_lines: source_lines,
            ..Default::default()
        };

        let highlighted = merge_side_highlighted_line(&highlights, MergeSide::Local, &row).unwrap();
        assert!(
            highlighted
                .spans
                .iter()
                .any(|span| span.role == SyntaxRole::Keyword)
        );

        let repeated = vec![row.text.clone(), row.text.clone()];
        let ambiguous = MergeSyntaxHighlights {
            local: highlights.local.clone(),
            local_unique_source_lines: merge_unique_source_line_indices(&repeated),
            local_source_lines: repeated,
            ..Default::default()
        };
        assert!(merge_side_highlighted_line(&ambiguous, MergeSide::Local, &row).is_none());
    }

    #[test]
    fn syntax_layout_drops_spans_that_split_identifiers() {
        let text = "const ratioOptions";
        let highlighted = HighlightedLine {
            spans: vec![crate::syntax::HighlightSpan {
                start: 4,
                end: 9,
                role: SyntaxRole::Keyword,
            }],
        };
        let job = merge_syntax_layout_job(
            text,
            Some(&highlighted),
            Color32::BLACK,
            merge_palette(MergeTheme::Light),
            MERGE_CODE_FONT_SIZE,
        );

        assert_eq!(job.sections.len(), 1);
        assert_eq!(job.sections[0].byte_range, 0..text.len());
    }

    #[test]
    fn ai_overlay_uses_cached_middle_rows_and_does_not_clone_suggestions_per_frame() {
        let source = include_str!("merge_tool.rs");
        let overlay = source
            .split("fn merge_ai_suggestion_overlays")
            .nth(1)
            .and_then(|tail| tail.split("fn merge_ai_middle_anchor").next())
            .unwrap();

        assert!(overlay.contains("app.ai_middle_edit_rows"));
        assert!(!overlay.contains("values().cloned()"));
        assert!(!overlay.contains("manual_result_lines"));
    }

    #[test]
    fn word_highlight_marks_only_changed_tokens_in_order() {
        let text = "runtime_mode = local";
        let reference = "runtime_mode = base";
        let ranges = merge_word_highlight_ranges(text, reference);

        assert_eq!(ranges.len(), 1);
        assert_eq!(&text[ranges[0].clone()], "local");
    }

    #[test]
    fn word_highlight_does_not_confuse_repeated_tokens() {
        let text = "alpha beta alpha";
        let reference = "alpha gamma alpha";
        let ranges = merge_word_highlight_ranges(text, reference);

        assert_eq!(ranges.len(), 1);
        assert_eq!(&text[ranges[0].clone()], "beta");
    }

    #[test]
    fn word_highlight_marks_a_pure_insertion_as_all_new() {
        let text = "local_insert_01 = enabled";
        let ranges = merge_word_highlight_ranges(text, "");

        assert_eq!(ranges.len(), 3);
        assert_eq!(&text[ranges[0].clone()], "local_insert_01");
        assert_eq!(&text[ranges[1].clone()], "=");
        assert_eq!(&text[ranges[2].clone()], "enabled");
    }

    #[test]
    fn word_mode_preserves_a_shallow_changed_line_background() {
        let palette = merge_palette(MergeTheme::Light);
        assert_ne!(
            merge_code_row_fill(
                MergeSideLineTone::Replaced,
                true,
                false,
                MergeHighlightMode::Words,
                palette,
            ),
            None
        );
    }

    #[test]
    fn word_mode_marks_non_conflicting_insertions_with_a_shallow_line_background() {
        let palette = merge_palette(MergeTheme::Light);

        assert_ne!(
            merge_code_row_fill(
                MergeSideLineTone::Added,
                false,
                false,
                MergeHighlightMode::Words,
                palette,
            ),
            None
        );
        assert_eq!(
            merge_code_row_fill(
                MergeSideLineTone::Added,
                false,
                false,
                MergeHighlightMode::Lines,
                palette,
            ),
            None
        );
    }

    #[test]
    fn word_mode_marks_unresolved_insertions_with_the_conflict_background() {
        let palette = merge_palette(MergeTheme::Light);
        let unresolved = merge_code_row_fill(
            MergeSideLineTone::Added,
            true,
            false,
            MergeHighlightMode::Words,
            palette,
        );
        let non_conflicting = merge_code_row_fill(
            MergeSideLineTone::Added,
            false,
            false,
            MergeHighlightMode::Words,
            palette,
        );

        assert_ne!(unresolved, None);
        assert_ne!(unresolved, non_conflicting);
        assert_eq!(
            unresolved.map(|color| color.a()),
            Some((255.0 * MERGE_WORD_BLOCK_OPACITY).round() as u8)
        );
    }

    #[test]
    fn result_editor_has_localized_placeholder() {
        let source = include_str!("merge_tool.rs");

        assert!(source.contains("mt(app.language, \"result_placeholder\")"));
        assert!(source.contains("TextEdit::singleline(text)"));
        assert!(source.contains("merge_editable_result_row"));
        assert!(source.contains("manual_result_override"));
        assert_eq!(
            mt(MergeLanguage::Chinese, "result_placeholder"),
            "\u{8bf7}\u{8f93}\u{5165}\u{5408}\u{5e76}\u{7ed3}\u{679c}"
        );
        assert_eq!(
            mt(MergeLanguage::English, "result_placeholder"),
            "Enter merge result"
        );
    }

    #[test]
    fn manual_result_edit_resolves_conflicts_and_is_undoable() {
        let document = three_way_merge(
            "keep\nbase\nend\n",
            "keep\nlocal\nend\n",
            "keep\nremote\nend\n",
        );
        let mut app = MergeToolApp::new(test_merge_args(), document);

        assert_eq!(app.unresolved_conflict_count(), 1);
        assert!(!app.can_apply_result());

        let before = app.snapshot();
        app.manual_result_lines = vec![
            "keep".to_owned(),
            "manual result".to_owned(),
            "end".to_owned(),
        ];
        app.finish_manual_result_edit(before);

        assert!(app.manual_result_override);
        assert_eq!(app.unresolved_conflict_count(), 0);
        assert!(app.can_apply_result());
        assert_eq!(app.result_text, "keep\nmanual result\nend\n");

        assert!(app.undo());
        assert!(!app.manual_result_override);
        assert_eq!(app.unresolved_conflict_count(), 1);
        assert!(!app.can_apply_result());

        assert!(app.redo());
        assert!(app.manual_result_override);
        assert_eq!(app.result_text, "keep\nmanual result\nend\n");
    }

    #[test]
    fn manual_result_edit_rehighlights_in_a_background_task() {
        let args = MergeArgs {
            output: PathBuf::from("src/view.ts"),
            ..test_merge_args()
        };
        let document = three_way_merge(
            "export const value = 1\n",
            "export const value = 1\n",
            "export const value = 1\n",
        );
        let mut app = MergeToolApp::new(args, document);
        let before = app.snapshot();
        app.manual_result_lines = vec!["export function updated() {}".to_owned()];
        app.finish_manual_result_edit(before);
        app.result_highlight_due = Some(Instant::now());

        let ctx = egui::Context::default();
        for _ in 0..100 {
            app.poll_result_highlight(&ctx);
            if app.result_highlight_task.is_none() && app.result_highlight_due.is_none() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        let document = app.syntax_highlights.result.as_ref().unwrap();
        assert!(
            document.lines[0]
                .spans
                .iter()
                .any(|span| span.role == SyntaxRole::Keyword)
        );
    }

    #[test]
    fn large_merge_uses_cached_virtual_rows_with_visible_connectors() {
        let content = (0..MERGE_VIRTUAL_ROW_THRESHOLD)
            .map(|index| format!("lock-entry-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let document = three_way_merge(&content, &content, &content);
        let app = MergeToolApp::new(test_merge_args(), document);
        let source = include_str!("merge_tool.rs");
        let columns_source = source
            .split("fn merge_editor_columns")
            .nth(1)
            .and_then(|tail| tail.split("fn merge_side_panel").next())
            .expect("merge editor columns implementation");
        let side_panel_source = source
            .split("fn merge_side_panel")
            .nth(1)
            .and_then(|tail| tail.split("fn merge_result_panel").next())
            .expect("merge side panel implementation");

        assert!(app.uses_virtual_merge_rows());
        assert_eq!(app.local_display_rows.len(), MERGE_VIRTUAL_ROW_THRESHOLD);
        assert_eq!(app.remote_display_rows.len(), MERGE_VIRTUAL_ROW_THRESHOLD);
        assert!(source.contains(".show_rows(ui, MERGE_CODE_ROW_HEIGHT, display_rows"));
        assert!(!columns_source.contains("if !use_virtual_rows"));
        assert!(!side_panel_source.contains("if !use_virtual_rows"));
        assert!(columns_source.contains("paint_merge_block_connectors("));
        assert!(side_panel_source.contains("paint_base_only_side_overlays("));
    }

    #[test]
    fn virtual_visible_geometry_keeps_conflict_and_base_only_connectors() {
        let prefix = (0..MERGE_VIRTUAL_ROW_THRESHOLD)
            .map(|index| format!("lock-entry-{index}"))
            .collect::<Vec<_>>();

        let mut conflict_base = prefix.clone();
        conflict_base.extend(["base conflict".to_owned(), "end".to_owned()]);
        let mut conflict_local = prefix.clone();
        conflict_local.extend(["local conflict".to_owned(), "end".to_owned()]);
        let mut conflict_remote = prefix.clone();
        conflict_remote.extend(["remote conflict".to_owned(), "end".to_owned()]);
        let conflict_document = three_way_merge(
            &(conflict_base.join("\n") + "\n"),
            &(conflict_local.join("\n") + "\n"),
            &(conflict_remote.join("\n") + "\n"),
        );
        let conflict = &conflict_document.conflicts()[0];
        let mut result_geometry = MergePanelGeometry::default();
        let (result_first, result_count) =
            merge_result_row_span_for_conflict(&conflict_document, conflict).unwrap();
        for offset in 0..result_count {
            result_geometry.record_row(
                result_first + offset,
                Rect::from_min_size(
                    Pos2::new(400.0, 100.0 + offset as f32 * MERGE_CODE_ROW_HEIGHT),
                    Vec2::new(300.0, MERGE_CODE_ROW_HEIGHT),
                ),
            );
        }
        assert!(
            merge_block_result_rect_from_geometry(&conflict_document, conflict, &result_geometry)
                .is_some()
        );
        for side in [MergeSide::Local, MergeSide::Remote] {
            let (side_first, side_count) =
                merge_side_row_span_for_conflict(&conflict_document, side, conflict).unwrap();
            let mut side_geometry = MergePanelGeometry::default();
            for offset in 0..side_count {
                side_geometry.record_row(
                    side_first + offset,
                    Rect::from_min_size(
                        Pos2::new(20.0, 100.0 + offset as f32 * MERGE_CODE_ROW_HEIGHT),
                        Vec2::new(300.0, MERGE_CODE_ROW_HEIGHT),
                    ),
                );
            }
            assert!(
                merge_block_side_rect_from_geometry(
                    &conflict_document,
                    conflict,
                    side,
                    &side_geometry
                )
                .is_some()
            );
        }

        let mut deletion_base = prefix.clone();
        deletion_base.extend([
            "keep".to_owned(),
            ".claude/*".to_owned(),
            ".agents/*".to_owned(),
            "end".to_owned(),
        ]);
        let mut deletion_local = prefix.clone();
        deletion_local.extend(["keep".to_owned(), "end".to_owned()]);
        let deletion_document = three_way_merge(
            &(deletion_base.join("\n") + "\n"),
            &(deletion_local.join("\n") + "\n"),
            &(deletion_base.join("\n") + "\n"),
        );
        let group = base_only_display_groups(&deletion_document)
            .into_iter()
            .next()
            .expect("large base-only deletion group");
        let result_first =
            merge_result_display_row_for_line(&deletion_document, group.line_index).unwrap();
        let mut deletion_result_geometry = MergePanelGeometry::default();
        for offset in 0..group.line_count {
            deletion_result_geometry.record_row(
                result_first + offset,
                Rect::from_min_size(
                    Pos2::new(400.0, 200.0 + offset as f32 * MERGE_CODE_ROW_HEIGHT),
                    Vec2::new(300.0, MERGE_CODE_ROW_HEIGHT),
                ),
            );
        }
        assert!(
            merge_base_only_result_rect_from_geometry(
                &deletion_document,
                group,
                &deletion_result_geometry
            )
            .is_some()
        );

        let boundary = merge_side_display_row_for_line(
            &deletion_document,
            group.missing_side,
            group.line_index,
        )
        .unwrap();
        let mut deletion_side_geometry = MergePanelGeometry::default();
        deletion_side_geometry.record_row(
            boundary,
            Rect::from_min_size(
                Pos2::new(20.0, 200.0),
                Vec2::new(300.0, MERGE_CODE_ROW_HEIGHT),
            ),
        );
        assert!(
            merge_base_only_side_rect_from_geometry(
                &deletion_document,
                group,
                &deletion_side_geometry
            )
            .is_some()
        );
    }

    #[test]
    fn connector_geometry_uses_fixed_viewport_x_after_horizontal_content_moves() {
        let viewport = Rect::from_min_max(Pos2::new(400.0, 80.0), Pos2::new(700.0, 500.0));
        for content_rect in [
            Rect::from_min_max(Pos2::new(400.0, 120.0), Pos2::new(1_400.0, 138.0)),
            Rect::from_min_max(Pos2::new(-240.0, 120.0), Pos2::new(760.0, 138.0)),
        ] {
            let mut geometry = MergePanelGeometry::default();
            geometry.record_row(7, content_rect);
            geometry.set_horizontal_bounds(viewport);

            let span = geometry.span_rect(7, 1).expect("visible connector span");
            assert_eq!(span.left(), viewport.left());
            assert_eq!(span.right(), viewport.right());
            assert_eq!(span.top(), content_rect.top());
            assert_eq!(span.bottom(), content_rect.bottom());

            let marker = geometry
                .boundary_marker_rect(7, MERGE_BASE_ONLY_MARKER_HEIGHT)
                .expect("visible connector boundary marker");
            assert_eq!(marker.left(), viewport.left());
            assert_eq!(marker.right(), viewport.right());
        }
    }

    #[test]
    fn large_merge_cached_scroll_anchors_keep_conflict_rows_aligned() {
        let mut base = (0..6_000)
            .map(|index| format!("lock-entry-{index}: base"))
            .collect::<Vec<_>>();
        let mut local = base.clone();
        let mut remote = base.clone();
        local.splice(
            1_000..1_000,
            (0..240).map(|index| format!("local-only-{index}: true")),
        );
        base[5_500] = "lock-entry-5500: base".to_owned();
        local[5_740] = "lock-entry-5500: local".to_owned();
        remote[5_500] = "lock-entry-5500: remote".to_owned();
        let document = three_way_merge(
            &(base.join("\n") + "\n"),
            &(local.join("\n") + "\n"),
            &(remote.join("\n") + "\n"),
        );
        assert_eq!(document.unresolved_conflict_count(), 1);
        let conflict_index = document.conflicts()[0].index;
        let app = MergeToolApp::new(test_merge_args(), document);
        assert!(app.uses_virtual_merge_rows());

        let result_scroll =
            merge_result_scroll_y_for_conflict(&app.document, conflict_index).unwrap();
        let viewport_height = 30.0 * MERGE_CODE_ROW_HEIGHT;
        let content_height =
            merge_result_display_rows(&app.document).len() as f32 * MERGE_CODE_ROW_HEIGHT;
        let centered_result_scroll = merge_result_scroll_y_for_conflict_in_view(
            &app.document,
            conflict_index,
            viewport_height,
            content_height,
        )
        .unwrap();
        let (conflict_result_row, conflict_result_count) =
            merge_result_row_span_for_conflict(&app.document, &app.document.conflicts()[0])
                .unwrap();
        let visible_start = centered_result_scroll / MERGE_CODE_ROW_HEIGHT;
        let visible_end = visible_start + viewport_height / MERGE_CODE_ROW_HEIGHT;
        assert!(conflict_result_row as f32 >= visible_start);
        assert!(
            (conflict_result_row + conflict_result_count) as f32 <= visible_end,
            "conflict rows must remain inside the centered result viewport"
        );

        for side in [MergeSide::Local, MergeSide::Remote] {
            let expected_side_row = match side {
                MergeSide::Local => &app.local_display_rows,
                MergeSide::Remote => &app.remote_display_rows,
            }
            .iter()
            .position(|row| row.conflict_index == Some(conflict_index))
            .unwrap() as f32;
            let mapped_side_row = app.cached_side_scroll_y_for_result_scroll(side, result_scroll)
                / MERGE_CODE_ROW_HEIGHT;
            assert!(
                (mapped_side_row - expected_side_row).abs() < 0.01,
                "{side:?} conflict mapped to {mapped_side_row}, expected {expected_side_row}"
            );

            let mapped_result_row = app.cached_result_scroll_y_for_side_scroll(
                side,
                expected_side_row * MERGE_CODE_ROW_HEIGHT,
            ) / MERGE_CODE_ROW_HEIGHT;
            assert!(
                (mapped_result_row - result_scroll / MERGE_CODE_ROW_HEIGHT).abs() < 0.01,
                "{side:?} reverse mapping returned {mapped_result_row}"
            );

            let centered_side_scroll =
                app.cached_side_scroll_y_for_result_scroll(side, centered_result_scroll);
            let centered_side_visible_start = centered_side_scroll / MERGE_CODE_ROW_HEIGHT;
            let centered_side_visible_end =
                centered_side_visible_start + viewport_height / MERGE_CODE_ROW_HEIGHT;
            assert!(expected_side_row >= centered_side_visible_start);
            assert!(
                expected_side_row < centered_side_visible_end,
                "{side:?} conflict row must remain visible after centered navigation"
            );
        }
    }

    #[test]
    fn conflict_navigation_wins_other_pane_scroll_sync() {
        let document = three_way_merge(
            "alpha\nstable\nbeta\n",
            "alpha-local\nstable\nbeta-local\n",
            "alpha-remote\nstable\nbeta-remote\n",
        );
        let conflict_index = document.conflicts()[1].index;
        let viewport_height = 2.0 * MERGE_CODE_ROW_HEIGHT;
        let content_height =
            merge_result_display_rows(&document).len() as f32 * MERGE_CODE_ROW_HEIGHT;
        let expected = merge_result_scroll_y_for_conflict_in_view(
            &document,
            conflict_index,
            viewport_height,
            content_height,
        )
        .unwrap();

        let next = merge_next_shared_scroll_y(
            &document,
            0.0,
            Some(18.0),
            Some(999.0),
            Some(MergeLineActionTarget::Conflict(conflict_index)),
            viewport_height,
            content_height,
            false,
        );

        assert_eq!(next, expected);
        assert_ne!(next, 999.0);
    }

    #[test]
    fn large_merge_cached_scroll_anchors_align_remote_deletion_gap() {
        let base = (0..8_000)
            .map(|index| format!("package-{index:05}: base"))
            .collect::<Vec<_>>();
        let local = base.clone();
        let mut remote = base.clone();
        remote.drain(6_375..6_379);
        let document = three_way_merge(
            &(base.join("\n") + "\n"),
            &(local.join("\n") + "\n"),
            &(remote.join("\n") + "\n"),
        );
        let group = base_only_display_groups(&document)
            .into_iter()
            .find(|group| group.missing_side == MergeSide::Remote)
            .expect("remote deletion group");
        assert_eq!(group.line_count, 4);
        let target = MergeLineActionTarget::BaseOnlyGroup(group.line_index);
        assert_eq!(
            merge_navigation_targets(&document, MergeSide::Remote),
            vec![target]
        );
        assert!(merge_navigation_targets(&document, MergeSide::Local).is_empty());

        let result_first = merge_result_display_row_for_line(&document, group.line_index).unwrap();
        let remote_boundary =
            merge_side_display_row_for_line(&document, MergeSide::Remote, group.line_index)
                .unwrap();
        let app = MergeToolApp::new(test_merge_args(), document.clone());
        assert!(app.uses_virtual_merge_rows());

        for result_row in [result_first, result_first + group.line_count] {
            let mapped_remote_row = app.cached_side_scroll_y_for_result_scroll(
                MergeSide::Remote,
                result_row as f32 * MERGE_CODE_ROW_HEIGHT,
            ) / MERGE_CODE_ROW_HEIGHT;
            assert!(
                (mapped_remote_row - remote_boundary as f32).abs() < 0.01,
                "result row {result_row} mapped to {mapped_remote_row}, expected deletion boundary {remote_boundary}"
            );
        }

        let rows_above = 18;
        let result_scroll_row = result_first - rows_above;
        let mapped_remote_scroll_row = app.cached_side_scroll_y_for_result_scroll(
            MergeSide::Remote,
            result_scroll_row as f32 * MERGE_CODE_ROW_HEIGHT,
        ) / MERGE_CODE_ROW_HEIGHT;
        let result_screen_row = result_first as f32 - result_scroll_row as f32;
        let remote_screen_row = remote_boundary as f32 - mapped_remote_scroll_row;
        assert!(
            (result_screen_row - remote_screen_row).abs() < 0.01,
            "result deletion appears at screen row {result_screen_row}, remote gap at {remote_screen_row}"
        );

        let viewport_height = 30.0 * MERGE_CODE_ROW_HEIGHT;
        let content_height =
            merge_result_display_rows(&document).len() as f32 * MERGE_CODE_ROW_HEIGHT;
        let target_scroll = merge_result_scroll_y_for_navigation_target_in_view(
            &document,
            target,
            viewport_height,
            content_height,
        )
        .expect("deletion navigation scroll");
        let visible_start = target_scroll / MERGE_CODE_ROW_HEIGHT;
        let visible_end = visible_start + viewport_height / MERGE_CODE_ROW_HEIGHT;
        assert!(result_first as f32 >= visible_start);
        assert!((result_first + group.line_count) as f32 <= visible_end);
    }

    #[test]
    fn minimap_viewport_thumb_keeps_size_and_reaches_both_track_ends() {
        let track = Rect::from_min_size(Pos2::ZERO, Vec2::new(18.0, 500.0));
        let viewport_height = 500.0;
        let content_height = 8_000.0;
        let max_scroll = content_height - viewport_height;
        let top = merge_overview_viewport_rect(track, 0.0, viewport_height, content_height);
        let middle =
            merge_overview_viewport_rect(track, max_scroll * 0.5, viewport_height, content_height);
        let bottom =
            merge_overview_viewport_rect(track, max_scroll, viewport_height, content_height);

        assert!((top.height() - middle.height()).abs() < 0.001);
        assert!((middle.height() - bottom.height()).abs() < 0.001);
        assert!((top.top() - track.top()).abs() < 0.001);
        assert!((bottom.bottom() - track.bottom()).abs() < 0.001);
    }

    #[test]
    fn minimap_viewport_thumb_tracks_the_true_visible_center() {
        let track = Rect::from_min_size(Pos2::ZERO, Vec2::new(18.0, 500.0));
        let viewport_height = 500.0;
        let content_height = 144_000.0;
        let max_scroll = content_height - viewport_height;

        for center_ratio in [0.06_f32, 0.38, 0.48, 0.80, 0.97] {
            let scroll_y =
                (content_height * center_ratio - viewport_height * 0.5).clamp(0.0, max_scroll);
            let thumb =
                merge_overview_viewport_rect(track, scroll_y, viewport_height, content_height);
            let expected_center = track.top() + track.height() * center_ratio;
            assert!(
                (thumb.center().y - expected_center).abs() < 0.01,
                "viewport center at {center_ratio:.2} mapped to {}, expected {expected_center}",
                thumb.center().y
            );
        }
    }

    #[test]
    fn minimap_click_centers_the_viewport_on_the_pointer() {
        let track = Rect::from_min_size(Pos2::new(7.0, 20.0), Vec2::new(18.0, 500.0));
        let result_rows = 8_000;
        let viewport_height = 500.0;
        let content_height = result_rows as f32 * MERGE_CODE_ROW_HEIGHT;

        for ratio in [0.06_f32, 0.38, 0.48, 0.80, 0.97] {
            let pointer_y = track.top() + track.height() * ratio;
            let scroll_y = merge_overview_scroll_target(
                track,
                pointer_y,
                result_rows,
                viewport_height,
                content_height,
            );
            let thumb =
                merge_overview_viewport_rect(track, scroll_y, viewport_height, content_height);

            assert!(
                (thumb.center().y - pointer_y).abs() < 0.01,
                "click at {ratio:.2} centered viewport at {}, expected {pointer_y}",
                thumb.center().y
            );
        }
    }

    #[test]
    fn shared_horizontal_scrollbar_reaches_both_ends() {
        let track = Rect::from_min_size(Pos2::new(20.0, 8.0), Vec2::new(900.0, 10.0));
        let thumb_width = 240.0;
        let max_scroll_x = 720.0;

        assert_eq!(
            merge_horizontal_scroll_target(track, track.left(), thumb_width, max_scroll_x),
            0.0,
        );
        assert_eq!(
            merge_horizontal_scroll_target(track, track.right(), thumb_width, max_scroll_x),
            max_scroll_x,
        );
    }

    #[test]
    fn shared_horizontal_offset_moves_all_code_areas_but_keeps_gutters_fixed() {
        let row = Rect::from_min_size(Pos2::new(10.0, 30.0), Vec2::new(360.0, 18.0));
        let scroll_x = 128.0;
        let content_width = 800.0;
        let (side_clip, side_content) = merge_scrolled_code_text_rects(
            row,
            MERGE_SIDE_CODE_GUTTER_WIDTH,
            scroll_x,
            content_width,
        );
        let (result_clip, result_content) = merge_scrolled_code_text_rects(
            row,
            MERGE_RESULT_CODE_GUTTER_WIDTH,
            scroll_x,
            content_width,
        );

        assert_eq!(side_clip.left(), row.left() + MERGE_SIDE_CODE_GUTTER_WIDTH);
        assert_eq!(
            result_clip.left(),
            row.left() + MERGE_RESULT_CODE_GUTTER_WIDTH
        );
        assert_eq!(side_content.left(), side_clip.left() - scroll_x);
        assert_eq!(result_content.left(), result_clip.left() - scroll_x);
        assert_eq!(side_content.width(), content_width);
        assert_eq!(result_content.width(), content_width);
    }

    #[test]
    fn wide_result_editor_stays_inside_its_own_pane() {
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 500.0));
        ctx.begin_pass(egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        });
        let pane = Rect::from_min_size(Pos2::new(260.0, 40.0), Vec2::new(300.0, 360.0));
        let mut observed = None;
        egui::CentralPanel::default().show(&ctx, |ui| {
            observed = Some(merge_pane_ui(ui, pane, |ui| {
                let initial_right = ui.max_rect().right();
                let mut line = "a-very-long-lockfile-value".repeat(80);
                for index in 0..3 {
                    let _ = merge_editable_result_row(
                        ui,
                        index,
                        &mut line,
                        None,
                        MergeSideLineTone::Unchanged,
                        None,
                        MergeHighlightMode::Lines,
                        None,
                        false,
                        0.0,
                        4_000.0,
                        merge_palette(MergeTheme::Light),
                    );
                }
                (initial_right, ui.max_rect().right(), ui.clip_rect())
            }));
        });
        let _ = ctx.end_pass();

        let (initial_right, final_right, clip) = observed.expect("pane rendered");
        assert_eq!(initial_right, pane.right());
        assert_eq!(final_right, pane.right());
        assert_eq!(clip, pane.intersect(screen));

        let source = include_str!("merge_tool.rs");
        let columns = source
            .split("fn merge_editor_columns")
            .nth(1)
            .and_then(|tail| tail.split("fn merge_side_scroll_input").next())
            .expect("merge editor columns implementation");
        assert_eq!(columns.matches("merge_pane_ui(ui,").count(), 3);
    }

    #[test]
    fn minimap_side_markers_project_to_result_rows_after_deletions() {
        let document = navigation_matrix_document();
        let conflict = document.conflicts().last().expect("last conflict");
        let conflict_index = conflict.index;
        let result_row = merge_result_row_span_for_conflict(&document, conflict)
            .expect("result conflict span")
            .0 as f32;
        let app = MergeToolApp::new(test_merge_args(), document);
        let result_rows = merge_visible_result_overview_tones(&app).len();

        for (column_index, rows) in [
            (0, app.local_display_rows.as_slice()),
            (2, app.remote_display_rows.as_slice()),
        ] {
            let side_row = rows
                .iter()
                .position(|row| row.conflict_index == Some(conflict_index))
                .expect("side conflict row") as f32;
            let projected = merge_overview_result_row(&app, column_index, side_row, result_rows);
            assert!(
                (projected - result_row).abs() < 0.01,
                "column {column_index} marker mapped to {projected}, expected {result_row}"
            );
        }
    }

    #[test]
    fn conflict_navigation_wraps_and_reselects_a_single_conflict() {
        let document = three_way_merge("base\n", "local\n", "remote\n");
        assert_eq!(document.unresolved_conflict_count(), 1);
        let targets = merge_navigation_targets(&document, MergeSide::Local);

        assert_eq!(targets.len(), 1);
        assert_eq!(previous_navigation_position(0, targets.len()), 0);
        assert_eq!(next_navigation_position(0, targets.len()), 0);
    }

    #[test]
    fn side_navigation_counts_conflicts_and_its_missing_side_deletion() {
        let document = navigation_matrix_document();
        let local_targets = merge_navigation_targets(&document, MergeSide::Local);
        let remote_targets = merge_navigation_targets(&document, MergeSide::Remote);

        assert_eq!(document.unresolved_conflict_count(), 3);
        assert_eq!(local_targets.len(), 4);
        assert_eq!(remote_targets.len(), 4);
        assert_eq!(
            local_targets
                .iter()
                .filter(|target| matches!(target, MergeLineActionTarget::Conflict(_)))
                .count(),
            3
        );
        assert_eq!(
            remote_targets
                .iter()
                .filter(|target| matches!(target, MergeLineActionTarget::Conflict(_)))
                .count(),
            3
        );
        let local_deletion = local_targets
            .iter()
            .find(|target| matches!(target, MergeLineActionTarget::BaseOnlyGroup(_)))
            .copied()
            .expect("local-side deletion target");
        let remote_deletion = remote_targets
            .iter()
            .find(|target| matches!(target, MergeLineActionTarget::BaseOnlyGroup(_)))
            .copied()
            .expect("remote-side deletion target");
        assert_ne!(local_deletion, remote_deletion);

        assert_eq!(
            previous_navigation_position(0, local_targets.len()),
            local_targets.len() - 1
        );
        assert_eq!(
            next_navigation_position(local_targets.len() - 1, local_targets.len()),
            0
        );
    }

    #[test]
    fn deleting_a_base_line_and_adding_after_it_on_the_other_side_auto_merges() {
        let document = three_way_merge(
            "base\nalpha change\n",
            "base\n",
            "base\nalpha change\nbeta change\n",
        );

        assert_eq!(document.unresolved_conflict_count(), 0);
        assert_eq!(document.result_text(), "base\nbeta change\n");
    }

    #[test]
    fn whitespace_ignore_modes_change_three_way_comparison_without_normalizing_output() {
        let document = three_way_merge_with_options(
            "value = 1\n",
            "value=1\n",
            "value = 1\n",
            MergeIgnoreMode::IgnoreWhitespace,
        );

        assert_eq!(document.unresolved_conflict_count(), 0);
        assert_eq!(document.result_text(), "value = 1\n");
        assert_eq!(
            three_way_merge("value = 1\n", "value=1\n", "value = 1\n").result_text(),
            "value=1\n"
        );
    }

    #[test]
    fn collapsed_tail_keeps_changed_rows_and_small_context() {
        let rows = (0..20)
            .map(|index| CachedMergeSideDisplayRow {
                text: format!("line-{index}"),
                reference_text: None,
                line_number: Some(index + 1),
                conflict_index: (index == 2).then_some(0),
                side_resolved: false,
                tone: MergeSideLineTone::Unchanged,
                show_conflict_actions: false,
                action_target: None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            merge_visible_tail_len(&rows, true),
            3 + MERGE_COLLAPSE_CONTEXT_ROWS
        );
        assert_eq!(merge_visible_tail_len(&rows, false), rows.len());
    }

    #[test]
    fn collapsed_overview_matches_the_editor_row_count() {
        let rows = (0..20)
            .map(|index| CachedMergeSideDisplayRow {
                text: format!("line-{index}"),
                reference_text: None,
                line_number: Some(index + 1),
                conflict_index: (index == 2).then_some(0),
                side_resolved: false,
                tone: MergeSideLineTone::Unchanged,
                show_conflict_actions: false,
                action_target: None,
            })
            .collect::<Vec<_>>();

        let visible = merge_visible_tail_len(&rows, true);
        let overview = merge_visible_side_overview_tones(&rows, true);

        assert_eq!(overview.len(), visible + 1);
        assert_eq!(overview.last(), Some(&MergeSideLineTone::Unchanged));
    }

    #[test]
    fn hidden_scroll_area_offsets_are_clamped_before_sharing() {
        assert_eq!(merge_clamp_scroll_offset(480.0, 180.0, 120.0), 60.0);
        assert_eq!(merge_clamp_scroll_offset(-12.0, 180.0, 120.0), 0.0);
        assert_eq!(merge_clamp_scroll_offset(42.0, 80.0, 120.0), 0.0);
    }

    #[test]
    fn passive_short_side_clamp_cannot_pull_shared_scroll_back_from_bottom() {
        let requested_side_scroll = 144.0;
        let short_side_clamped_scroll = 72.0;

        assert!(!merge_side_offset_changed_by_user(
            false,
            requested_side_scroll,
            short_side_clamped_scroll,
        ));
        assert!(merge_side_offset_changed_by_user(
            true,
            requested_side_scroll,
            short_side_clamped_scroll,
        ));
    }

    #[test]
    fn side_wheel_updates_canonical_result_scroll_and_reaches_the_bottom() {
        let viewport_height = 540.0;
        let content_height = 702.0;
        let max_scroll = content_height - viewport_height;

        assert_eq!(
            merge_scroll_offset_after_input(126.0, -90.0, viewport_height, content_height),
            max_scroll,
        );
        assert_eq!(
            merge_scroll_offset_after_input(max_scroll, 54.0, viewport_height, content_height),
            max_scroll - 54.0,
        );
    }

    #[test]
    fn merge_toolbar_exposes_ignore_highlight_collapse_and_minimap_controls() {
        let source = include_str!("merge_tool.rs");

        assert!(source.contains("MergeIgnoreMode::IgnoreWhitespace"));
        assert!(source.contains("MergeHighlightMode::Words"));
        assert!(source.contains("collapse_unchanged"));
        assert!(source.contains("merge_overview_target"));
        assert!(source.contains("ScrollBarVisibility::AlwaysHidden"));
    }

    #[test]
    fn merge_tool_actions_are_undoable_and_redoable() {
        let document = three_way_merge(
            "keep\nbase\nend\n",
            "keep\nlocal\nend\n",
            "keep\nremote\nend\n",
        );
        let mut app = MergeToolApp::new(test_merge_args(), document);

        assert!(!app.has_unsaved_edits());
        assert!(!app.can_undo());
        assert!(!app.can_redo());

        app.apply_line_action(
            MergeLineActionTarget::Conflict(0),
            MergeSide::Local,
            MergeLineAction::Take,
        );

        assert!(app.has_unsaved_edits());
        assert!(app.can_undo());
        assert_eq!(app.result_text, "keep\nlocal\nend\n");

        assert!(app.undo());
        assert_eq!(app.result_text, "keep\nend\n");
        assert!(!app.has_unsaved_edits());
        assert!(app.can_redo());

        assert!(app.redo());
        assert_eq!(app.result_text, "keep\nlocal\nend\n");
        assert!(app.has_unsaved_edits());
    }

    #[test]
    fn new_merge_action_clears_redo_history() {
        let document = three_way_merge(
            "keep\nbase\nend\n",
            "keep\nlocal\nend\n",
            "keep\nremote\nend\n",
        );
        let mut app = MergeToolApp::new(test_merge_args(), document);

        app.apply_line_action(
            MergeLineActionTarget::Conflict(0),
            MergeSide::Local,
            MergeLineAction::Take,
        );
        assert!(app.undo());
        assert!(app.can_redo());

        app.apply_line_action(
            MergeLineActionTarget::Conflict(0),
            MergeSide::Remote,
            MergeLineAction::Take,
        );

        assert_eq!(app.result_text, "keep\nremote\nend\n");
        assert!(!app.can_redo());
    }

    #[test]
    fn conflict_is_resolved_only_after_both_side_decisions() {
        let mut document = three_way_merge(
            "keep\nbase\nend\n",
            "keep\nlocal\nend\n",
            "keep\nremote\nend\n",
        );

        assert_eq!(document.unresolved_conflict_count(), 1);
        document.drop_conflict_side(0, MergeSide::Local);
        assert_eq!(document.unresolved_conflict_count(), 1);
        document.take_conflict_side(0, MergeSide::Remote);

        assert_eq!(document.unresolved_conflict_count(), 0);
        assert_eq!(document.result_text(), "keep\nremote\nend\n");
    }

    #[test]
    fn resolved_empty_conflict_does_not_fall_back_to_base_preview() {
        let mut document =
            three_way_merge("keep\nbase\nend\n", "keep\nlocal\nend\n", "keep\nend\n");

        document.drop_conflict_side(0, MergeSide::Local);
        document.take_conflict_side(0, MergeSide::Remote);

        assert_eq!(document.unresolved_conflict_count(), 0);
        assert_eq!(document.result_text(), "keep\nend\n");
        assert!(!merge_result_display_lines(&document).contains(&"base"));
        assert!(!merge_result_display_lines(&document).contains(&"local"));
    }

    #[test]
    fn apply_stays_disabled_until_every_conflict_is_resolved() {
        let document = three_way_merge(
            "keep\nbase\nend\n",
            "keep\nlocal\nend\n",
            "keep\nremote\nend\n",
        );
        let mut app = MergeToolApp::new(test_merge_args(), document);

        assert!(!app.can_apply_result());
        app.apply_line_action(
            MergeLineActionTarget::Conflict(0),
            MergeSide::Local,
            MergeLineAction::Drop,
        );
        assert!(!app.can_apply_result());
        app.apply_line_action(
            MergeLineActionTarget::Conflict(0),
            MergeSide::Remote,
            MergeLineAction::Take,
        );
        assert!(app.can_apply_result());
    }

    #[test]
    fn taking_remote_after_dropping_local_handles_moved_remote_block() {
        let base = "shell\ntrust\nminimum\nmicro\nversioned-ant\n";
        let local = "shell\ntrust\nminimum\nmicro\n";
        let remote = "minimum\nmicro\nunversioned-ant\nshell\ntrust\n";
        let mut document = three_way_merge(base, local, remote);

        assert_eq!(document.conflicts().len(), 1);
        document.drop_conflict_side(0, MergeSide::Local);
        document.take_conflict_side(0, MergeSide::Remote);

        assert_eq!(document.unresolved_conflict_count(), 0);
        assert_eq!(document.result_text(), remote);
        assert_eq!(
            merge_result_display_lines(&document).join("\n") + "\n",
            remote
        );
    }

    #[test]
    fn cancel_merge_prompts_only_after_user_edits() {
        let document = three_way_merge(
            "keep\nbase\nend\n",
            "keep\nlocal\nend\n",
            "keep\nremote\nend\n",
        );
        let mut app = MergeToolApp::new(test_merge_args(), document);

        assert_eq!(app.request_cancel(), MergeCancelRequest::ExitNow);
        assert!(!app.show_cancel_confirm);

        app.apply_line_action(
            MergeLineActionTarget::Conflict(0),
            MergeSide::Local,
            MergeLineAction::Take,
        );

        assert_eq!(app.request_cancel(), MergeCancelRequest::ShowConfirm);
        assert!(app.show_cancel_confirm);
    }

    #[test]
    fn cancel_merge_confirmation_is_localized_and_close_is_intercepted() {
        let source = include_str!("merge_tool.rs");

        assert_eq!(MERGE_TOOL_CANCEL_EXIT_CODE, 10);
        assert!(source.contains("std::process::exit(MERGE_TOOL_CANCEL_EXIT_CODE)"));
        assert_eq!(
            mt(MergeLanguage::English, "cancel_merge_title"),
            "Cancel Merge"
        );
        assert_eq!(
            mt(MergeLanguage::Chinese, "cancel_merge_title"),
            "\u{53d6}\u{6d88}\u{5408}\u{5e76}"
        );
        assert!(source.contains("ViewportCommand::CancelClose"));
        assert!(source.contains("viewport().close_requested()"));
        assert!(source.contains("ctrl && i.key_pressed(egui::Key::Z)"));
        assert!(source.contains("ctrl && i.key_pressed(egui::Key::Y)"));
    }

    #[test]
    fn consecutive_conflict_lines_form_one_block() {
        let document = three_way_merge(
            "keep\nbase-a\nbase-b\nbase-c\n",
            "keep\nlocal-a\nlocal-b\n",
            "keep\nremote-a\nremote-b\nremote-c\nremote-d\n",
        );

        assert_eq!(document.conflicts().len(), 1);
        let conflict = &document.conflicts()[0];
        assert_eq!(conflict.local, vec!["local-a", "local-b"]);
        assert_eq!(
            conflict.remote,
            vec!["remote-a", "remote-b", "remote-c", "remote-d"]
        );
        assert_eq!(conflict.line_indices.len(), 4);
        assert!(
            conflict
                .line_indices
                .iter()
                .all(|index| document.lines[*index].conflict_index == Some(0))
        );
    }

    #[test]
    fn unresolved_conflict_result_does_not_include_base_lines() {
        let document = three_way_merge(
            "node_modules/\nbuild/\ncache/\ngraph-cost.json\n",
            "node_modules/\ndist-local/\ncache-local/\ngraph-cost.json\n",
            "node_modules/\nrelease-dist/\ngraph-cache/\ngraph-cost.json\n",
        );

        assert!(document.result_text().contains("node_modules/"));
        assert!(document.result_text().contains("graph-cost.json"));
        assert!(!document.result_text().contains("build/"));
        assert!(!document.result_text().contains("cache/"));
        assert!(
            document
                .lines
                .iter()
                .filter(|line| line.kind == MergeLineKind::Conflict)
                .all(|line| !line.include_in_result)
        );
    }

    #[test]
    fn insertion_conflict_uses_a_boundary_marker_without_a_blank_result_row() {
        let document = three_way_merge(
            "before\nafter\n",
            "before\nlocal add\nafter\n",
            "before\nremote add\nafter\n",
        );
        let conflict = document
            .conflicts()
            .iter()
            .find(|conflict| conflict.base.is_empty())
            .expect("insertion conflict");

        assert!(merge_result_row_span_for_conflict(&document, conflict).is_none());
        assert_eq!(
            merge_result_display_lines(&document),
            vec!["before", "after"]
        );
        assert_eq!(
            merge_result_display_boundary_before_line(
                &document,
                *conflict.line_indices.first().unwrap(),
            ),
            1,
        );
    }

    #[test]
    fn word_mode_paints_every_row_of_an_unresolved_insertion_conflict() {
        let document = three_way_merge(
            "before\nafter\n",
            "before\nlocal insert one\nlocal insert two\nafter\n",
            "before\nremote insert one\nremote insert two\nafter\n",
        );
        let palette = merge_palette(MergeTheme::Light);
        let rows = merge_side_display_rows(&document, MergeSide::Local);
        let conflict_rows = rows
            .iter()
            .filter(|row| row.conflict_index == Some(0))
            .collect::<Vec<_>>();

        assert_eq!(conflict_rows.len(), 2);
        assert!(conflict_rows.iter().all(|row| {
            row.tone == MergeSideLineTone::Replaced
                && merge_code_row_fill(
                    row.tone,
                    !row.side_resolved,
                    false,
                    MergeHighlightMode::Words,
                    palette,
                ) == Some(palette.conflict_fill)
        }));

        let cached_rows = cached_merge_side_display_rows(&document, MergeSide::Local);
        let first_conflict_row = cached_rows
            .iter()
            .position(|row| row.conflict_index == Some(0))
            .expect("insertion conflict row");
        assert_eq!(
            merge_side_background_run(
                &cached_rows,
                first_conflict_row,
                0,
                cached_rows.len(),
                usize::MAX,
                MergeHighlightMode::Words,
                palette,
            ),
            Some((palette.conflict_fill, 2))
        );
    }

    #[test]
    fn one_side_action_block_uses_one_background_even_when_line_tones_differ() {
        let document = three_way_merge(
            "start\n  if (isVip && total <= limit) {\n    return \"approve\";\n  }\nend\n",
            "start\nend\n",
            "start\n  if (isVip && total < limit) {\n    return \"approve\";\n  }\nend\n",
        );
        let palette = merge_palette(MergeTheme::Light);
        let rows = cached_merge_side_display_rows(&document, MergeSide::Remote)
            .into_iter()
            .filter(|row| row.conflict_index == Some(0))
            .collect::<Vec<_>>();
        let distinct_tones = rows.iter().map(|row| row.tone).collect::<HashSet<_>>();
        let expected_fill = palette.conflict_fill;

        assert!(
            distinct_tones.len() > 1,
            "fixture must contain mixed line tones"
        );
        assert!(rows.iter().all(|row| {
            merge_side_row_fill(row, usize::MAX, MergeHighlightMode::Lines, palette)
                == Some(expected_fill)
        }));
        assert_eq!(
            merge_side_background_run(
                &rows,
                0,
                0,
                rows.len(),
                usize::MAX,
                MergeHighlightMode::Lines,
                palette,
            ),
            Some((expected_fill, rows.len())),
        );
    }

    #[test]
    fn result_display_rows_show_unresolved_base_blocks_without_side_lines() {
        let document = three_way_merge(
            "# Merge Tool Complex Fixture\n\nsection: stable header\nalpha: unchanged\nbeta: unchanged\n\nsection: shared ignore patterns\ndist/\nnode_modules/\nbuild/\ncache/\ngraph-cost.json\ncache-remote/\n\nsection: agent files\n.vscode/*\n.cursor/*\n.claude/*\n.agents/*\n!.vscode/extensions.json\n.idea\n.vscode\nvite.config.mts.*.mjs\nnil\nCLAUDE.md\n.codegraph/*\nAGENTS.md\n.codex/*\n\nsection: stable footer\nomega: unchanged\nzeta: unchanged\n",
            "# Merge Tool Complex Fixture\n\nsection: stable header\nalpha: unchanged\nbeta: unchanged\n\nsection: shared ignore patterns\ndist/\nnode_modules/\ndist-local/\ncache-local/\ngraph-cost.json\ncache-remote/\n\nsection: agent files\n.vscode/*\n.cursor/*\n.claude/*\n.agents/*\n!.vscode/extensions.json\n.idea\nvite.config.mts.*.mjs\nnil\nCLAUDE.local.md\n.codegraph-local/*\nAGENTS.local.md\n.codex-local/*\n\nsection: stable footer\nomega: unchanged\nzeta: unchanged\n",
            "# Merge Tool Complex Fixture\n\nsection: stable header\nalpha: unchanged\nbeta: unchanged\n\nsection: shared ignore patterns\ndist/\nnode_modules/\nrelease-dist/\ngraph-cache/\ngraph-cost.json\ncache-remote/\n\nsection: agent files\n.vscode/*\n.cursor/*\n.claude/*\n.agents/*\n!.vscode/extensions.json\n.idea\nvite.config.mts.*.mjs\nnil\n**/graphify-out/cache/\n**/graphify-out/cost.json\n\nsection: stable footer\nomega: unchanged\nzeta: unchanged\n",
        );

        let rows = merge_result_display_lines(&document);

        assert!(rows.contains(&"build/"));
        assert!(rows.contains(&"cache/"));
        assert!(!rows.contains(&"dist-local/"));
        assert!(!rows.contains(&"release-dist/"));
        assert_eq!(rows.iter().filter(|row| row.is_empty()).count(), 4);
        assert_eq!(rows[8], "node_modules/");
        assert_eq!(rows[9], "build/");
        assert_eq!(rows[10], "cache/");
        assert!(rows.contains(&".claude/*"));
        assert!(rows.contains(&".agents/*"));
        assert_eq!(rows[22], "nil");
        assert_eq!(rows.len(), 31);
    }

    #[test]
    fn result_display_rows_show_unresolved_base_replacement_rows() {
        let document = three_way_merge(
            "keep\nbuild/\ncache/\nend\n",
            "keep\ndist-local/\ncache-local/\nend\n",
            "keep\nrelease-dist/\ngraph-cache/\nend\n",
        );

        let rows = merge_result_display_lines(&document);

        assert_eq!(rows, vec!["keep", "build/", "cache/", "end"]);
    }

    #[test]
    fn result_connector_rect_spans_unresolved_base_replacement_rows() {
        let document = three_way_merge(
            "keep\nbuild/\ncache/\nend\n",
            "keep\ndist-local/\ncache-local/\nend\n",
            "keep\nrelease-dist/\ngraph-cache/\nend\n",
        );
        let panel = Rect::from_min_size(Pos2::new(100.0, 40.0), Vec2::new(360.0, 640.0));
        let rect = merge_block_result_rect(panel, &document, &document.conflicts()[0], 0.0)
            .expect("replacement result block");
        let top = merge_scroll_content_top(panel) + MERGE_CODE_ROW_HEIGHT;

        assert_eq!(rect.top(), top);
        assert_eq!(rect.bottom(), top + MERGE_CODE_ROW_HEIGHT * 2.0);
    }

    #[test]
    fn result_connector_rect_extends_to_line_number_gutter() {
        let document = three_way_merge(
            "keep\nbuild/\ncache/\nend\n",
            "keep\ndist-local/\ncache-local/\nend\n",
            "keep\nrelease-dist/\ngraph-cache/\nend\n",
        );
        let panel = Rect::from_min_size(Pos2::new(100.0, 40.0), Vec2::new(360.0, 640.0));
        let rect = merge_block_result_rect(panel, &document, &document.conflicts()[0], 0.0)
            .expect("result connector rect");

        assert_eq!(rect.left(), merge_scroll_clip_rect(panel).left());
    }

    #[test]
    fn base_only_result_connector_extends_to_line_number_gutter() {
        let document = three_way_merge(
            "keep\n.claude/*\n.agents/*\nend\n",
            "keep\nend\n",
            "keep\n.claude/*\n.agents/*\nend\n",
        );
        let panel = Rect::from_min_size(Pos2::new(100.0, 40.0), Vec2::new(360.0, 640.0));
        let group = base_only_display_groups(&document)
            .into_iter()
            .next()
            .expect("base-only group");
        let rect = merge_base_only_result_rect(panel, &document, group, 0.0)
            .expect("base-only result connector rect");

        assert_eq!(rect.left(), merge_scroll_clip_rect(panel).left());
    }

    #[test]
    fn base_only_rows_do_not_draw_top_or_bottom_outline() {
        assert!(!should_paint_result_block_outline(
            MergeSideLineTone::BaseOnly
        ));
        assert!(should_paint_result_block_outline(MergeSideLineTone::Added));
        assert!(!should_paint_result_block_outline(
            MergeSideLineTone::Replaced
        ));
    }

    #[test]
    fn connector_fill_uses_row_color_with_ninety_percent_opacity() {
        let palette = merge_palette(MergeTheme::Light);

        assert_eq!(
            merge_connector_fill(MergeSideLineTone::BaseOnly, palette),
            Color32::from_rgba_unmultiplied(214, 221, 230, 230)
        );

        assert_eq!(
            merge_connector_fill(MergeSideLineTone::Replaced, palette),
            Color32::from_rgba_unmultiplied(
                palette.conflict_fill.r(),
                palette.conflict_fill.g(),
                palette.conflict_fill.b(),
                230,
            )
        );
    }

    #[test]
    fn active_conflict_fill_is_distinct_from_regular_conflict_fill() {
        for theme in [MergeTheme::Light, MergeTheme::Dark] {
            let palette = merge_palette(theme);
            assert_ne!(palette.active_conflict_fill, palette.conflict_fill);
        }
    }

    #[test]
    fn word_mode_keeps_base_only_deletions_fully_visible() {
        let palette = merge_palette(MergeTheme::Light);

        assert_eq!(
            merge_highlight_fill(
                MergeSideLineTone::BaseOnly,
                palette.base_only_fill,
                false,
                MergeHighlightMode::Words,
            ),
            palette.base_only_fill
        );
        assert_eq!(
            merge_highlight_fill(
                MergeSideLineTone::Replaced,
                palette.conflict_fill,
                false,
                MergeHighlightMode::Words,
            ),
            palette.conflict_fill
        );
    }

    #[test]
    fn contiguous_result_conflict_rows_use_one_background_span() {
        let palette = merge_palette(MergeTheme::Light);
        let styles = vec![
            (MergeSideLineTone::Replaced, false),
            (MergeSideLineTone::Replaced, false),
            (MergeSideLineTone::Unchanged, false),
        ];

        assert_eq!(
            merge_result_background_run(
                &styles,
                0,
                0,
                styles.len(),
                MergeHighlightMode::Lines,
                palette,
            ),
            Some((
                merge_result_row_fill(MergeSideLineTone::Replaced, false, palette),
                2,
            ))
        );
        assert_eq!(
            merge_result_background_run(
                &styles,
                1,
                0,
                styles.len(),
                MergeHighlightMode::Lines,
                palette,
            ),
            None
        );
    }

    #[test]
    fn result_editor_uses_display_tones_for_base_only_and_conflict_rows() {
        let source = include_str!("merge_tool.rs");
        let result_panel = source
            .split("fn merge_result_panel")
            .nth(1)
            .and_then(|tail| tail.split("fn result_header").next())
            .expect("result panel source");
        let row_painter = source
            .split("fn merge_editable_result_row")
            .nth(1)
            .and_then(|tail| tail.split("#[cfg(test)]").next())
            .expect("result row source");

        assert!(result_panel.contains("app.result_display_rows"));
        assert!(!result_panel.contains("merge_result_display_rows(&app.document)"));
        assert!(row_painter.contains("MergeSideLineTone::BaseOnly"));
        assert!(row_painter.contains("paint_result_side_status_badges"));
        assert!(row_painter.contains("MergeSideLineTone::LocalDeletedRemoteEdited"));
        assert!(row_painter.contains("MergeSideLineTone::LocalEditedRemoteDeleted"));
        assert!(row_painter.contains("palette.active_conflict_fill"));
    }

    #[test]
    fn base_only_marker_line_uses_base_only_row_fill() {
        let source = include_str!("merge_tool.rs");
        let marker_source = source
            .split("fn paint_base_only_gap_marker_rect")
            .nth(1)
            .and_then(|tail| tail.split("fn base_only_gap_marker_rect").next())
            .expect("marker painter source");

        assert!(marker_source.contains("palette.base_only_fill"));
        assert!(!marker_source.contains("palette.base_only_text"));
    }

    #[test]
    fn result_connector_uses_display_rows_without_vertical_offset() {
        let document = three_way_merge(
            "# Merge Tool Complex Fixture\n\nsection: stable header\nalpha: unchanged\nbeta: unchanged\n\nsection: shared ignore patterns\ndist/\nnode_modules/\nbuild/\ncache/\ngraph-cost.json\ncache-remote/\n\nsection: agent files\n.vscode/*\n.cursor/*\n.claude/*\n.agents/*\n!.vscode/extensions.json\n.idea\n.vscode\nvite.config.mts.*.mjs\nnil\nCLAUDE.md\n.codegraph/*\nAGENTS.md\n.codex/*\n\nsection: stable footer\nomega: unchanged\nzeta: unchanged\n",
            "# Merge Tool Complex Fixture\n\nsection: stable header\nalpha: unchanged\nbeta: unchanged\n\nsection: shared ignore patterns\ndist/\nnode_modules/\ndist-local/\ncache-local/\ngraph-cost.json\ncache-remote/\n\nsection: agent files\n.vscode/*\n.cursor/*\n.claude/*\n.agents/*\n!.vscode/extensions.json\n.idea\nvite.config.mts.*.mjs\nnil\nCLAUDE.local.md\n.codegraph-local/*\nAGENTS.local.md\n.codex-local/*\n\nsection: stable footer\nomega: unchanged\nzeta: unchanged\n",
            "# Merge Tool Complex Fixture\n\nsection: stable header\nalpha: unchanged\nbeta: unchanged\n\nsection: shared ignore patterns\ndist/\nnode_modules/\nrelease-dist/\ngraph-cache/\ngraph-cost.json\ncache-remote/\n\nsection: agent files\n.vscode/*\n.cursor/*\n.claude/*\n.agents/*\n!.vscode/extensions.json\n.idea\nvite.config.mts.*.mjs\nnil\n**/graphify-out/cache/\n**/graphify-out/cost.json\n\nsection: stable footer\nomega: unchanged\nzeta: unchanged\n",
        );
        let panel = Rect::from_min_size(Pos2::new(100.0, 40.0), Vec2::new(360.0, 640.0));
        let first = merge_block_result_rect(panel, &document, &document.conflicts()[0], 0.0)
            .expect("first conflict line");
        let second = merge_block_result_rect(panel, &document, &document.conflicts()[1], 0.0)
            .expect("second conflict line");

        let content_top = merge_scroll_content_top(panel);
        assert_eq!(first.top(), content_top + 9.0 * MERGE_CODE_ROW_HEIGHT);
        assert_eq!(second.top(), content_top + 23.0 * MERGE_CODE_ROW_HEIGHT);
    }

    #[test]
    fn side_display_rows_show_replacements_as_replace_rows() {
        let document = three_way_merge(
            "keep\nbuild/\ncache/\nend\n",
            "keep\ndist-local/\ncache-local/\nend\n",
            "keep\nrelease-dist/\ngraph-cache/\nend\n",
        );

        let rows = merge_side_display_rows(&document, MergeSide::Local);
        let changed = rows
            .iter()
            .filter(|row| row.conflict_index == Some(0))
            .map(|row| (row.text, row.tone))
            .collect::<Vec<_>>();

        assert_eq!(
            changed,
            vec![
                ("dist-local/", MergeSideLineTone::Replaced),
                ("cache-local/", MergeSideLineTone::Replaced),
            ]
        );
    }

    #[test]
    fn extra_rows_inside_side_replacement_blocks_stay_replacements() {
        let document = three_way_merge(
            "keep\nclaude.md\n.codegraph/*\n.mcp.json\nAGENTS.md\n.codex/*\nend\n",
            "keep\nCLAUDE.local.md\n.codegraph-local/*\nAGENTS.local.md\n.codex-local/*\nend\n",
            "keep\n**/graphify-out/cache/\n**/graphify-out/cost.json\nend\n",
        );

        let local_rows = merge_side_display_rows(&document, MergeSide::Local)
            .iter()
            .filter(|row| row.conflict_index == Some(0))
            .map(|row| (row.text, row.tone))
            .collect::<Vec<_>>();
        let remote_rows = merge_side_display_rows(&document, MergeSide::Remote)
            .iter()
            .filter(|row| row.conflict_index == Some(0))
            .map(|row| (row.text, row.tone))
            .collect::<Vec<_>>();

        assert_eq!(
            local_rows,
            vec![
                ("CLAUDE.local.md", MergeSideLineTone::Replaced),
                (".codegraph-local/*", MergeSideLineTone::Replaced),
                ("AGENTS.local.md", MergeSideLineTone::Replaced),
                (".codex-local/*", MergeSideLineTone::Replaced),
            ]
        );
        assert_eq!(
            remote_rows,
            vec![
                ("**/graphify-out/cache/", MergeSideLineTone::Replaced),
                ("**/graphify-out/cost.json", MergeSideLineTone::Replaced),
            ]
        );
    }

    #[test]
    fn opposing_base_deletions_render_as_replacements() {
        let document = three_way_merge(
            "keep\nalpha: unchanged\nbeta: unchanged\nend\n",
            "keep\nbeta: unchanged\nend\n",
            "keep\nalpha: unchanged\nend\n",
        );

        assert_eq!(document.conflicts().len(), 1);
        assert_eq!(
            document.conflicts()[0].base,
            vec!["alpha: unchanged", "beta: unchanged"]
        );

        let local_rows = merge_side_display_rows(&document, MergeSide::Local)
            .iter()
            .filter(|row| row.conflict_index == Some(0))
            .map(|row| (row.text, row.tone, row.line_number))
            .collect::<Vec<_>>();
        let remote_rows = merge_side_display_rows(&document, MergeSide::Remote)
            .iter()
            .filter(|row| row.conflict_index == Some(0))
            .map(|row| (row.text, row.tone, row.line_number))
            .collect::<Vec<_>>();

        assert_eq!(
            local_rows,
            vec![("beta: unchanged", MergeSideLineTone::Replaced, Some(2))]
        );
        assert_eq!(
            remote_rows,
            vec![("alpha: unchanged", MergeSideLineTone::Replaced, Some(2))]
        );

        let result_rows = merge_result_display_rows(&document)
            .iter()
            .filter(|row| row.conflict_index == Some(0))
            .map(|row| (row.text, row.tone))
            .collect::<Vec<_>>();
        assert_eq!(
            result_rows,
            vec![
                ("alpha: unchanged", MergeSideLineTone::Replaced),
                ("beta: unchanged", MergeSideLineTone::Replaced),
            ]
        );
    }

    #[test]
    fn repeated_return_statements_stay_with_their_local_conflict_block() {
        let document = three_way_merge(
            "start\n  if (fraudSignals > allowedFraudSignalCount(policy)) {\n    return \"review\";\n  }\n  if (cartTotal > policy.manualReviewAbove) {\n    return \"review\";\n  }\n  if (isVip && cartTotal <= vipAutoApproveLimit(policy)) {\n    return \"approve\";\n  }\nend\n",
            "start\n  if (fraudSignals >= allowedFraudSignalCount(policy)) {\n    return \"review\";\n  }\n  if (cartTotal > policy.manualReviewAbove) {\n    return \"review\";\n  }\nend\n",
            "start\n  // Reject excessive fraud evidence.\n  if (fraudSignals > signalLimit) {\n    return \"reject\";\n  }\n  if (cartTotal > policy.manualReviewAbove || fraudSignals === signalLimit) {\n    return \"review\";\n  }\n  if (isVip && cartTotal < vipAutoApproveLimit(policy)) {\n    return \"approve\";\n  }\nend\n",
        );

        let conflict_index = document
            .conflicts()
            .iter()
            .find(|conflict| {
                conflict
                    .base
                    .iter()
                    .any(|line| line.contains("fraudSignals > allowedFraudSignalCount"))
            })
            .map(|conflict| conflict.index)
            .expect("fraud policy conflict");
        let rows = merge_result_display_rows(&document)
            .into_iter()
            .filter(|row| row.conflict_index == Some(conflict_index))
            .map(|row| (row.text, row.tone))
            .collect::<Vec<_>>();
        let fraud_condition = rows
            .iter()
            .position(|(text, _)| text.contains("fraudSignals > allowedFraudSignalCount"))
            .expect("base fraud condition");

        assert_eq!(rows[fraud_condition].1, MergeSideLineTone::Replaced);
        assert_eq!(
            rows[fraud_condition + 1],
            ("    return \"review\";", MergeSideLineTone::Replaced),
            "the later remote review return must not be matched to the replaced fraud branch"
        );
    }

    #[test]
    fn delete_versus_edit_marks_the_whole_conflict_block_as_mixed() {
        let document = three_way_merge(
            "start\n  if (isVip && cartTotal <= vipAutoApproveLimit(policy)) {\n    return \"approve\";\n  }\nend\n",
            "start\nend\n",
            "start\n  if (isVip && cartTotal < vipAutoApproveLimit(policy)) {\n    return \"approve\";\n  }\nend\n",
        );

        assert_eq!(document.conflicts().len(), 1);
        let rows = merge_result_display_rows(&document)
            .into_iter()
            .filter(|row| row.conflict_index == Some(0))
            .map(|row| (row.text, row.tone))
            .collect::<Vec<_>>();

        assert_eq!(
            rows,
            vec![
                (
                    "  if (isVip && cartTotal <= vipAutoApproveLimit(policy)) {",
                    MergeSideLineTone::LocalDeletedRemoteEdited,
                ),
                (
                    "    return \"approve\";",
                    MergeSideLineTone::LocalDeletedRemoteEdited,
                ),
                ("  }", MergeSideLineTone::LocalDeletedRemoteEdited),
            ],
        );
    }

    #[test]
    fn edit_versus_delete_keeps_the_opposite_direction_for_status_badges() {
        let document = three_way_merge(
            "start\n  if (isVip && total <= limit) {\n    return \"approve\";\n  }\nend\n",
            "start\n  if (isVip && total < limit) {\n    return \"approve\";\n  }\nend\n",
            "start\nend\n",
        );
        let tones = merge_result_display_rows(&document)
            .into_iter()
            .filter(|row| row.conflict_index == Some(0))
            .map(|row| row.tone)
            .collect::<Vec<_>>();

        assert!(
            tones
                .iter()
                .all(|tone| *tone == MergeSideLineTone::LocalEditedRemoteDeleted)
        );
    }

    #[test]
    fn delete_edit_result_uses_directional_badges_without_changing_solid_fill() {
        for theme in [MergeTheme::Light, MergeTheme::Dark] {
            let palette = merge_palette(theme);
            assert_eq!(
                merge_result_row_fill(MergeSideLineTone::LocalDeletedRemoteEdited, false, palette,),
                palette.conflict_fill,
            );
            assert_eq!(
                result_side_status_pair(MergeSideLineTone::LocalDeletedRemoteEdited),
                Some((
                    MergeResultSideStatus::Deleted,
                    MergeResultSideStatus::Edited,
                )),
            );
            assert_eq!(
                result_side_status_pair(MergeSideLineTone::LocalEditedRemoteDeleted),
                Some((
                    MergeResultSideStatus::Edited,
                    MergeResultSideStatus::Deleted,
                )),
            );
        }
    }

    #[test]
    fn unresolved_result_rows_keep_side_text_for_word_highlighting() {
        let document = three_way_merge(
            "keep\nruntime_mode = base\nendpoint = /v1/base\nend\n",
            "keep\nruntime_mode = local\nendpoint = /v1/local\nend\n",
            "keep\nruntime_mode = remote\nendpoint = /v2/remote\nend\n",
        );

        let rows = merge_result_display_rows(&document)
            .iter()
            .filter(|row| row.conflict_index == Some(0))
            .map(|row| (row.text, row.reference_text))
            .collect::<Vec<_>>();

        assert_eq!(
            rows,
            vec![
                ("runtime_mode = base", Some("runtime_mode = local")),
                ("endpoint = /v1/base", Some("endpoint = /v1/local")),
            ]
        );
    }

    #[test]
    fn one_sided_base_deletions_render_as_base_only_rows() {
        let document = three_way_merge(
            "keep\nshared before\n.claude/*\n.agents/*\nbase replaced\nend\n",
            "keep\nshared before\nlocal replacement\nend\n",
            "keep\nshared before\n.claude/*\n.agents/*\nremote replacement\nend\n",
        );

        assert_eq!(document.conflicts().len(), 1);
        let result_rows = merge_result_display_rows(&document)
            .iter()
            .filter(|row| row.conflict_index == Some(0))
            .map(|row| (row.text, row.tone))
            .collect::<Vec<_>>();

        assert_eq!(
            result_rows,
            vec![
                (".claude/*", MergeSideLineTone::BaseOnly),
                (".agents/*", MergeSideLineTone::BaseOnly),
                ("base replaced", MergeSideLineTone::Replaced),
            ]
        );
    }

    #[test]
    fn auto_resolved_one_sided_deletions_remain_visible_as_base_only_rows() {
        let document = three_way_merge(
            "keep\n.claude/*\n.agents/*\nend\n",
            "keep\nend\n",
            "keep\n.claude/*\n.agents/*\nend\n",
        );

        assert!(document.conflicts().is_empty());
        assert_eq!(document.result_text(), "keep\nend\n");

        let result_rows = merge_result_display_rows(&document)
            .iter()
            .map(|row| (row.text, row.tone))
            .collect::<Vec<_>>();

        assert_eq!(
            result_rows,
            vec![
                ("keep", MergeSideLineTone::Unchanged),
                (".claude/*", MergeSideLineTone::BaseOnly),
                (".agents/*", MergeSideLineTone::BaseOnly),
                ("end", MergeSideLineTone::Unchanged),
            ]
        );
    }

    #[test]
    fn complex_auto_deleted_base_rows_remain_in_result_display_before_replacement_block() {
        let document = three_way_merge(
            "section: agent files\n.vscode/*\n.cursor/*\n.claude/*\n.agents/*\n!.vscode/extensions.json\n.idea\nvite.config.mts.*.mjs\nnil\nclaude.md\n.codegraph/*\n.mcp.json\nAGENTS.md\n.codex/*\nend\n",
            "section: agent files\n.vscode/*\n.cursor/*\n!.vscode/extensions.json\n.idea\nvite.config.mts.*.mjs\nnil\nCLAUDE.local.md\n.codegraph-local/*\nAGENTS.local.md\n.codex-local/*\nend\n",
            "section: agent files\n.vscode/*\n.cursor/*\n.claude/*\n.agents/*\n!.vscode/extensions.json\n.idea\nvite.config.mts.*.mjs\nnil\n**/graphify-out/cache/\n**/graphify-out/cost.json\nend\n",
        );

        assert!(!document.result_text().contains(".claude/*"));
        assert!(!document.result_text().contains(".agents/*"));

        let rows = merge_result_display_rows(&document)
            .iter()
            .map(|row| (row.text, row.tone))
            .collect::<Vec<_>>();
        let start = rows
            .iter()
            .position(|row| row.0 == ".vscode/*")
            .expect("agent section start");

        assert_eq!(
            &rows[start..start + 12],
            &[
                (".vscode/*", MergeSideLineTone::Unchanged),
                (".cursor/*", MergeSideLineTone::Unchanged),
                (".claude/*", MergeSideLineTone::BaseOnly),
                (".agents/*", MergeSideLineTone::BaseOnly),
                ("!.vscode/extensions.json", MergeSideLineTone::Unchanged),
                (".idea", MergeSideLineTone::Unchanged),
                ("vite.config.mts.*.mjs", MergeSideLineTone::Unchanged),
                ("nil", MergeSideLineTone::Unchanged),
                ("claude.md", MergeSideLineTone::LocalDeletedRemoteEdited,),
                (".codegraph/*", MergeSideLineTone::Replaced),
                (".mcp.json", MergeSideLineTone::LocalDeletedRemoteEdited,),
                ("AGENTS.md", MergeSideLineTone::Replaced),
            ]
        );
    }

    #[test]
    fn base_only_side_rows_mark_missing_side_with_gap_rows() {
        let document = three_way_merge(
            "keep\n.claude/*\n.agents/*\nend\n",
            "keep\nend\n",
            "keep\n.claude/*\n.agents/*\nend\n",
        );

        let local_rows = merge_side_display_rows(&document, MergeSide::Local)
            .iter()
            .map(|row| {
                (
                    row.text,
                    row.tone,
                    row.line_number,
                    row.show_conflict_actions,
                    row.action_target,
                )
            })
            .collect::<Vec<_>>();
        let remote_rows = merge_side_display_rows(&document, MergeSide::Remote)
            .iter()
            .map(|row| (row.text, row.tone, row.line_number))
            .collect::<Vec<_>>();

        assert_eq!(
            local_rows,
            vec![
                ("keep", MergeSideLineTone::Unchanged, Some(1), false, None),
                ("end", MergeSideLineTone::Unchanged, Some(2), false, None),
            ]
        );
        assert_eq!(
            remote_rows,
            vec![
                ("keep", MergeSideLineTone::Unchanged, Some(1)),
                (".claude/*", MergeSideLineTone::Unchanged, Some(2)),
                (".agents/*", MergeSideLineTone::Unchanged, Some(3)),
                ("end", MergeSideLineTone::Unchanged, Some(4)),
            ]
        );
    }

    #[test]
    fn base_only_marker_is_overlay_not_a_side_display_row() {
        let document = three_way_merge(
            "keep\n.claude/*\n.agents/*\nend\n",
            "keep\nend\n",
            "keep\n.claude/*\n.agents/*\nend\n",
        );
        let local_rows = merge_side_display_rows(&document, MergeSide::Local);

        assert!(
            !local_rows
                .iter()
                .any(|row| row.tone == MergeSideLineTone::BaseOnly && row.text.is_empty())
        );
        assert_eq!(
            local_rows.iter().map(|row| row.text).collect::<Vec<_>>(),
            vec!["keep", "end"]
        );
    }

    #[test]
    fn base_only_markers_use_overlay_geometry_without_display_spacers() {
        let source = include_str!("merge_tool.rs");
        let implementation = source
            .split("#[cfg(test)]")
            .next()
            .expect("implementation section");

        assert!(!implementation.contains("base_only_gap_rows"));
        assert!(implementation.contains("paint_base_only_side_overlays"));
        assert!(implementation.contains("MERGE_BASE_ONLY_MARKER_HEIGHT"));
    }

    #[test]
    fn base_only_group_actions_keep_or_restore_deleted_base_rows() {
        let mut document = three_way_merge(
            "keep\n.claude/*\n.agents/*\nend\n",
            "keep\nend\n",
            "keep\n.claude/*\n.agents/*\nend\n",
        );

        assert_eq!(document.result_text(), "keep\nend\n");

        document.drop_base_only_group(1, MergeSide::Local);
        assert_eq!(document.result_text(), "keep\n.claude/*\n.agents/*\nend\n");

        let mut document = three_way_merge(
            "keep\n.claude/*\n.agents/*\nend\n",
            "keep\nend\n",
            "keep\n.claude/*\n.agents/*\nend\n",
        );
        document.take_base_only_group(1, MergeSide::Local);
        assert_eq!(document.result_text(), "keep\nend\n");
    }

    #[test]
    fn undoing_base_only_action_restores_all_merge_display_rows() {
        let document = three_way_merge(
            "keep\nremoved line\nend\n",
            "keep\nend\n",
            "keep\nremoved line\nend\n",
        );
        let mut app = MergeToolApp::new(test_merge_args(), document);
        let initial_result_rows = app.manual_result_lines.clone();
        let initial_local_rows = app.local_display_rows.clone();
        let initial_remote_rows = app.remote_display_rows.clone();
        let initial_epoch = app.display_epoch;

        app.apply_line_action(
            MergeLineActionTarget::BaseOnlyGroup(1),
            MergeSide::Local,
            MergeLineAction::Take,
        );
        assert_ne!(app.manual_result_lines, initial_result_rows);
        assert_ne!(app.display_epoch, initial_epoch);

        assert!(app.undo());
        assert_eq!(app.manual_result_lines, initial_result_rows);
        assert_eq!(app.local_display_rows, initial_local_rows);
        assert_eq!(app.remote_display_rows, initial_remote_rows);
        assert!(app.display_epoch > initial_epoch);
    }

    #[test]
    fn taking_base_only_group_hides_result_rows_and_missing_side_marker() {
        let mut document = three_way_merge(
            "keep\n.claude/*\n.agents/*\nend\n",
            "keep\nend\n",
            "keep\n.claude/*\n.agents/*\nend\n",
        );

        assert!(
            merge_result_display_lines(&document)
                .iter()
                .any(|line| *line == ".claude/*")
        );
        assert!(
            base_only_display_groups(&document)
                .iter()
                .any(|group| group.missing_side == MergeSide::Local)
        );

        document.take_base_only_group(1, MergeSide::Local);

        assert_eq!(document.result_text(), "keep\nend\n");
        assert!(
            !merge_result_display_lines(&document)
                .iter()
                .any(|line| *line == ".claude/*" || *line == ".agents/*")
        );
        assert!(
            !base_only_display_groups(&document)
                .iter()
                .any(|group| group.missing_side == MergeSide::Local)
        );
        assert!(
            merge_side_display_rows(&document, MergeSide::Remote)
                .iter()
                .any(|row| row.text == ".claude/*" && row.tone == MergeSideLineTone::Unchanged)
        );
    }

    #[test]
    fn base_only_connector_rects_join_missing_marker_to_result_rows() {
        let document = three_way_merge(
            "keep\n.claude/*\n.agents/*\nend\n",
            "keep\nend\n",
            "keep\n.claude/*\n.agents/*\nend\n",
        );
        let group = base_only_display_groups(&document)
            .into_iter()
            .next()
            .expect("base-only deletion group");
        let result_panel = Rect::from_min_size(Pos2::new(100.0, 40.0), Vec2::new(360.0, 640.0));
        let side_panel = Rect::from_min_size(Pos2::new(20.0, 40.0), Vec2::new(260.0, 640.0));

        assert_eq!(group.line_index, 1);
        assert_eq!(group.line_count, 2);
        assert_eq!(group.missing_side, MergeSide::Local);

        let result_rect =
            merge_base_only_result_rect(result_panel, &document, group, 0.0).expect("result rect");
        let side_rect =
            merge_base_only_side_rect(side_panel, &document, group, 0.0).expect("side rect");
        let content_top = merge_scroll_content_top(result_panel);

        assert_eq!(result_rect.top(), content_top + MERGE_CODE_ROW_HEIGHT);
        assert_eq!(
            result_rect.bottom(),
            content_top + MERGE_CODE_ROW_HEIGHT * 3.0
        );
        assert_eq!(side_rect.height(), MERGE_BASE_ONLY_MARKER_HEIGHT);
        assert_eq!(
            side_rect.top(),
            merge_scroll_content_top(side_panel) + MERGE_CODE_ROW_HEIGHT
                - MERGE_BASE_ONLY_MARKER_HEIGHT * 0.5
        );
        assert_eq!(
            side_rect.left(),
            merge_scroll_clip_rect(side_panel).left() + 58.0
        );
        assert_eq!(
            side_rect.right(),
            merge_scroll_clip_rect(side_panel).right() - 8.0
        );
    }

    #[test]
    fn connector_bridge_fills_panel_padding_and_slopes_only_between_columns() {
        let result_rect = Rect::from_min_max(Pos2::new(100.0, 40.0), Pos2::new(260.0, 76.0));
        let local_marker = Rect::from_min_max(Pos2::new(20.0, 50.0), Pos2::new(80.0, 53.0));
        let result_column = Rect::from_min_max(Pos2::new(95.0, 0.0), Pos2::new(265.0, 100.0));
        let local_column = Rect::from_min_max(Pos2::new(10.0, 0.0), Pos2::new(85.0, 100.0));

        assert_eq!(
            connector_bridge_x_positions(
                result_rect,
                local_marker,
                result_column,
                local_column,
                MergeSide::Local,
            ),
            (100.0, 85.0, 80.0, 80.0),
        );
        assert_eq!(
            connector_gap_points(
                result_rect,
                local_marker,
                result_column,
                local_column,
                MergeSide::Local,
            ),
            vec![
                Pos2::new(85.0, 40.0),
                Pos2::new(80.0, 50.0),
                Pos2::new(80.0, 53.0),
                Pos2::new(85.0, 76.0),
            ]
        );
        let remote_marker = Rect::from_min_max(Pos2::new(280.0, 50.0), Pos2::new(340.0, 53.0));
        let remote_column = Rect::from_min_max(Pos2::new(275.0, 0.0), Pos2::new(350.0, 100.0));
        assert_eq!(
            connector_bridge_x_positions(
                result_rect,
                remote_marker,
                result_column,
                remote_column,
                MergeSide::Remote,
            ),
            (260.0, 275.0, 280.0, 280.0),
        );
        assert_eq!(
            connector_gap_points(
                result_rect,
                remote_marker,
                result_column,
                remote_column,
                MergeSide::Remote,
            ),
            vec![
                Pos2::new(275.0, 40.0),
                Pos2::new(280.0, 50.0),
                Pos2::new(280.0, 53.0),
                Pos2::new(275.0, 76.0),
            ]
        );
    }

    #[test]
    fn remote_base_only_connector_rect_joins_missing_marker_to_result_rows() {
        let document = three_way_merge(
            "keep\n.claude/*\n.agents/*\nend\n",
            "keep\n.claude/*\n.agents/*\nend\n",
            "keep\nend\n",
        );
        let group = base_only_display_groups(&document)
            .into_iter()
            .next()
            .expect("base-only deletion group");
        let side_panel = Rect::from_min_size(Pos2::new(500.0, 40.0), Vec2::new(260.0, 640.0));

        assert_eq!(group.missing_side, MergeSide::Remote);

        let side_rect =
            merge_base_only_side_rect(side_panel, &document, group, 0.0).expect("side rect");

        assert_eq!(side_rect.height(), MERGE_BASE_ONLY_MARKER_HEIGHT);
        assert_eq!(
            side_rect.left(),
            merge_scroll_clip_rect(side_panel).left() + 58.0
        );
    }

    #[test]
    fn side_scroll_offset_maps_from_result_visible_rows() {
        let document = three_way_merge(
            "keep\n.claude/*\n.agents/*\nend\n",
            "keep\nend\n",
            "keep\n.claude/*\n.agents/*\nend\n",
        );
        let result_scroll_y = MERGE_CODE_ROW_HEIGHT * 3.0;
        let compressed_side_scroll_y = MERGE_CODE_ROW_HEIGHT;

        assert_eq!(
            merge_side_scroll_y_for_result_scroll(&document, MergeSide::Local, result_scroll_y),
            compressed_side_scroll_y
        );
        assert_eq!(
            merge_side_scroll_y_for_result_scroll(&document, MergeSide::Remote, result_scroll_y),
            result_scroll_y
        );
        assert_eq!(
            merge_result_scroll_y_for_side_scroll(
                &document,
                MergeSide::Local,
                compressed_side_scroll_y
            ),
            compressed_side_scroll_y
        );
    }

    #[test]
    fn side_rows_after_base_only_gap_keep_result_row_alignment() {
        let document = three_way_merge(
            "keep\n.claude/*\n.agents/*\nend\n",
            "keep\nend\n",
            "keep\n.claude/*\n.agents/*\nend\n",
        );
        let end_index = document
            .lines
            .iter()
            .position(|line| line.result == "end")
            .expect("end line");

        assert_eq!(
            merge_result_display_row_for_line(&document, end_index),
            Some(3)
        );
        assert_eq!(
            merge_side_display_row_for_line(&document, MergeSide::Local, end_index),
            Some(1)
        );
        assert_eq!(
            merge_side_display_row_for_line(&document, MergeSide::Remote, end_index),
            Some(3)
        );
    }

    #[test]
    fn base_only_gap_lines_keep_distinct_scroll_anchors() {
        let document = three_way_merge(
            "keep\n.claude/*\n.agents/*\nend\n",
            "keep\nend\n",
            "keep\n.claude/*\n.agents/*\nend\n",
        );

        assert_eq!(merge_result_display_row_for_line(&document, 1), Some(1));
        assert_eq!(merge_result_display_row_for_line(&document, 2), Some(2));
        assert_eq!(
            merge_side_display_row_for_line(&document, MergeSide::Local, 1),
            Some(1)
        );
        assert_eq!(
            merge_side_display_row_for_line(&document, MergeSide::Local, 2),
            Some(1)
        );
        assert_eq!(
            merge_side_scroll_y_for_result_scroll(
                &document,
                MergeSide::Local,
                MERGE_CODE_ROW_HEIGHT * 2.0
            ),
            MERGE_CODE_ROW_HEIGHT
        );
    }

    #[test]
    fn base_only_gap_marker_does_not_consume_side_row_height() {
        let document = three_way_merge(
            "keep\n.claude/*\n.agents/*\nend\n",
            "keep\nend\n",
            "keep\n.claude/*\n.agents/*\nend\n",
        );
        let local_rows = merge_side_display_rows(&document, MergeSide::Local);

        assert!(
            !local_rows
                .iter()
                .any(|row| row.tone == MergeSideLineTone::BaseOnly && row.text.is_empty())
        );
        assert_eq!(
            merge_side_display_row_for_line(&document, MergeSide::Local, 3),
            Some(1)
        );
    }

    #[test]
    fn first_unchanged_line_after_base_only_gap_stays_scroll_aligned() {
        let document = three_way_merge(
            "section: agent files\n.vscode/*\n.cursor/*\n.claude/*\n.agents/*\n!.vscode/extensions.json\n.idea\nvite.config.mts.*.mjs\nnil\nclaude.md\n.codegraph/*\n.mcp.json\nAGENTS.md\n.codex/*\nend\n",
            "section: agent files\n.vscode/*\n.cursor/*\n!.vscode/extensions.json\n.idea\nvite.config.mts.*.mjs\nnil\nCLAUDE.local.md\n.codegraph-local/*\nAGENTS.local.md\n.codex-local/*\nend\n",
            "section: agent files\n.vscode/*\n.cursor/*\n.claude/*\n.agents/*\n!.vscode/extensions.json\n.idea\nvite.config.mts.*.mjs\nnil\n**/graphify-out/cache/\n**/graphify-out/cost.json\nend\n",
        );
        let line_index = document
            .lines
            .iter()
            .position(|line| line.result == "!.vscode/extensions.json")
            .expect("first shared line after base-only gap");
        let result_row = merge_result_display_row_for_line(&document, line_index);

        assert_eq!(result_row, Some(5));
        assert_eq!(
            merge_side_display_row_for_line(&document, MergeSide::Local, line_index),
            Some(3)
        );
        assert_eq!(
            result_row,
            merge_side_display_row_for_line(&document, MergeSide::Remote, line_index)
        );
        let scroll_y = result_row.expect("result row") as f32 * MERGE_CODE_ROW_HEIGHT;
        assert_eq!(
            merge_side_scroll_y_for_result_scroll(&document, MergeSide::Local, scroll_y),
            MERGE_CODE_ROW_HEIGHT * 3.0
        );
        let fractional_scroll_y = scroll_y - MERGE_CODE_ROW_HEIGHT * 0.5;
        assert_eq!(
            merge_side_scroll_y_for_result_scroll(&document, MergeSide::Local, fractional_scroll_y),
            MERGE_CODE_ROW_HEIGHT * 3.0
        );
    }

    #[test]
    fn side_connector_rect_uses_side_display_rows() {
        let document = three_way_merge(
            "keep\nbuild/\ncache/\nend\n",
            "keep\ndist-local/\ncache-local/\nend\n",
            "keep\nrelease-dist/\ngraph-cache/\nend\n",
        );
        let panel = Rect::from_min_size(Pos2::new(20.0, 40.0), Vec2::new(260.0, 420.0));
        let rect = merge_block_side_rect(
            panel,
            &document,
            &document.conflicts()[0],
            MergeSide::Local,
            0.0,
        )
        .expect("side connector rect");
        let top = merge_scroll_content_top(panel) + MERGE_CODE_ROW_HEIGHT;

        assert_eq!(rect.top(), top);
        assert_eq!(rect.bottom(), top + MERGE_CODE_ROW_HEIGHT * 2.0);
    }

    #[test]
    fn bridge_uses_inner_row_edges_and_outer_column_edges() {
        let source = include_str!("merge_tool.rs");
        let implementation = source
            .split("fn paint_side_block_bridge")
            .nth(1)
            .and_then(|tail| tail.split("fn merge_connector_color").next())
            .expect("bridge implementation");

        assert!(implementation.contains("paint_connector_endpoint_extension"));
        assert!(implementation.contains("Pos2::new(result_edge_x, result_rect.top())"));
        assert!(implementation.contains("Pos2::new(side_edge_x, side_rect.top())"));
        assert!(!implementation.contains("fn merge_connector_side_y("));
        assert!(!implementation.contains("fn merge_connector_side_bottom_y("));
    }

    #[test]
    fn base_only_marker_uses_boundary_anchor_not_block_bridge() {
        let source = include_str!("merge_tool.rs");
        let base_only_section = source
            .split("fn paint_merge_block_connectors")
            .nth(1)
            .and_then(|tail| tail.split("fn merge_connector_debug_mode").next())
            .and_then(|tail| tail.split("for cached in &cache.base_only_groups").nth(1))
            .expect("base-only connector loop");

        assert!(base_only_section.contains("paint_base_only_marker_bridge"));
        assert!(!base_only_section.contains("paint_side_block_bridge"));
    }

    #[test]
    fn cached_merge_geometry_matches_dynamic_document_mapping() {
        let document = three_way_merge(
            "start\nremove locally\nstable\nbase conflict\nend\n",
            "start\nstable\nlocal conflict\nend\n",
            "start\nremove locally\nstable\nremote conflict\nend\n",
        );
        let result_rows = merge_result_display_rows(&document);
        let local_rows = cached_merge_side_display_rows(&document, MergeSide::Local);
        let remote_rows = cached_merge_side_display_rows(&document, MergeSide::Remote);
        let cached_result_rows = cached_merge_result_display_rows(&result_rows);
        let cache = merge_geometry_cache(&document, &cached_result_rows, &local_rows, &remote_rows);

        assert!(!cache.base_only_groups.is_empty());
        for cached in &cache.base_only_groups {
            assert_eq!(
                Some(cached.result_row),
                merge_result_display_row_for_line(&document, cached.group.line_index)
            );
            assert_eq!(
                Some(cached.side_boundary_row),
                merge_side_display_row_for_line(
                    &document,
                    cached.group.missing_side,
                    cached.group.line_index,
                )
            );
        }

        for conflict in document.conflicts() {
            let cached = cache.conflicts.get(&conflict.index).unwrap();
            assert_eq!(
                cached.result_span,
                merge_result_row_span_for_conflict(&document, conflict)
            );
            assert_eq!(
                cached.local_span,
                merge_side_row_span_for_conflict(&document, MergeSide::Local, conflict)
            );
            assert_eq!(
                cached.remote_span,
                merge_side_row_span_for_conflict(&document, MergeSide::Remote, conflict)
            );
            assert_eq!(cached.tone, merge_block_connector_tone(&document, conflict));
        }
    }

    #[test]
    fn frame_hot_paths_use_precomputed_merge_geometry() {
        let source = include_str!("merge_tool.rs");
        for function in [
            "fn paint_base_only_side_overlays",
            "fn paint_merge_block_connectors",
            "fn merge_ai_suggestion_overlays",
            "fn merge_editor_columns",
            "fn merge_side_panel",
        ] {
            let implementation = source
                .split(function)
                .nth(1)
                .and_then(|tail| tail.split("\nfn ").next())
                .unwrap();
            assert!(!implementation.contains("base_only_display_groups("));
            assert!(!implementation.contains("merge_side_display_row_for_line("));
            assert!(!implementation.contains("merge_side_display_rows("));
            assert!(!implementation.contains("merge_diff_base_to_side("));
            assert!(!implementation.contains("merge_side_scroll_y_for_result_scroll("));
            assert!(!implementation.contains("merge_result_scroll_y_for_side_scroll("));
        }
    }

    #[test]
    fn connector_debug_mode_uses_build_time_configuration() {
        assert_eq!(
            merge_build_config_bool("connector_guides"),
            MERGE_BUILD_CONFIG.contains("connector_guides = true")
        );
    }

    #[test]
    fn connector_debug_mode_is_read_from_build_config() {
        let source = include_str!("merge_tool.rs");
        let shortcuts = source
            .split("fn handle_keyboard_shortcuts")
            .nth(1)
            .and_then(|tail| tail.split("fn handle_close_request").next())
            .expect("merge shortcut implementation");
        assert!(source.contains("connector_debug: MergeConnectorDebug"));
        assert!(source.contains("include_str!(\"../config/merge-tool.toml\")"));
        assert!(source.contains("merge_build_config_bool(\"connector_guides\")"));
        assert!(!shortcuts.contains("Key::D"));
    }

    #[test]
    fn connector_guides_outline_complete_result_and_side_rectangles() {
        let source = include_str!("merge_tool.rs");
        let implementation = source
            .split("fn paint_side_block_debug")
            .nth(1)
            .and_then(|tail| tail.split("fn merge_connector_color").next())
            .expect("connector debug painter");

        assert_eq!(implementation.matches("painter.rect_stroke(").count(), 2);
        assert!(implementation.contains("egui::StrokeKind::Inside"));
        assert!(!implementation.contains("side_rect.left_top()"));
        assert!(!implementation.contains("result_rect.left_bottom()"));
    }

    #[test]
    fn conflict_action_buttons_are_limited_to_block_start() {
        let document = three_way_merge(
            "keep\nbuild/\ncache/\nend\n",
            "keep\ndist-local/\ncache-local/\nend\n",
            "keep\nrelease-dist/\ngraph-cache/\nend\n",
        );

        let flags = merge_side_display_rows(&document, MergeSide::Remote)
            .iter()
            .filter(|row| row.conflict_index == Some(0))
            .map(|row| row.show_conflict_actions)
            .collect::<Vec<_>>();

        assert_eq!(flags, vec![true, false]);
    }

    #[test]
    fn deleted_side_display_rows_do_not_take_side_line_numbers() {
        let document = three_way_merge(
            "keep\nbuild/\ncache/\nend\n",
            "keep\ndist-local/\ncache-local/\nend\n",
            "keep\nrelease-dist/\ngraph-cache/\nend\n",
        );

        let line_numbers = merge_side_display_rows(&document, MergeSide::Local)
            .iter()
            .filter(|row| row.conflict_index == Some(0))
            .map(|row| (row.tone, row.line_number))
            .collect::<Vec<_>>();

        assert_eq!(
            line_numbers,
            vec![
                (MergeSideLineTone::Replaced, Some(2)),
                (MergeSideLineTone::Replaced, Some(3)),
            ]
        );
    }

    #[test]
    fn merge_tool_scroll_and_spacing_are_shared_and_dense() {
        let source = include_str!("merge_tool.rs");
        let implementation = source
            .split("#[cfg(test)]")
            .next()
            .expect("implementation section");

        assert!(implementation.contains("shared_scroll_y"));
        assert!(implementation.contains(".vertical_scroll_offset(scroll_y)"));
        assert!(implementation.contains("item_spacing.y = 0.0"));
        assert!(implementation.contains("MERGE_CODE_ROW_HEIGHT"));
        assert!(!implementation.contains("let row_h = 22.0"));
        assert!(!implementation.contains("ui.add_space(30.0)"));

        for panel_name in ["fn merge_side_panel(", "fn merge_result_panel("] {
            let panel = implementation
                .split(panel_name)
                .nth(1)
                .unwrap_or_else(|| panic!("missing {panel_name}"));
            let spacing = panel
                .find("item_spacing.y = 0.0")
                .unwrap_or_else(|| panic!("{panel_name} must set dense row spacing"));
            let show_rows = panel
                .find(".show_rows(")
                .unwrap_or_else(|| panic!("{panel_name} must use virtual rows"));
            assert!(
                spacing < show_rows,
                "{panel_name} must set spacing before show_rows calculates virtual offsets"
            );
        }
    }

    #[test]
    fn remote_conflict_actions_put_take_before_drop() {
        let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(240.0, 18.0));

        let local = conflict_action_rects(rect, MergeSide::Local);
        assert!(local.drop.left() < local.take.left());
        assert_eq!(local.drop.top(), rect.top());
        assert_eq!(local.drop.bottom(), rect.bottom());

        let remote = conflict_action_rects(rect, MergeSide::Remote);
        assert!(remote.take.left() < remote.drop.left());
        assert_eq!(remote.take.top(), rect.top());
        assert_eq!(remote.take.bottom(), rect.bottom());
    }

    #[test]
    fn result_panel_paints_conflict_connectors() {
        let source = include_str!("merge_tool.rs");
        let implementation = source
            .split("mod tests")
            .next()
            .expect("implementation section");

        assert!(implementation.contains("paint_merge_block_connectors"));
        assert!(implementation.contains("merge_block_result_rect"));
        assert!(!implementation.contains("fn paint_result_connector("));
        assert!(!implementation.contains("fn paint_side_connector("));
        assert!(!implementation.contains("palette.connector.gamma_multiply(0.10)"));
        assert!(implementation.contains("Shape::convex_polygon"));
        assert!(implementation.contains("merge_block_connector_tone"));
        assert!(implementation.contains("paint_result_block_outline(ui, result_rect, tone"));
    }

    #[test]
    fn merge_connectors_use_real_row_geometry_for_current_frame() {
        let source = include_str!("merge_tool.rs");
        let implementation = source
            .split("fn merge_editor_columns")
            .nth(1)
            .and_then(|tail| tail.split("fn merge_side_panel").next())
            .expect("merge editor columns implementation");

        assert!(implementation.contains("let requested_scroll_y = app.shared_scroll_y;"));
        assert!(implementation.contains("let frame_scroll_y = result_output.scroll_y;"));
        assert!(implementation.contains("&local_output.geometry"));
        assert!(implementation.contains("&result_output.geometry"));
        assert!(implementation.contains("&remote_output.geometry"));
        assert!(implementation.contains("MergeConnectorColumns"));
        assert!(implementation.contains("local: left"));
        assert!(implementation.contains("result,"));
        assert!(implementation.contains("remote: right"));
        assert!(implementation.contains("app.shared_scroll_y = next_shared_scroll_y;"));
    }
}
