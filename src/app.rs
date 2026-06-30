use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

use eframe::{
    App, CreationContext,
    egui::{
        self, Align, Align2, Color32, CornerRadius, FontId, Layout, Pos2, Rect, RichText,
        ScrollArea, Sense, Shape, Stroke, TextEdit, Ui, Vec2, epaint::CubicBezierShape,
    },
};
use serde::{Deserialize, Serialize};

use crate::{
    git::{
        self, Commit, CommitDetails, FileDiff, RepositorySnapshot, ResetMode, StashEntry, Tag,
        WorktreeFile,
    },
    graph::{self, EdgeKind, GraphLayout},
    i18n::{self, Language},
    theme,
};

const TITLE_BAR_HEIGHT: f32 = 32.0;
const TITLE_MENU_RESERVED_WIDTH: f32 = 420.0;
const TOP_BAR_TAB_TOOL_JOIN_OVERLAP: f32 = 6.0;
const TOP_BAR_HEIGHT: f32 =
    TITLE_BAR_HEIGHT + TOP_BAR_ROW_HEIGHT * 2.0 - TOP_BAR_TAB_TOOL_JOIN_OVERLAP;
const TOP_BAR_ROW_HEIGHT: f32 = 40.0;
const TOP_BAR_GLOBAL_WIDTH: f32 = 480.0;
const TOP_BAR_GLOBAL_ACTION_Y_OFFSET: f32 = -1.0;
const TOP_BAR_MIN_TABS_WIDTH: f32 = 420.0;
const TOP_BAR_PANEL_X_INSET: f32 = 8.0;
const REPO_TAB_STRIP_LEFT_PADDING: f32 = TOP_BAR_PANEL_X_INSET;
const REPO_TAB_ITEM_GAP: f32 = 6.0;
const REPO_TAB_HEIGHT: f32 = 28.0;
const REPO_TAB_PLUS_WIDTH: f32 = 34.0;
const REPO_TAB_OVERFLOW_WIDTH: f32 = 82.0;
const TOOLBAR_BUTTON_HEIGHT: f32 = 18.0;
const TOOLBAR_BUTTON_ICON: f32 = 13.0;
const TOOLBAR_BUTTON_TEXT: f32 = 11.0;
const TOOLBAR_BUTTON_X_PADDING: f32 = 36.0;
const TOOLBAR_BUTTON_MIN_WIDTH: f32 = 48.0;
const TOOLBAR_BUTTON_MAX_WIDTH: f32 = 160.0;
const TOOLBAR_DOUBLE_CLICK_DELAY: f64 = 0.28;
const FILE_ROW_HEIGHT: f32 = 24.0;
const FILE_ROW_ICON_SLOT: f32 = 24.0;
const FILE_ROW_LEFT_INSET: f32 = 10.0;
const BRANCH_CURRENT_BADGE_RIGHT_GAP: f32 = 4.0;
const BRANCH_CURRENT_BADGE_Y_OFFSET: f32 = 0.0;
const WORKSPACE_LIST_COMMIT_GAP: f32 = 2.0;
const WORKSPACE_HEADER_TOP_GAP: f32 = 4.0;
const WORKSPACE_HEADER_BOTTOM_GAP: f32 = 6.0;
const WORKSPACE_HEADER_TITLE_SIZE: f32 = 20.0;
const WORKSPACE_CARD_RADIUS: u8 = 6;
const WORKSPACE_CARD_SHADOW_PAD: f32 = 14.0;
const COMMIT_MESSAGE_EDITOR_MIN_HEIGHT: f32 = 34.0;
const COMMIT_BUTTON_ROW_HEIGHT: f32 = 30.0;
const COMMIT_MESSAGE_BOTTOM_GAP: f32 = 4.0;
const COMMIT_SUBMIT_BUTTON_SIZE: Vec2 = Vec2 { x: 54.0, y: 24.0 };
const HISTORY_TABLE_HEADER_HEIGHT: f32 = 24.0;
const HISTORY_TABLE_ROW_HEIGHT: f32 = 22.0;
const HISTORY_REF_BADGE_MIN_WIDTH: f32 = 34.0;
const HISTORY_REF_BADGE_MAX_WIDTH: f32 = 142.0;
const HISTORY_REF_BADGE_X_PADDING: f32 = 8.0;
const HISTORY_REF_BADGE_GAP: f32 = 6.0;
const HISTORY_BOTTOM_MIN_HEIGHT: f32 = 230.0;
const HISTORY_DETAILS_MIN_HEIGHT: f32 = 260.0;
const HISTORY_LIST_MIN_HEIGHT: f32 = 260.0;
const FILE_SEARCH_TIMEOUT: Duration = Duration::from_secs(30);
const CONTENT_PANEL_INSET_X: i8 = 14;
const CONTENT_PANEL_INSET_Y: i8 = 12;
const RESOURCE_ROW_HEIGHT: f32 = 30.0;
const RESOURCE_TABLE_HEADER_HEIGHT: f32 = 24.0;
const SETTINGS_DIALOG_WIDTH: f32 = 760.0;
const SETTINGS_DIALOG_HEIGHT: f32 = 580.0;
const REPO_SETTINGS_DIALOG_WIDTH: f32 = 700.0;
const REPO_SETTINGS_DIALOG_HEIGHT: f32 = 460.0;
const REPO_SETTINGS_REMOTE_DIALOG_WIDTH: f32 = 520.0;
const SETTINGS_DIALOG_TITLE_HEIGHT: f32 = 32.0;
const SETTINGS_DIALOG_TITLE_SIZE: f32 = 18.0;
const ACTION_DIALOG_WIDTH: f32 = 392.0;
const FETCH_DIALOG_WIDTH: f32 = 392.0;
const PULL_DIALOG_WIDTH: f32 = 700.0;
const PUSH_DIALOG_WIDTH: f32 = 790.0;
const PUSH_REMOTE_FORM_ROW_HEIGHT: f32 = 30.0;
const PUSH_REMOTE_FORM_LABEL_WIDTH: f32 = 72.0;
const PUSH_REMOTE_FORM_SELECTOR_WIDTH: f32 = 110.0;
const PUSH_REMOTE_FORM_CONTROL_HEIGHT: f32 = 26.0;
const PUSH_SELECT_COLUMN_WIDTH: f32 = 82.0;
const PUSH_LOCAL_BRANCH_COLUMN_WIDTH: f32 = 160.0;
const PUSH_TRACK_COLUMN_WIDTH: f32 = 58.0;
const PUSH_TABLE_COLUMN_GAP: f32 = 8.0;
const PUSH_TABLE_ROW_HEIGHT: f32 = 28.0;
const PUSH_TABLE_BODY_TEXT_Y_OFFSET: f32 = 3.0;
const ACTION_DIALOG_TITLE_HEIGHT: f32 = 34.0;
const ACTION_DIALOG_TITLE_SIZE: f32 = 16.0;
const REPO_SETTINGS_TABS_HEIGHT: f32 = 34.0;
const REPO_SETTINGS_TAB_WIDTH: f32 = 104.0;
const REPO_SETTINGS_TAB_HEIGHT: f32 = 28.0;
const SETTINGS_NAV_WIDTH: f32 = 190.0;
const SETTINGS_FOOTER_HEIGHT: f32 = 44.0;
const SETTINGS_REMOTE_ACCOUNT_INPUT_WIDTH: f32 = 172.0;
const LAYOUT_GAP: i8 = 8;
const RESIZE_HANDLE_THICKNESS: f32 = 8.0;

static MAIN_LAYOUT_DEBUG_LOGGED: AtomicBool = AtomicBool::new(false);
static WORKSPACE_LAYOUT_DEBUG_LOGGED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug)]
struct MainLayoutRects {
    content: Rect,
    sidebar: Rect,
    sidebar_center_gap: Rect,
    center: Rect,
    center_details_gap: Rect,
    details: Rect,
}

fn main_layout_rects(
    full: Rect,
    sidebar_pct: f32,
    details_pct: f32,
    details_visible: bool,
) -> MainLayoutRects {
    let gap = LAYOUT_GAP as f32;
    let content_top = full.top().round();
    let content_bottom = full.bottom().max(full.top()).round();
    let content = Rect::from_min_max(
        Pos2::new(full.left().round(), content_top),
        Pos2::new(full.right().round(), content_bottom),
    );
    let full_width = content.width();

    let mut sidebar_width = (full_width * sidebar_pct)
        .clamp(220.0, 340.0)
        .min(full_width * 0.34)
        .round();
    let mut details_width = if details_visible {
        (full_width * details_pct)
            .clamp(340.0, 640.0)
            .min(full_width * 0.46)
            .round()
    } else {
        0.0
    };
    let details_gap = if details_visible { gap } else { 0.0 };
    let min_center = 360.0;
    let min_sidebar = 200.0;
    let min_details = if details_visible { 320.0 } else { 0.0 };
    let spare = (full_width - gap - details_gap - min_center).max(0.0);
    if sidebar_width + details_width > spare {
        let overflow = sidebar_width + details_width - spare;
        if details_visible {
            details_width = (details_width - overflow).max(min_details);
        }
        if sidebar_width + details_width > spare {
            sidebar_width = (spare - details_width).max(min_sidebar);
        }
    }

    let sidebar = Rect::from_min_max(
        content.left_top(),
        Pos2::new(content.left() + sidebar_width, content.bottom()),
    );
    let details = if details_visible {
        Rect::from_min_max(
            Pos2::new(content.right() - details_width, content.top()),
            content.right_bottom(),
        )
    } else {
        Rect::from_min_size(Pos2::new(content.right(), content.top()), Vec2::ZERO)
    };
    let center_left = sidebar.right() + gap;
    let center_right = if details_visible {
        details.left() - gap
    } else {
        content.right()
    };
    let center = Rect::from_min_max(
        Pos2::new(center_left, content.top()),
        Pos2::new(center_right.max(center_left), content.bottom()),
    );
    let sidebar_center_gap = Rect::from_min_max(
        Pos2::new(sidebar.right(), content.top()),
        Pos2::new(center.left(), content.bottom()),
    );
    let center_details_gap = if details_visible {
        Rect::from_min_max(
            Pos2::new(center.right(), content.top()),
            Pos2::new(details.left(), content.bottom()),
        )
    } else {
        Rect::from_min_size(Pos2::new(center.right(), content.top()), Vec2::ZERO)
    };

    MainLayoutRects {
        content,
        sidebar,
        sidebar_center_gap,
        center,
        center_details_gap,
        details,
    }
}

fn central_panel_margin(source_active: bool) -> egui::Margin {
    egui::Margin {
        left: LAYOUT_GAP,
        right: LAYOUT_GAP,
        top: if source_active { 0 } else { LAYOUT_GAP },
        bottom: LAYOUT_GAP,
    }
}

fn repository_source_panel_y_margin() -> i8 {
    0
}

fn repo_tab_strip_rect(tab_row: Rect, source_active: bool) -> Rect {
    if source_active {
        Rect::from_min_max(
            Pos2::new(tab_row.left(), tab_row.bottom() - REPO_TAB_HEIGHT),
            tab_row.right_bottom(),
        )
    } else {
        tab_row
    }
}

fn top_island_rect(full: Rect, title_row: Rect, tool_row: Rect, source_active: bool) -> Rect {
    let bottom = if source_active {
        tool_row.bottom()
    } else {
        tool_row.bottom() - 4.0
    };
    Rect::from_min_max(
        Pos2::new(
            full.left() + TOP_BAR_PANEL_X_INSET,
            title_row.bottom() + 2.0,
        ),
        Pos2::new(full.right() - TOP_BAR_PANEL_X_INSET, bottom),
    )
}

fn custom_title_drag_rect(rect: Rect, controls_width: f32) -> Rect {
    let drag_left =
        (rect.left() + TITLE_MENU_RESERVED_WIDTH).min(rect.right() - controls_width - 24.0);
    Rect::from_min_max(
        Pos2::new(drag_left, rect.top()),
        Pos2::new(rect.right() - controls_width, rect.bottom()),
    )
}

fn layout_debug_enabled() -> bool {
    env::var("GIT_AGENT_LAYOUT_DEBUG")
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            !value.is_empty() && value != "0" && value != "false"
        })
        .unwrap_or(false)
}

fn paint_layout_debug_rect(ui: &Ui, rect: Rect, label: &str, color: Color32) {
    if !layout_debug_enabled() {
        return;
    }
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(2),
        Stroke::new(1.0, color),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.left_top() + Vec2::new(4.0, 4.0),
        Align2::LEFT_TOP,
        label,
        FontId::monospace(11.0),
        color,
    );
}

fn log_layout_debug_once(flag: &AtomicBool, label: &str, rects: &[(&str, Rect)]) {
    if !layout_debug_enabled() || flag.swap(true, Ordering::Relaxed) {
        return;
    }
    eprintln!("[layout-debug] {label}");
    for (name, rect) in rects {
        eprintln!(
            "[layout-debug] {name}: left={:.1} top={:.1} right={:.1} bottom={:.1} width={:.1} height={:.1}",
            rect.left(),
            rect.top(),
            rect.right(),
            rect.bottom(),
            rect.width(),
            rect.height()
        );
    }
}

pub struct GitAgentApp {
    repo_tabs: Vec<RepoTab>,
    active_repo_tab: Option<usize>,
    repo_tab_drag: RepoTabDragState,
    source_tab_open: bool,
    repo_source_tab: RepoSourceTab,
    snapshot: Option<RepositorySnapshot>,
    snapshot_cache: HashMap<String, RepositorySnapshot>,
    layout: GraphLayout,
    selected_commit: Option<usize>,
    error: Option<String>,
    search: String,
    search_view_query: String,
    search_selected_commit: Option<usize>,
    search_selected_file_path: Option<String>,
    search_selected_diff_rows: Vec<DiffLineKey>,
    search_diff_display_mode: DiffDisplayMode,
    repo_source_search: String,
    known_repositories: Vec<KnownRepository>,
    clone_url: String,
    clone_destination: String,
    create_repo_path: String,
    clone_url_status: CloneUrlStatus,
    clone_url_last_edited: Option<Instant>,
    clone_url_task: Option<Receiver<(String, anyhow::Result<()>)>>,
    search_dimension: SearchDimension,
    repo_task: Option<Receiver<RepoTaskResult>>,
    remote_git_task: Option<Receiver<RemoteGitTaskResult>>,
    branch_checkout_task: Option<Receiver<BranchCheckoutTaskResult>>,
    merge_tool_task: Option<Receiver<MergeToolTaskResult>>,
    repo_source_task: Option<Receiver<anyhow::Result<PathBuf>>>,
    details_task: Option<Receiver<anyhow::Result<CommitDetails>>>,
    diff_task: Option<Receiver<anyhow::Result<FileDiff>>>,
    file_search_task: Option<Receiver<(String, anyhow::Result<Vec<String>>)>>,
    file_search_started_at: Option<Instant>,
    file_search_query: String,
    file_search_hashes: HashSet<String>,
    details_cache: HashMap<String, CommitDetails>,
    diff_cache: HashMap<String, FileDiff>,
    selected_file_path: Option<String>,
    history_diff_display_mode: DiffDisplayMode,
    history_sort_order: HistorySortOrder,
    history_branch_scope: HistoryBranchScope,
    history_cherry_pick_mode: bool,
    selected_cherry_pick_hashes: HashSet<String>,
    selected_diff_rows: Vec<DiffLineKey>,
    history_rows_cache: HistoryRowsCache,
    selected_worktree_file: Option<SelectedWorktreeFile>,
    worktree_selection: WorktreeSelectionState,
    worktree_display_mode: WorktreeDisplayMode,
    worktree_collapsed_dirs: HashSet<String>,
    loading_repo: bool,
    loading_details_hash: Option<String>,
    loading_diff_key: Option<String>,
    pending_branch_checkout: Option<String>,
    pending_commit_action: Option<CommitActionDialog>,
    last_notice: Option<String>,
    toast_notice: Option<(String, Instant)>,
    pending_toolbar_single_click: Option<PendingToolbarClick>,
    pending_worktree_action: Option<WorktreeActionDialog>,
    pending_fetch_action: Option<FetchActionDialog>,
    pending_pull_action: Option<PullActionDialog>,
    pending_push_action: Option<PushActionDialog>,
    commit_message: String,
    commit_state: RepoCommitState,
    focus_commit_message: bool,
    language: Language,
    pending_stash_action: Option<StashActionDialog>,
    pending_branch_action: Option<BranchActionDialog>,
    pending_tag_action: Option<TagActionDialog>,
    active_view: MainView,
    branches_open: bool,
    tags_open: bool,
    remotes_open: bool,
    local_branch_collapsed_groups: HashSet<String>,
    remote_branch_collapsed_groups: HashSet<String>,
    stashes_open: bool,
    sidebar_tree_states: HashMap<String, SidebarTreeState>,
    settings_open: bool,
    settings_tab: SettingsTab,
    repo_settings_open: bool,
    repo_settings_tab: SettingsTab,
    pending_repo_remote_action: Option<RepoRemoteActionDialog>,
    remote_accounts: Vec<RemoteAccountSettings>,
    remote_account_name_input: String,
    remote_account_host_input: String,
    remote_account_error: Option<String>,
    theme_mode: theme::ThemeMode,
    theme_accent: theme::ThemeAccent,
    layout_prefs: LayoutPrefs,
    history_show_remote_refs: bool,
}

#[derive(Clone, Debug)]
struct RepoTab {
    root: PathBuf,
    name: String,
}

#[derive(Clone, Copy, Debug, Default)]
struct RepoTabDragState {
    dragging_index: Option<usize>,
}

#[derive(Clone, Debug)]
struct KnownRepository {
    root: PathBuf,
    name: String,
}

type RemoteGitTaskResult = (PathBuf, anyhow::Result<()>);
type BranchCheckoutTaskResult = (PathBuf, String, anyhow::Result<()>);
type MergeToolTaskResult = (PathBuf, anyhow::Result<bool>);
type RepoTaskResult = (PathBuf, anyhow::Result<RepositorySnapshot>);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct UpstreamSyncCounts {
    ahead: usize,
    behind: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct RepoCommitStateStore {
    repositories: BTreeMap<String, RepoCommitState>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct RepoCommitState {
    push_immediately: bool,
    amend: bool,
    no_verify: bool,
    gpg_sign: bool,
    message_history: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct RepoTabsState {
    tabs: Vec<String>,
    active_repo_tab: Option<usize>,
    source_tab_open: bool,
    source_tab_active: bool,
    sidebar_tree_states: HashMap<String, SidebarTreeState>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default)]
struct SidebarTreeState {
    branches_open: bool,
    tags_open: bool,
    remotes_open: bool,
    stashes_open: bool,
}

impl Default for SidebarTreeState {
    fn default() -> Self {
        Self {
            branches_open: true,
            tags_open: false,
            remotes_open: false,
            stashes_open: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CloneUrlStatus {
    Empty,
    Pending,
    Checking,
    Valid,
    Invalid(String),
}

impl Default for CloneUrlStatus {
    fn default() -> Self {
        Self::Empty
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MainView {
    Workspace,
    History,
    Search,
    Branches,
    Tags,
    Stashes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RepoSourceTab {
    Local,
    Remote,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum SearchDimension {
    Message,
    Files,
    Author,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffDisplayMode {
    Blocks,
    Full,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum HistorySortOrder {
    Date,
    Topology,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum HistoryBranchScope {
    Current,
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SettingsTab {
    General,
    RepoRemotes,
    RepoAdvanced,
}

#[derive(Clone, Debug)]
enum CommitActionDialog {
    CreateBranch {
        hash: String,
        short_hash: String,
        name: String,
        checkout: bool,
    },
    CreateTag {
        hash: String,
        short_hash: String,
        name: String,
        push_after_create: bool,
        remote: String,
    },
    ConfirmCheckout {
        hash: String,
        short_hash: String,
    },
    ConfirmCherryPick {
        hash: String,
        short_hash: String,
    },
    ConfirmCherryPickBatch {
        hashes: Vec<String>,
        short_hashes: Vec<String>,
    },
    ConfirmRevert {
        hash: String,
        short_hash: String,
    },
    ConfirmReset {
        hash: String,
        short_hash: String,
        mode: ResetMode,
    },
}

#[derive(Clone, Debug)]
enum CommitMenuAction {
    Checkout { hash: String, short_hash: String },
    CreateBranch { hash: String, short_hash: String },
    CreateTag { hash: String, short_hash: String },
    CherryPick { hash: String, short_hash: String },
    Revert { hash: String, short_hash: String },
    Reset { hash: String, short_hash: String },
    CompareWithWorktree { hash: String, short_hash: String },
    ExternalDiff { hash: String, short_hash: String },
    OpenRemote { hash: String },
}

#[derive(Clone, Debug)]
enum WorktreeActionDialog {
    ConfirmDiscard { path: String, untracked: bool },
    ResolveConflicts { selected_path: Option<String> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RepoToolbarAction {
    Pull,
    Push,
    Fetch,
}

#[derive(Clone, Copy, Debug)]
struct PendingToolbarClick {
    action: RepoToolbarAction,
    due_at: f64,
}

#[derive(Clone, Debug)]
struct PullActionDialog {
    remote: String,
    remote_branch: String,
    local_branch: String,
    commit_merge: bool,
    include_tags: bool,
    force_merge_commit: bool,
    rebase: bool,
}

#[derive(Clone, Debug)]
struct FetchActionDialog {
    all_remotes: bool,
    prune_tracking: bool,
    fetch_tags: bool,
    force_tags: bool,
}

#[derive(Clone, Debug)]
struct PushActionDialog {
    remote: String,
    rows: Vec<PushBranchRow>,
    push_tags: bool,
    force: bool,
}

#[derive(Clone, Debug)]
struct PushBranchRow {
    selected: bool,
    local_branch: String,
    remote_branch: String,
    track: bool,
    upstream: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConflictResolutionDialogAction {
    Accept(git::ConflictSide),
    Merge,
}

const CONFLICT_ACTION_BUTTON_SIZE: Vec2 = Vec2 { x: 112.0, y: 32.0 };
const CONFLICT_MODAL_SIZE: Vec2 = Vec2 { x: 760.0, y: 360.0 };
const CONFLICT_MODAL_INNER_SIZE: Vec2 = Vec2 { x: 720.0, y: 320.0 };
const CONFLICT_LIST_PANEL_SIZE: Vec2 = Vec2 { x: 560.0, y: 260.0 };
const CONFLICT_ACTION_PANEL_SIZE: Vec2 = Vec2 { x: 132.0, y: 260.0 };
const CONFLICT_MODAL_PANEL_GAP: f32 = 14.0;

#[derive(Clone, Debug)]
enum StashActionDialog {
    Create { message: String },
    ConfirmDrop { selector: String, message: String },
}

#[derive(Clone, Debug)]
enum BranchActionDialog {
    Create {
        name: String,
        checkout: bool,
    },
    ConfirmCheckout {
        name: String,
        discard_changes: bool,
    },
    CheckoutRemote {
        remote_branch: String,
        local_branch: String,
    },
    Rename {
        old_name: String,
        new_name: String,
    },
    ConfirmDelete {
        name: String,
        force: bool,
    },
    ConfirmDeleteRemote {
        remote_branch: String,
    },
}

#[derive(Clone, Debug)]
enum TagActionDialog {
    Create {
        name: String,
        push_after_create: bool,
        remote: String,
    },
    Push {
        name: String,
        remote: String,
    },
    ConfirmDelete {
        name: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RepoRemoteActionDialog {
    Add {
        name: String,
        url: String,
        account_index: usize,
        validation_error: Option<String>,
    },
    Edit {
        original_name: String,
        name: String,
        url: String,
        account_index: usize,
        validation_error: Option<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct RemoteAccountSettings {
    name: String,
    host: String,
}

impl Default for RemoteAccountSettings {
    fn default() -> Self {
        Self {
            name: "Generic Account".to_owned(),
            host: "Generic Host".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedWorktreeFile {
    path: String,
    display_path: String,
    staged: bool,
    untracked: bool,
}

#[derive(Clone, Debug)]
struct WorktreeRowClick {
    file: SelectedWorktreeFile,
    modifiers: WorktreeSelectionModifiers,
}

#[derive(Clone, Copy, Debug)]
struct WorkspaceMainLayout {
    staged_rect: Rect,
    staged_unstaged_splitter_rect: Rect,
    unstaged_rect: Rect,
    list_commit_splitter_rect: Rect,
    commit_rect: Rect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorktreeDisplayMode {
    Flat,
    Tree,
}

impl WorktreeDisplayMode {
    fn toggle(self) -> Self {
        match self {
            Self::Flat => Self::Tree,
            Self::Tree => Self::Flat,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct WorktreeSelectionModifiers {
    ctrl: bool,
    shift: bool,
}

#[derive(Clone, Debug, Default)]
struct WorktreeSelectionState {
    staged: HashSet<String>,
    unstaged: HashSet<String>,
    staged_anchor: Option<String>,
    unstaged_anchor: Option<String>,
}

impl WorktreeSelectionState {
    fn paths(&self, staged: bool) -> &HashSet<String> {
        if staged { &self.staged } else { &self.unstaged }
    }

    fn paths_mut(&mut self, staged: bool) -> &mut HashSet<String> {
        if staged {
            &mut self.staged
        } else {
            &mut self.unstaged
        }
    }

    fn anchor(&self, staged: bool) -> Option<&str> {
        if staged {
            self.staged_anchor.as_deref()
        } else {
            self.unstaged_anchor.as_deref()
        }
    }

    fn set_anchor(&mut self, staged: bool, path: &str) {
        if staged {
            self.staged_anchor = Some(path.to_owned());
        } else {
            self.unstaged_anchor = Some(path.to_owned());
        }
    }

    fn clear_other_side(&mut self, staged: bool) {
        if staged {
            self.unstaged.clear();
            self.unstaged_anchor = None;
        } else {
            self.staged.clear();
            self.staged_anchor = None;
        }
    }

    fn contains(&self, staged: bool, path: &str) -> bool {
        self.paths(staged).contains(path)
    }

    fn apply(
        &mut self,
        files: &[WorktreeFile],
        path: &str,
        staged: bool,
        modifiers: WorktreeSelectionModifiers,
    ) {
        self.clear_other_side(staged);
        if modifiers.shift {
            let had_anchor = self.anchor(staged).is_some();
            let anchor = self.anchor(staged).unwrap_or(path).to_owned();
            let range = worktree_selection_range(files, &anchor, path);
            if !modifiers.ctrl {
                self.paths_mut(staged).clear();
            }
            self.paths_mut(staged).extend(range);
            if !had_anchor {
                self.set_anchor(staged, path);
            }
            return;
        }

        if modifiers.ctrl {
            let paths = self.paths_mut(staged);
            if !paths.insert(path.to_owned()) {
                paths.remove(path);
            }
        } else {
            let paths = self.paths_mut(staged);
            paths.clear();
            paths.insert(path.to_owned());
        }
        self.set_anchor(staged, path);
    }
}

fn worktree_selection_range(files: &[WorktreeFile], anchor: &str, path: &str) -> Vec<String> {
    let Some(anchor_index) = files.iter().position(|file| file.path == anchor) else {
        return vec![path.to_owned()];
    };
    let Some(path_index) = files.iter().position(|file| file.path == path) else {
        return vec![path.to_owned()];
    };
    let start = anchor_index.min(path_index);
    let end = anchor_index.max(path_index);
    files[start..=end]
        .iter()
        .map(|file| file.path.clone())
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HistoryRowsCacheKey {
    scope: HistoryBranchScope,
    sort_order: HistorySortOrder,
    search_dimension: SearchDimension,
    search: String,
    commit_count: usize,
    first_hash: String,
    last_hash: String,
    details_cache_len: usize,
    file_search_query: String,
    file_search_hash_count: usize,
}

#[derive(Clone, Debug, Default)]
struct HistoryRowsCache {
    key: Option<HistoryRowsCacheKey>,
    visible_hashes: Vec<String>,
    hash_to_index: HashMap<String, usize>,
}

impl HistoryRowsCache {
    fn clear(&mut self) {
        self.key = None;
        self.visible_hashes.clear();
        self.hash_to_index.clear();
    }

    fn len(&self) -> usize {
        self.visible_hashes.len()
    }

    fn first_index(&self) -> Option<usize> {
        self.visible_hashes
            .first()
            .and_then(|hash| self.hash_to_index.get(hash))
            .copied()
    }

    fn index_at(&self, row_index: usize) -> Option<usize> {
        self.visible_hashes
            .get(row_index)
            .and_then(|hash| self.hash_to_index.get(hash))
            .copied()
    }

    fn refresh(
        &mut self,
        key: HistoryRowsCacheKey,
        snapshot: &RepositorySnapshot,
        details_cache: &HashMap<String, CommitDetails>,
        file_search_query: &str,
        file_search_hashes: &HashSet<String>,
    ) {
        if self.key.as_ref() == Some(&key) {
            return;
        }

        self.key = Some(key.clone());
        self.visible_hashes.clear();
        self.hash_to_index.clear();

        let query = key.search.to_lowercase();
        for (index, commit) in snapshot.commits.iter().enumerate() {
            self.hash_to_index.insert(commit.hash.clone(), index);
            if query.is_empty()
                || history_commit_matches_search(
                    commit,
                    key.search_dimension,
                    &query,
                    details_cache,
                    file_search_query,
                    file_search_hashes,
                )
            {
                self.visible_hashes.push(commit.hash.clone());
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default)]
struct LayoutPrefs {
    sidebar_pct: f32,
    details_pct: f32,
    workspace_list_pct: f32,
    workspace_staged_pct: f32,
    workspace_diff_pct: f32,
    history_graph_pct: f32,
    history_top_pct: f32,
    history_desc_pct: f32,
    history_date_pct: f32,
    history_author_pct: f32,
    history_hash_pct: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct AppSettings {
    theme: SettingsThemeMode,
    theme_accent: SettingsThemeAccent,
    language: SettingsLanguage,
    remote_accounts: Vec<RemoteAccountSettings>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum SettingsThemeMode {
    Dark,
    Light,
}

impl Default for SettingsThemeMode {
    fn default() -> Self {
        Self::Dark
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum SettingsThemeAccent {
    Green,
    Blue,
    Purple,
    Rose,
    Orange,
}

impl Default for SettingsThemeAccent {
    fn default() -> Self {
        Self::Green
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum SettingsLanguage {
    English,
    Chinese,
}

impl Default for SettingsLanguage {
    fn default() -> Self {
        Self::Chinese
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: SettingsThemeMode::Dark,
            theme_accent: SettingsThemeAccent::Green,
            language: SettingsLanguage::Chinese,
            remote_accounts: vec![RemoteAccountSettings::default()],
        }
    }
}

impl AppSettings {
    fn load() -> Self {
        let Some(path) = app_settings_path() else {
            return Self::default();
        };
        fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn save(self) {
        let Some(path) = app_settings_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(raw) = serde_json::to_string_pretty(&self) {
            let _ = fs::write(path, raw);
        }
    }
}

fn normalized_remote_accounts(accounts: &[RemoteAccountSettings]) -> Vec<RemoteAccountSettings> {
    if accounts.is_empty() {
        vec![RemoteAccountSettings::default()]
    } else {
        accounts.to_vec()
    }
}

fn validate_remote_account_settings(name: &str, host: &str) -> Result<(), String> {
    let name = name.trim();
    let host = host.trim();
    if name.is_empty() {
        return Err("remote account name is empty".to_owned());
    }
    if host.is_empty() {
        return Err("remote account host is empty".to_owned());
    }
    if name.contains(['\n', '\r']) || host.contains(['\n', '\r']) {
        return Err("remote account fields must be single line".to_owned());
    }
    if !remote_account_host_is_valid(host) {
        return Err("remote account host is invalid".to_owned());
    }
    Ok(())
}

fn remote_account_host_is_valid(host: &str) -> bool {
    let host = host.trim();
    if host.eq_ignore_ascii_case("Generic Host") {
        return true;
    }
    if host.contains(char::is_whitespace) {
        return false;
    }
    if let Some(rest) = host
        .strip_prefix("https://")
        .or_else(|| host.strip_prefix("http://"))
    {
        return !rest.is_empty() && rest.contains('.');
    }
    if let Some(rest) = host.strip_prefix("git@") {
        return rest.contains(':')
            && rest
                .split(':')
                .next()
                .is_some_and(|part| part.contains('.'));
    }
    host.contains('.') || host.contains(':')
}

fn validate_repo_remote_action_dialog(name: &str, url: &str) -> Result<(), String> {
    let name = name.trim();
    let url = url.trim();
    if name.is_empty() {
        return Err("remote name is empty".to_owned());
    }
    if url.is_empty() {
        return Err("remote URL is empty".to_owned());
    }
    if name.contains(char::is_whitespace) || name.contains(['\n', '\r']) {
        return Err("remote name must not contain whitespace".to_owned());
    }
    if url.contains(['\n', '\r']) {
        return Err("remote URL must be single line".to_owned());
    }
    Ok(())
}

impl From<SettingsThemeMode> for theme::ThemeMode {
    fn from(value: SettingsThemeMode) -> Self {
        match value {
            SettingsThemeMode::Dark => Self::Dark,
            SettingsThemeMode::Light => Self::Light,
        }
    }
}

impl From<theme::ThemeMode> for SettingsThemeMode {
    fn from(value: theme::ThemeMode) -> Self {
        match value {
            theme::ThemeMode::Dark => Self::Dark,
            theme::ThemeMode::Light => Self::Light,
        }
    }
}

impl From<SettingsThemeAccent> for theme::ThemeAccent {
    fn from(value: SettingsThemeAccent) -> Self {
        match value {
            SettingsThemeAccent::Green => Self::Green,
            SettingsThemeAccent::Blue => Self::Blue,
            SettingsThemeAccent::Purple => Self::Purple,
            SettingsThemeAccent::Rose => Self::Rose,
            SettingsThemeAccent::Orange => Self::Orange,
        }
    }
}

impl From<theme::ThemeAccent> for SettingsThemeAccent {
    fn from(value: theme::ThemeAccent) -> Self {
        match value {
            theme::ThemeAccent::Green => Self::Green,
            theme::ThemeAccent::Blue => Self::Blue,
            theme::ThemeAccent::Purple => Self::Purple,
            theme::ThemeAccent::Rose => Self::Rose,
            theme::ThemeAccent::Orange => Self::Orange,
        }
    }
}

impl From<SettingsLanguage> for Language {
    fn from(value: SettingsLanguage) -> Self {
        match value {
            SettingsLanguage::English => Self::English,
            SettingsLanguage::Chinese => Self::Chinese,
        }
    }
}

impl From<Language> for SettingsLanguage {
    fn from(value: Language) -> Self {
        match value {
            Language::English => Self::English,
            Language::Chinese => Self::Chinese,
        }
    }
}

fn app_data_dir() -> Option<PathBuf> {
    env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .or_else(|| env::current_dir().ok())
        .map(|base| base.join("data"))
}

fn app_settings_path() -> Option<PathBuf> {
    app_data_dir().map(|base| base.join("settings.json"))
}

fn repo_tabs_path() -> Option<PathBuf> {
    app_data_dir().map(|base| base.join("tabs.json"))
}

fn repo_commit_state_path() -> Option<PathBuf> {
    app_data_dir().map(|base| base.join("commit-options.json"))
}

fn repo_state_key(path: &Path) -> String {
    path.display().to_string()
}

impl RepoCommitStateStore {
    fn load() -> Self {
        let Some(path) = repo_commit_state_path() else {
            return Self::default();
        };
        fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        let Some(path) = repo_commit_state_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, raw);
        }
    }

    fn state_for(path: &Path) -> RepoCommitState {
        Self::load()
            .repositories
            .get(&repo_state_key(path))
            .cloned()
            .unwrap_or_default()
    }

    fn save_for(path: &Path, state: &RepoCommitState) {
        let mut store = Self::load();
        store
            .repositories
            .insert(repo_state_key(path), state.clone());
        store.save();
    }
}

impl RepoTabsState {
    fn load() -> Self {
        let Some(path) = repo_tabs_path() else {
            return Self::default();
        };
        fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn from_app(app: &GitAgentApp) -> Self {
        Self {
            tabs: app
                .repo_tabs
                .iter()
                .map(|tab| tab.root.display().to_string())
                .collect(),
            active_repo_tab: app.active_repo_tab,
            source_tab_open: app.source_tab_open,
            source_tab_active: app.repository_source_active(),
            sidebar_tree_states: app.sidebar_tree_states.clone(),
        }
    }

    fn save(&self) {
        let Some(path) = repo_tabs_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, raw);
        }
    }

    fn repo_tabs(&self) -> Vec<RepoTab> {
        let mut tabs = Vec::new();
        for raw in &self.tabs {
            let root = PathBuf::from(raw);
            if tabs
                .iter()
                .any(|tab: &RepoTab| paths_equal(&tab.root, &root))
            {
                continue;
            }
            let name = root
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .unwrap_or("Repository")
                .to_owned();
            tabs.push(RepoTab { root, name });
        }
        tabs
    }
}

fn reorder_repo_tabs(
    tabs: &mut Vec<RepoTab>,
    active_repo_tab: Option<usize>,
    from: usize,
    to: usize,
) -> Option<usize> {
    if from >= tabs.len() || to >= tabs.len() || from == to {
        return active_repo_tab;
    }

    let tab = tabs.remove(from);
    tabs.insert(to, tab);

    active_repo_tab.map(|active| {
        if active == from {
            to
        } else if from < active && active <= to {
            active - 1
        } else if to <= active && active < from {
            active + 1
        } else {
            active
        }
    })
}

fn active_repo_root_for(
    repo_tabs: &[RepoTab],
    active_repo_tab: Option<usize>,
    snapshot_root: Option<&PathBuf>,
) -> Option<PathBuf> {
    active_repo_tab
        .and_then(|index| repo_tabs.get(index))
        .map(|tab| tab.root.clone())
        .or_else(|| snapshot_root.cloned())
}

fn conflict_temp_name(path: &str) -> String {
    path.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

impl Default for LayoutPrefs {
    fn default() -> Self {
        Self {
            sidebar_pct: 0.19,
            details_pct: 0.32,
            workspace_list_pct: 0.58,
            workspace_staged_pct: 0.5,
            workspace_diff_pct: 0.36,
            history_graph_pct: 0.24,
            history_top_pct: 0.0,
            history_desc_pct: 0.52,
            history_date_pct: 0.18,
            history_author_pct: 0.18,
            history_hash_pct: 0.08,
        }
    }
}

impl LayoutPrefs {
    fn load() -> Self {
        let Some(path) = layout_prefs_path() else {
            return Self::default();
        };
        let Ok(raw) = fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str::<Self>(&raw)
            .ok()
            .or_else(|| Self::parse(&raw))
            .unwrap_or_default()
    }

    fn parse(raw: &str) -> Option<Self> {
        let mut prefs = Self::default();
        for line in raw.lines() {
            let (key, value) = line.split_once('=')?;
            let value = value.trim().parse::<f32>().ok()?;
            match key.trim() {
                "sidebar_pct" => prefs.sidebar_pct = value,
                "details_pct" => prefs.details_pct = value,
                "workspace_list_pct" => prefs.workspace_list_pct = value,
                "workspace_staged_pct" => prefs.workspace_staged_pct = value,
                "workspace_diff_pct" => prefs.workspace_diff_pct = value,
                "history_graph_pct" => prefs.history_graph_pct = value,
                "history_top_pct" => prefs.history_top_pct = value,
                "history_desc_pct" => prefs.history_desc_pct = value,
                "history_date_pct" => prefs.history_date_pct = value,
                "history_author_pct" => prefs.history_author_pct = value,
                "history_hash_pct" => prefs.history_hash_pct = value,
                _ => {}
            }
        }
        prefs.clamp();
        Some(prefs)
    }

    fn clamp(&mut self) {
        self.sidebar_pct = self.sidebar_pct.clamp(0.14, 0.34);
        self.details_pct = self.details_pct.clamp(0.22, 0.46);
        self.workspace_list_pct = self.workspace_list_pct.clamp(0.42, 0.74);
        self.workspace_staged_pct = self.workspace_staged_pct.clamp(0.24, 0.76);
        self.workspace_diff_pct = self.workspace_diff_pct.clamp(0.28, 0.52);
        self.history_graph_pct = self.history_graph_pct.clamp(0.03, 0.45);
        self.history_top_pct = sanitize_optional_pct(self.history_top_pct, 0.22, 0.72);
        self.history_desc_pct = sanitize_history_pct(self.history_desc_pct, 0.52);
        self.history_date_pct = sanitize_history_pct(self.history_date_pct, 0.18);
        self.history_author_pct = sanitize_history_pct(self.history_author_pct, 0.18);
        self.history_hash_pct = sanitize_history_pct(self.history_hash_pct, 0.08);
    }

    fn save(self) {
        let Some(path) = layout_prefs_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(raw) = serde_json::to_string_pretty(&self) {
            let _ = fs::write(path, raw);
        }
    }
}

fn sanitize_history_pct(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value.clamp(0.001, 0.999)
    } else {
        fallback
    }
}

fn sanitize_optional_pct(value: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value.clamp(min, max)
    } else {
        0.0
    }
}

fn layout_prefs_path() -> Option<PathBuf> {
    app_data_dir().map(|base| base.join("layout.json"))
}

impl GitAgentApp {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let app_settings = AppSettings::load();
        let tabs_state = RepoTabsState::load();
        let has_saved_tabs = tabs_state.source_tab_open || !tabs_state.tabs.is_empty();
        let repo_tabs = tabs_state.repo_tabs();
        let mut active_repo_tab = tabs_state
            .active_repo_tab
            .filter(|index| *index < repo_tabs.len());
        if tabs_state.source_tab_open && tabs_state.source_tab_active {
            active_repo_tab = None;
        } else if active_repo_tab.is_none() && !repo_tabs.is_empty() {
            active_repo_tab = Some(0);
        }

        let mut app = Self {
            repo_tabs,
            active_repo_tab,
            repo_tab_drag: RepoTabDragState::default(),
            source_tab_open: tabs_state.source_tab_open,
            repo_source_tab: RepoSourceTab::Local,
            snapshot: None,
            snapshot_cache: HashMap::new(),
            layout: GraphLayout::default(),
            selected_commit: None,
            error: None,
            search: String::new(),
            search_view_query: String::new(),
            search_selected_commit: None,
            search_selected_file_path: None,
            search_selected_diff_rows: Vec::new(),
            search_diff_display_mode: DiffDisplayMode::Blocks,
            repo_source_search: String::new(),
            known_repositories: Vec::new(),
            clone_url: String::new(),
            clone_destination: String::new(),
            create_repo_path: String::new(),
            clone_url_status: CloneUrlStatus::Empty,
            clone_url_last_edited: None,
            clone_url_task: None,
            search_dimension: SearchDimension::Message,
            repo_task: None,
            remote_git_task: None,
            branch_checkout_task: None,
            merge_tool_task: None,
            repo_source_task: None,
            details_task: None,
            diff_task: None,
            file_search_task: None,
            file_search_started_at: None,
            file_search_query: String::new(),
            file_search_hashes: HashSet::new(),
            details_cache: HashMap::new(),
            diff_cache: HashMap::new(),
            selected_file_path: None,
            history_diff_display_mode: DiffDisplayMode::Blocks,
            history_sort_order: HistorySortOrder::Date,
            history_branch_scope: HistoryBranchScope::Current,
            history_cherry_pick_mode: false,
            selected_cherry_pick_hashes: HashSet::new(),
            selected_diff_rows: Vec::new(),
            history_rows_cache: HistoryRowsCache::default(),
            selected_worktree_file: None,
            worktree_selection: WorktreeSelectionState::default(),
            worktree_display_mode: WorktreeDisplayMode::Flat,
            worktree_collapsed_dirs: HashSet::new(),
            loading_repo: false,
            loading_details_hash: None,
            loading_diff_key: None,
            pending_branch_checkout: None,
            pending_commit_action: None,
            last_notice: None,
            toast_notice: None,
            pending_toolbar_single_click: None,
            pending_worktree_action: None,
            pending_fetch_action: None,
            pending_pull_action: None,
            pending_push_action: None,
            commit_message: String::new(),
            commit_state: RepoCommitState::default(),
            focus_commit_message: false,
            language: app_settings.language.into(),
            pending_stash_action: None,
            pending_branch_action: None,
            pending_tag_action: None,
            active_view: MainView::Workspace,
            branches_open: SidebarTreeState::default().branches_open,
            tags_open: SidebarTreeState::default().tags_open,
            remotes_open: SidebarTreeState::default().remotes_open,
            local_branch_collapsed_groups: HashSet::new(),
            remote_branch_collapsed_groups: HashSet::new(),
            stashes_open: SidebarTreeState::default().stashes_open,
            sidebar_tree_states: tabs_state.sidebar_tree_states.clone(),
            settings_open: env::var("GIT_AGENT_OPEN_SETTINGS_ON_START").ok().as_deref()
                == Some("1"),
            settings_tab: SettingsTab::General,
            repo_settings_open: env::var("GIT_AGENT_OPEN_REPO_SETTINGS_ON_START")
                .ok()
                .as_deref()
                == Some("1"),
            repo_settings_tab: SettingsTab::RepoRemotes,
            pending_repo_remote_action: None,
            remote_accounts: normalized_remote_accounts(&app_settings.remote_accounts),
            remote_account_name_input: String::new(),
            remote_account_host_input: String::new(),
            remote_account_error: None,
            theme_mode: if env::var("GIT_AGENT_THEME").ok().as_deref() == Some("light") {
                theme::ThemeMode::Light
            } else if env::var("GIT_AGENT_THEME").ok().as_deref() == Some("dark") {
                theme::ThemeMode::Dark
            } else {
                app_settings.theme.into()
            },
            theme_accent: app_settings.theme_accent.into(),
            layout_prefs: LayoutPrefs::load(),
            history_show_remote_refs: true,
        };

        app.refresh_known_repositories();
        if let Some(index) = app.active_repo_tab {
            if let Some(path) = app.repo_tabs.get(index).map(|tab| tab.root.clone()) {
                app.load_repository(path);
            }
        } else if !has_saved_tabs {
            if let Ok(cwd) = env::current_dir() {
                app.load_repository(cwd);
            }
        } else {
            if app.repo_tabs.is_empty() && !app.source_tab_open {
                app.source_tab_open = true;
            }
            app.save_repo_tabs();
        }
        app.save_app_settings();

        app
    }

    fn load_repository(&mut self, path: PathBuf) {
        self.load_repository_with_cache_mode(path, true);
    }

    fn load_repository_uncached(&mut self, path: PathBuf) {
        self.load_repository_with_cache_mode(path, false);
    }

    fn load_repository_with_cache_mode(&mut self, path: PathBuf, use_cache: bool) {
        let can_keep_current_snapshot = self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| paths_equal(&snapshot.root, &path));
        self.ensure_repo_tab(path.clone());
        self.load_commit_state_for_active_repo();
        if use_cache {
            if !self.apply_cached_snapshot_for(&path) {
                self.clear_repository_snapshot_view();
            }
        } else if !can_keep_current_snapshot {
            self.clear_repository_snapshot_view();
        }
        let (sender, receiver) = mpsc::channel();
        self.repo_task = Some(receiver);
        self.loading_repo = true;
        self.error = None;

        thread::spawn(move || {
            let requested_root = path.clone();
            let _ = sender.send((requested_root, git::open_repository(path)));
        });
    }

    fn open_repository_source_tab(&mut self) {
        self.source_tab_open = true;
        self.active_repo_tab = None;
        self.active_view = MainView::Workspace;
        self.commit_state = RepoCommitState::default();
        self.refresh_known_repositories();
        self.save_repo_tabs();
    }

    fn repository_source_active(&self) -> bool {
        self.source_tab_open && self.active_repo_tab.is_none()
    }

    fn ensure_repo_tab(&mut self, path: PathBuf) {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("Repository")
            .to_owned();

        if let Some(index) = self
            .repo_tabs
            .iter()
            .position(|tab| paths_equal(&tab.root, &path))
        {
            self.active_repo_tab = Some(index);
            self.save_repo_tabs();
            return;
        }

        self.repo_tabs.push(RepoTab { root: path, name });
        self.active_repo_tab = Some(self.repo_tabs.len() - 1);
        self.refresh_known_repositories();
        self.save_repo_tabs();
    }

    fn switch_repo_tab(&mut self, index: usize) {
        if self.active_repo_tab == Some(index) {
            return;
        }
        if let Some(tab) = self.repo_tabs.get(index).cloned() {
            self.active_repo_tab = Some(index);
            self.save_repo_tabs();
            self.load_repository(tab.root);
        }
    }

    fn close_repo_tab(&mut self, index: usize) {
        if index >= self.repo_tabs.len() {
            return;
        }

        let was_active = self.active_repo_tab == Some(index);
        self.repo_tabs.remove(index);
        self.active_repo_tab = match self.active_repo_tab {
            Some(active) if active == index && !self.repo_tabs.is_empty() => {
                Some(index.min(self.repo_tabs.len() - 1))
            }
            Some(active) if active > index => Some(active - 1),
            Some(active) if active < self.repo_tabs.len() => Some(active),
            _ => None,
        };

        if self.repo_tabs.is_empty() {
            self.snapshot = None;
            self.source_tab_open = true;
            self.active_repo_tab = None;
        }
        self.refresh_known_repositories();
        self.save_repo_tabs();

        if was_active {
            if let Some(active) = self.active_repo_tab {
                if let Some(path) = self.repo_tabs.get(active).map(|tab| tab.root.clone()) {
                    self.load_repository(path);
                }
            }
        }
    }

    fn sync_active_tab_with_snapshot(&mut self, snapshot: &RepositorySnapshot) {
        let name = snapshot
            .root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("Repository")
            .to_owned();

        if let Some(index) = self
            .repo_tabs
            .iter()
            .position(|tab| paths_equal(&tab.root, &snapshot.root))
        {
            self.active_repo_tab = Some(index);
            if let Some(tab) = self.repo_tabs.get_mut(index) {
                tab.root = snapshot.root.clone();
                tab.name = name;
            }
        } else {
            self.repo_tabs.push(RepoTab {
                root: snapshot.root.clone(),
                name,
            });
            self.active_repo_tab = Some(self.repo_tabs.len() - 1);
        }
        self.refresh_known_repositories();
        self.save_repo_tabs();
    }

    fn active_repo_root_matches(&self, root: &Path) -> bool {
        self.active_repo_tab
            .and_then(|index| self.repo_tabs.get(index))
            .is_some_and(|tab| paths_equal(&tab.root, root))
    }

    fn cache_repository_snapshot(&mut self, snapshot: &RepositorySnapshot) {
        self.snapshot_cache
            .insert(repo_state_key(&snapshot.root), snapshot.clone());
    }

    fn cached_snapshot_for(&self, path: &Path) -> Option<RepositorySnapshot> {
        self.snapshot_cache
            .get(&repo_state_key(path))
            .cloned()
            .or_else(|| {
                self.snapshot_cache
                    .values()
                    .find(|snapshot| paths_equal(&snapshot.root, path))
                    .cloned()
            })
    }

    fn apply_cached_snapshot_for(&mut self, path: &Path) -> bool {
        let Some(snapshot) = self.cached_snapshot_for(path) else {
            return false;
        };
        self.apply_repository_snapshot(snapshot);
        true
    }

    fn clear_repository_snapshot_view(&mut self) {
        self.snapshot = None;
        self.layout = GraphLayout::default();
        self.selected_commit = None;
        self.search_selected_commit = None;
        self.details_cache.clear();
        self.diff_cache.clear();
        self.file_search_task = None;
        self.file_search_started_at = None;
        self.file_search_query.clear();
        self.file_search_hashes.clear();
        self.selected_file_path = None;
        self.search_selected_file_path = None;
        self.selected_diff_rows.clear();
        self.search_selected_diff_rows.clear();
        self.history_rows_cache.clear();
        self.clear_cherry_pick_selection();
        self.selected_worktree_file = None;
        self.worktree_selection = WorktreeSelectionState::default();
    }

    fn apply_repository_snapshot(&mut self, mut snapshot: RepositorySnapshot) {
        self.apply_history_sort_order_to_snapshot(&mut snapshot);
        self.layout = graph::layout(&snapshot.commits);
        self.selected_commit = (!snapshot.commits.is_empty()).then_some(0);
        self.search_selected_commit = None;
        self.sync_active_tab_with_snapshot(&snapshot);
        self.apply_sidebar_tree_state_for_repo(&snapshot.root);
        self.apply_merge_commit_message_default(&snapshot);
        self.snapshot = Some(snapshot);
        self.details_cache.clear();
        self.diff_cache.clear();
        self.file_search_task = None;
        self.file_search_started_at = None;
        self.file_search_query.clear();
        self.file_search_hashes.clear();
        self.selected_file_path = None;
        self.search_selected_file_path = None;
        self.selected_diff_rows.clear();
        self.search_selected_diff_rows.clear();
        self.history_rows_cache.clear();
        self.clear_cherry_pick_selection();
        self.selected_worktree_file = None;
        self.worktree_selection = WorktreeSelectionState::default();
        self.request_selected_details();
    }

    fn apply_merge_commit_message_default(&mut self, snapshot: &RepositorySnapshot) {
        if let Some(message) = snapshot.merge_message.as_deref() {
            if self.commit_message.trim().is_empty() {
                self.commit_message = message.to_owned();
            }
        }
    }

    fn refresh(&mut self) {
        if let Some(root) = self.snapshot.as_ref().map(|snapshot| snapshot.root.clone()) {
            self.load_repository(root);
        }
    }

    fn apply_history_sort_order_to_snapshot(&self, snapshot: &mut RepositorySnapshot) {
        snapshot.commits = match (self.history_branch_scope, self.history_sort_order) {
            (HistoryBranchScope::Current, HistorySortOrder::Date) => snapshot.date_commits.clone(),
            (HistoryBranchScope::Current, HistorySortOrder::Topology) => {
                snapshot.topology_commits.clone()
            }
            (HistoryBranchScope::All, HistorySortOrder::Date) => snapshot.all_date_commits.clone(),
            (HistoryBranchScope::All, HistorySortOrder::Topology) => {
                snapshot.all_topology_commits.clone()
            }
        };
    }

    fn set_history_sort_order(&mut self, order: HistorySortOrder) {
        self.set_history_ordering(self.history_branch_scope, order);
    }

    fn set_history_branch_scope(&mut self, scope: HistoryBranchScope) {
        self.set_history_ordering(scope, self.history_sort_order);
    }

    fn set_history_ordering(&mut self, scope: HistoryBranchScope, order: HistorySortOrder) {
        if self.history_branch_scope == scope && self.history_sort_order == order {
            return;
        }
        let selected_hash = self.selected_commit_hash().map(str::to_owned);
        self.history_branch_scope = scope;
        self.history_sort_order = order;
        if let Some(mut snapshot) = self.snapshot.take() {
            self.apply_history_sort_order_to_snapshot(&mut snapshot);
            self.layout = graph::layout(&snapshot.commits);
            self.selected_commit = selected_hash
                .as_deref()
                .and_then(|hash| {
                    snapshot
                        .commits
                        .iter()
                        .position(|commit| commit.hash == hash)
                })
                .or_else(|| (!snapshot.commits.is_empty()).then_some(0));
            self.snapshot = Some(snapshot);
        }
    }

    fn active_repo_root(&self) -> Option<PathBuf> {
        active_repo_root_for(
            &self.repo_tabs,
            self.active_repo_tab,
            self.snapshot.as_ref().map(|snapshot| &snapshot.root),
        )
    }

    fn current_sidebar_tree_state(&self) -> SidebarTreeState {
        SidebarTreeState {
            branches_open: self.branches_open,
            tags_open: self.tags_open,
            remotes_open: self.remotes_open,
            stashes_open: self.stashes_open,
        }
    }

    fn apply_sidebar_tree_state_for_repo(&mut self, root: &Path) {
        let state = self
            .sidebar_tree_states
            .get(&repo_state_key(root))
            .copied()
            .unwrap_or_default();
        self.branches_open = state.branches_open;
        self.tags_open = state.tags_open;
        self.remotes_open = state.remotes_open;
        self.stashes_open = state.stashes_open;
    }

    fn save_sidebar_tree_state_for_active_repo(&mut self) {
        let Some(root) = self.active_repo_root() else {
            return;
        };
        self.sidebar_tree_states
            .insert(repo_state_key(&root), self.current_sidebar_tree_state());
        self.save_repo_tabs();
    }

    fn open_git_workflow(&mut self) {
        self.active_view = MainView::Branches;
        self.last_notice = Some(self.tr("repo.git_flow.opened").to_owned());
    }

    fn open_remote_url(&mut self) {
        let Some(url) = self.default_remote_web_url() else {
            self.error = Some(self.tr("repo.remote.missing").to_owned());
            return;
        };
        if let Err(error) = open_url(&url) {
            self.error = Some(format!("{}: {error}", self.tr("repo.remote.failed")));
        }
    }

    fn open_branch_compare_url(&mut self, branch: &str) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let Some(base_url) = self.default_remote_web_url() else {
            self.error = Some(self.tr("repo.remote.missing").to_owned());
            return;
        };
        let url = branch_compare_url(&base_url, &snapshot.branch, branch);
        if let Err(error) = open_url(&url) {
            self.error = Some(format!("{}: {error}", self.tr("repo.remote.failed")));
        }
    }

    fn open_branch_pull_request_url(&mut self, branch: &str) {
        let Some(base_url) = self.default_remote_web_url() else {
            self.error = Some(self.tr("repo.remote.missing").to_owned());
            return;
        };
        let url = branch_pull_request_url(&base_url, branch);
        if let Err(error) = open_url(&url) {
            self.error = Some(format!("{}: {error}", self.tr("repo.remote.failed")));
        }
    }

    fn open_commit_remote_url(&mut self, hash: &str) {
        let Some(base_url) = self.default_remote_web_url() else {
            self.error = Some(self.tr("repo.remote.missing").to_owned());
            return;
        };
        let url = commit_remote_url(&base_url, hash);
        if let Err(error) = open_url(&url) {
            self.error = Some(format!("{}: {error}", self.tr("repo.remote.failed")));
        }
    }

    fn open_commit_diff_tool(&mut self, hash: String, short_hash: String) {
        let title = format!("{short_hash} vs working tree");
        let theme = merge_theme_arg(self.theme_mode).to_owned();
        let language = merge_language_arg(self.language).to_owned();
        self.start_remote_git_action(move |root| {
            let diff_text = git::diff_worktree_against_commit(root, &hash)?;
            let temp_dir = env::temp_dir()
                .join("git-agent-diffs")
                .join(format!("{}-{short_hash}", std::process::id()));
            fs::create_dir_all(&temp_dir)?;
            let diff_path = temp_dir.join("changes.patch");
            fs::write(&diff_path, diff_text)?;
            let diff_exe = env::current_exe()?.with_file_name(if cfg!(windows) {
                "git-agent-diff.exe"
            } else {
                "git-agent-diff"
            });
            Command::new(&diff_exe)
                .current_dir(root)
                .arg("--title")
                .arg(&title)
                .arg("--left")
                .arg(&short_hash)
                .arg("--right")
                .arg("worktree")
                .arg("--diff")
                .arg(&diff_path)
                .arg("--theme")
                .arg(&theme)
                .arg("--language")
                .arg(&language)
                .spawn()
                .map(|_| ())
                .map_err(Into::into)
        });
    }

    fn default_remote_web_url(&self) -> Option<String> {
        let snapshot = self.snapshot.as_ref()?;
        let remote = snapshot.remotes.first()?;
        let remote_url = if remote.fetch_url.is_empty() {
            remote.push_url.as_str()
        } else {
            remote.fetch_url.as_str()
        };
        remote_web_url(remote_url)
    }

    fn open_command_mode(&mut self) {
        let Some(root) = self.active_repo_root() else {
            return;
        };
        if let Err(error) = open_command_prompt(&root) {
            self.error = Some(format!("{}: {error}", self.tr("repo.command_mode.failed")));
        }
    }

    fn open_resource_manager(&mut self) {
        let Some(root) = self.active_repo_root() else {
            return;
        };
        if let Err(error) = open_file_manager(&root) {
            self.error = Some(format!(
                "{}: {error}",
                self.tr("repo.resource_manager.failed")
            ));
        }
    }

    fn open_repo_config_file(&mut self) {
        let Some(path) = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.config.config_path.clone())
        else {
            return;
        };
        if let Err(error) = open_path(&path) {
            self.error = Some(format!(
                "{}: {error}",
                self.tr("repo.settings.config_failed")
            ));
        }
    }

    fn begin_add_remote_settings(&mut self) {
        self.pending_repo_remote_action = Some(RepoRemoteActionDialog::Add {
            name: String::new(),
            url: String::new(),
            account_index: 0,
            validation_error: None,
        });
    }

    fn begin_edit_remote_settings(&mut self, remote: &git::Remote) {
        self.pending_repo_remote_action = Some(RepoRemoteActionDialog::Edit {
            original_name: remote.name.clone(),
            name: remote.name.clone(),
            url: remote_display_url(remote).to_owned(),
            account_index: 0,
            validation_error: None,
        });
    }

    fn start_repo_source_task(
        &mut self,
        action: impl FnOnce() -> anyhow::Result<PathBuf> + Send + 'static,
    ) {
        let (sender, receiver) = mpsc::channel();
        self.repo_source_task = Some(receiver);
        self.error = None;
        self.last_notice = None;

        thread::spawn(move || {
            let _ = sender.send(action());
        });
    }

    fn clone_from_source_tab(&mut self) {
        let url = self.clone_url.trim().to_owned();
        let destination = self.clone_destination.trim().to_owned();
        if !self.clone_url_is_valid() || destination.is_empty() {
            self.error = Some(self.tr("repo.source.clone_missing").to_owned());
            return;
        }

        self.start_repo_source_task(move || {
            git::clone_repository(&url, PathBuf::from(destination))
        });
    }

    fn clone_url_is_valid(&self) -> bool {
        matches!(self.clone_url_status, CloneUrlStatus::Valid)
    }

    fn on_clone_url_changed(&mut self) {
        self.clone_destination.clear();
        self.clone_url_task = None;
        self.clone_url_last_edited = Some(Instant::now());
        self.clone_url_status = if self.clone_url.trim().is_empty() {
            CloneUrlStatus::Empty
        } else {
            CloneUrlStatus::Pending
        };
    }

    fn maybe_start_clone_url_validation(&mut self, ctx: &egui::Context) {
        if !matches!(self.clone_url_status, CloneUrlStatus::Pending) {
            return;
        }
        let Some(last_edited) = self.clone_url_last_edited else {
            return;
        };
        let debounce = Duration::from_millis(500);
        if last_edited.elapsed() < debounce {
            ctx.request_repaint_after(debounce - last_edited.elapsed());
            return;
        }

        let url = self.clone_url.trim().to_owned();
        if url.is_empty() {
            self.clone_url_status = CloneUrlStatus::Empty;
            return;
        }

        let (sender, receiver) = mpsc::channel();
        self.clone_url_task = Some(receiver);
        self.clone_url_status = CloneUrlStatus::Checking;
        thread::spawn(move || {
            let result = git::validate_remote_url(&url);
            let _ = sender.send((url, result));
        });
    }

    fn create_from_source_tab(&mut self) {
        let path = self.create_repo_path.trim().to_owned();
        if path.is_empty() {
            self.error = Some(self.tr("repo.source.create_missing").to_owned());
            return;
        }

        self.start_repo_source_task(move || git::init_repository(PathBuf::from(path)));
    }

    fn execute_git_action(
        &mut self,
        action: impl FnOnce(&std::path::Path) -> anyhow::Result<()> + Send + 'static,
    ) {
        self.start_remote_git_action(action);
    }

    fn branch_checkout_busy(&self) -> bool {
        self.branch_checkout_task.is_some() || self.pending_branch_checkout.is_some()
    }

    fn merge_tool_busy(&self) -> bool {
        self.merge_tool_task.is_some()
    }

    fn branch_actions_busy(&self) -> bool {
        self.loading_repo
            || self.branch_checkout_busy()
            || self.remote_git_busy()
            || self.merge_tool_busy()
    }

    fn repo_toolbar_loading_busy(&self) -> bool {
        self.loading_repo
            || self.branch_checkout_busy()
            || self.remote_git_busy()
            || self.merge_tool_busy()
            || self.repo_source_task.is_some()
    }

    fn request_branch_checkout(&mut self, name: String) {
        if self.branch_actions_busy() {
            return;
        }
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        if snapshot.branch == name {
            return;
        }
        if !snapshot.status.is_empty() {
            self.pending_branch_action = Some(BranchActionDialog::ConfirmCheckout {
                name,
                discard_changes: false,
            });
            return;
        }

        self.start_branch_checkout(name, false);
    }

    fn start_branch_checkout(&mut self, name: String, discard_changes: bool) {
        if self.branch_actions_busy() {
            return;
        }
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        if snapshot.branch == name {
            return;
        }
        let root = snapshot.root.clone();

        let (sender, receiver) = mpsc::channel();
        self.branch_checkout_task = Some(receiver);
        self.pending_branch_checkout = Some(name.clone());
        self.loading_repo = true;
        self.error = None;
        self.last_notice = None;

        thread::spawn(move || {
            let result = (|| {
                if discard_changes {
                    git::discard_all_changes(&root)?;
                }
                git::checkout_branch(&root, &name)
            })();
            let _ = sender.send((root, name, result));
        });
    }

    fn remote_git_busy(&self) -> bool {
        self.remote_git_task.is_some()
    }

    fn start_remote_git_action(
        &mut self,
        action: impl FnOnce(&std::path::Path) -> anyhow::Result<()> + Send + 'static,
    ) {
        if self.remote_git_busy() || self.loading_repo {
            return;
        }
        self.pending_toolbar_single_click = None;
        let Some(root) = self.snapshot.as_ref().map(|snapshot| snapshot.root.clone()) else {
            return;
        };

        let (sender, receiver) = mpsc::channel();
        self.remote_git_task = Some(receiver);
        self.loading_repo = true;
        self.error = None;
        self.last_notice = None;

        thread::spawn(move || {
            let result = action(&root);
            let _ = sender.send((root, result));
        });
    }

    fn handle_repo_toolbar_action_response(
        &mut self,
        ctx: &egui::Context,
        response: egui::Response,
        action: RepoToolbarAction,
    ) {
        if response.double_clicked() {
            self.pending_toolbar_single_click = None;
            self.run_quick_toolbar_action(action);
        } else if response.clicked() {
            let now = ctx.input(|input| input.time);
            self.pending_toolbar_single_click = Some(PendingToolbarClick {
                action,
                due_at: now + TOOLBAR_DOUBLE_CLICK_DELAY,
            });
            ctx.request_repaint_after(Duration::from_secs_f64(TOOLBAR_DOUBLE_CLICK_DELAY));
        }
    }

    fn flush_pending_toolbar_single_click(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_toolbar_single_click else {
            return;
        };
        let now = ctx.input(|input| input.time);
        if now >= pending.due_at {
            self.pending_toolbar_single_click = None;
            self.open_toolbar_action_dialog(pending.action);
        } else {
            ctx.request_repaint_after(Duration::from_secs_f64(pending.due_at - now));
        }
    }

    fn open_toolbar_action_dialog(&mut self, action: RepoToolbarAction) {
        match action {
            RepoToolbarAction::Pull => self.pull_current(),
            RepoToolbarAction::Push => self.push_current(),
            RepoToolbarAction::Fetch => self.fetch_all(),
        }
    }

    fn run_quick_toolbar_action(&mut self, action: RepoToolbarAction) {
        match action {
            RepoToolbarAction::Pull => self.quick_pull_current(),
            RepoToolbarAction::Push => self.quick_push_current(),
            RepoToolbarAction::Fetch => self.quick_fetch_all(),
        }
    }

    fn quick_fetch_all(&mut self) {
        self.start_remote_git_action(|root| {
            git::fetch_with_options(root, git::FetchOptions::default())
        });
    }

    fn fetch_all(&mut self) {
        self.open_fetch_dialog();
    }

    fn open_fetch_dialog(&mut self) {
        self.pending_fetch_action = Some(FetchActionDialog {
            all_remotes: true,
            prune_tracking: true,
            fetch_tags: false,
            force_tags: false,
        });
    }

    fn fetch_action_modal(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.pending_fetch_action.take() else {
            return;
        };

        let mut keep_open = true;
        let mut close_after = false;
        let mut execute: Option<Box<dyn FnOnce(&std::path::Path) -> anyhow::Result<()> + Send>> =
            None;
        let actions_enabled = !self.branch_actions_busy();

        compact_action_dialog(ctx, self.tr("fetch.title"), FETCH_DIALOG_WIDTH, |ui| {
            ui.checkbox(&mut dialog.all_remotes, self.tr("fetch.all_remotes"));
            ui.checkbox(&mut dialog.prune_tracking, self.tr("fetch.prune_tracking"));
            ui.horizontal(|ui| {
                ui.checkbox(&mut dialog.fetch_tags, self.tr("fetch.tags"));
                if !dialog.fetch_tags {
                    dialog.force_tags = false;
                }
                ui.add_enabled_ui(dialog.fetch_tags, |ui| {
                    ui.checkbox(&mut dialog.force_tags, self.tr("fetch.force_tags"));
                });
            });

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(self.tr("dialog.cancel")).clicked() {
                        close_after = true;
                    }
                    let submit_requested = dialog_default_submit_requested(ui);
                    if ui
                        .add_enabled(actions_enabled, egui::Button::new(self.tr("dialog.ok")))
                        .clicked()
                        || (submit_requested && actions_enabled)
                    {
                        let options = git::FetchOptions {
                            all_remotes: dialog.all_remotes,
                            prune_tracking: dialog.prune_tracking,
                            fetch_tags: dialog.fetch_tags,
                            force_tags: dialog.force_tags,
                        };
                        execute =
                            Some(Box::new(move |root| git::fetch_with_options(root, options)));
                        close_after = true;
                    }
                });
            });
        });

        if let Some(action) = execute {
            self.execute_git_action(action);
        }
        if close_after {
            keep_open = false;
        }
        if keep_open {
            self.pending_fetch_action = Some(dialog);
        }
    }

    fn pull_current(&mut self) {
        self.open_pull_dialog(None);
    }

    fn quick_pull_current(&mut self) {
        self.start_remote_git_action(|root| git::pull(root));
    }

    fn open_pull_dialog(&mut self, local_branch: Option<String>) {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        let local_branch = local_branch.unwrap_or_else(|| snapshot.branch.clone());
        let target_upstream = snapshot
            .branches
            .iter()
            .find(|branch| !branch.remote && branch.name == local_branch)
            .and_then(|branch| branch.upstream.as_ref())
            .or_else(|| {
                (local_branch == snapshot.branch)
                    .then_some(snapshot.upstream.as_ref())
                    .flatten()
            });
        let (upstream_remote, upstream_branch) = target_upstream
            .as_ref()
            .and_then(|upstream| split_remote_branch_name(&upstream.name))
            .unwrap_or_else(|| (self.default_remote_name(), String::new()));
        let remote = if self
            .remote_names()
            .iter()
            .any(|candidate| candidate == &upstream_remote)
        {
            upstream_remote
        } else {
            self.default_remote_name()
        };
        let remote_branches = self.remote_branch_names_for(&remote);
        let remote_branch = if !upstream_branch.is_empty()
            && remote_branches
                .iter()
                .any(|candidate| candidate == &upstream_branch)
        {
            upstream_branch
        } else if remote_branches
            .iter()
            .any(|candidate| candidate == &local_branch)
        {
            local_branch.clone()
        } else {
            remote_branches.first().cloned().unwrap_or_default()
        };
        self.pending_pull_action = Some(PullActionDialog {
            remote,
            remote_branch,
            local_branch,
            commit_merge: true,
            include_tags: false,
            force_merge_commit: false,
            rebase: false,
        });
    }

    fn remote_names(&self) -> Vec<String> {
        self.snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .remotes
                    .iter()
                    .map(|remote| remote.name.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn default_remote_name(&self) -> String {
        self.remote_names()
            .into_iter()
            .next()
            .unwrap_or_else(|| "origin".to_owned())
    }

    fn remote_branch_names_for(&self, remote: &str) -> Vec<String> {
        let prefix = format!("{remote}/");
        self.snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .branches
                    .iter()
                    .filter(|branch| branch.remote)
                    .filter_map(|branch| branch.name.strip_prefix(&prefix).map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn remote_url_for_name(&self, remote_name: &str) -> Option<String> {
        self.snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .remotes
                .iter()
                .find(|remote| remote.name == remote_name)
                .map(|remote| {
                    if remote.fetch_url.is_empty() {
                        remote.push_url.clone()
                    } else {
                        remote.fetch_url.clone()
                    }
                })
        })
    }

    fn active_repo_display_name(&self) -> String {
        self.active_repo_root()
            .and_then(|root| {
                root.file_name()
                    .and_then(|name| name.to_str())
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "Repository".to_owned())
    }

    fn push_current(&mut self) {
        self.open_push_dialog(None, None);
    }

    fn quick_push_current(&mut self) {
        self.start_remote_git_action(|root| git::push(root));
    }

    fn open_push_dialog(&mut self, target_branch: Option<String>, target_remote: Option<String>) {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };

        let remote_names = self.remote_names();
        let target_upstream = target_branch
            .as_ref()
            .and_then(|target| {
                snapshot
                    .branches
                    .iter()
                    .find(|branch| !branch.remote && branch.name == *target)
            })
            .and_then(|branch| branch.upstream.as_ref())
            .or_else(|| snapshot.upstream.as_ref());
        let upstream_remote = target_upstream
            .as_ref()
            .and_then(|upstream| split_remote_branch_name(&upstream.name))
            .map(|(remote, _)| remote);
        let mut remote = target_remote
            .or(upstream_remote)
            .unwrap_or_else(|| self.default_remote_name());
        if !remote_names.iter().any(|candidate| candidate == &remote) {
            remote = self.default_remote_name();
        }

        let remote_branches = self.remote_branch_names_for(&remote);
        let selected_branch = target_branch.unwrap_or_else(|| snapshot.branch.clone());
        let rows = snapshot
            .branches
            .iter()
            .filter(|branch| !branch.remote)
            .map(|branch| {
                let selected = branch.name == selected_branch;
                let mut remote_branch =
                    push_remote_branch_default(branch, &remote, &remote_branches);
                if selected && remote_branch.trim().is_empty() {
                    remote_branch = branch.name.clone();
                }
                PushBranchRow {
                    selected,
                    local_branch: branch.name.clone(),
                    remote_branch,
                    track: true,
                    upstream: branch
                        .upstream
                        .as_ref()
                        .map(|upstream| upstream.name.clone()),
                }
            })
            .collect();

        self.pending_push_action = Some(PushActionDialog {
            remote,
            rows,
            push_tags: true,
            force: false,
        });
    }

    fn selected_push_branches(dialog: &PushActionDialog) -> Vec<git::PushBranchSpec> {
        dialog
            .rows
            .iter()
            .filter(|row| row.selected)
            .filter_map(|row| {
                let local_branch = row.local_branch.trim();
                let remote_branch = row.remote_branch.trim();
                (!local_branch.is_empty() && !remote_branch.is_empty()).then(|| {
                    git::PushBranchSpec {
                        local_branch: local_branch.to_owned(),
                        remote_branch: remote_branch.to_owned(),
                        track: row.track,
                    }
                })
            })
            .collect()
    }

    fn update_push_rows_for_remote(rows: &mut [PushBranchRow], remote: &str, branches: &[String]) {
        for row in rows {
            row.remote_branch = push_remote_branch_default_for_row(row, remote, branches);
            if row.selected && row.remote_branch.trim().is_empty() {
                row.remote_branch = row.local_branch.clone();
            }
        }
    }

    fn handle_global_shortcuts(&mut self, ctx: &egui::Context) {
        if stage_toggle_shortcut_pressed(ctx) {
            self.pending_toolbar_single_click = None;
            self.active_view = MainView::Workspace;
            self.focus_commit_message = true;
            let action = self
                .snapshot
                .as_ref()
                .and_then(shortcut_stage_toggle_action);
            if let Some(action) = action {
                self.handle_worktree_action(action);
            }
            return;
        }

        if ctx.wants_keyboard_input() {
            return;
        }

        if shortcut_pressed(ctx, egui::Key::P, true) {
            self.quick_push_current();
        } else if shortcut_pressed(ctx, egui::Key::P, false) {
            self.push_current();
        } else if shortcut_pressed(ctx, egui::Key::L, true) {
            self.quick_pull_current();
        } else if shortcut_pressed(ctx, egui::Key::L, false) {
            self.pull_current();
        } else if shortcut_pressed(ctx, egui::Key::F, true) {
            self.quick_fetch_all();
        } else if shortcut_pressed(ctx, egui::Key::F, false) {
            self.fetch_all();
        }
    }

    fn poll_tasks(&mut self, ctx: &egui::Context) {
        if let Some(receiver) = self.repo_task.take() {
            match receiver.try_recv() {
                Ok((requested_root, Ok(snapshot))) => {
                    self.cache_repository_snapshot(&snapshot);
                    if self.active_repo_root_matches(&requested_root) {
                        self.apply_repository_snapshot(snapshot);
                        self.pending_branch_checkout = None;
                        self.loading_repo = false;
                        self.error = None;
                    }
                    ctx.request_repaint();
                }
                Ok((requested_root, Err(error))) => {
                    if self.active_repo_root_matches(&requested_root) {
                        self.pending_branch_checkout = None;
                        self.clear_repository_snapshot_view();
                        self.loading_repo = false;
                        self.error = Some(error.to_string());
                    }
                    ctx.request_repaint();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.repo_task = Some(receiver);
                    ctx.request_repaint_after(std::time::Duration::from_millis(80));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending_branch_checkout = None;
                    self.loading_repo = false;
                    self.error = Some("Repository loader stopped unexpectedly".to_owned());
                    ctx.request_repaint();
                }
            }
        }

        if let Some(receiver) = self.branch_checkout_task.take() {
            match receiver.try_recv() {
                Ok((root, name, Ok(()))) => {
                    self.error = None;
                    self.last_notice = None;
                    self.pending_branch_checkout = Some(name);
                    self.load_repository_uncached(root);
                    ctx.request_repaint();
                }
                Ok((_, _, Err(error))) => {
                    self.branch_checkout_task = None;
                    self.pending_branch_checkout = None;
                    self.loading_repo = false;
                    self.error = Some(error.to_string());
                    self.last_notice = None;
                    ctx.request_repaint();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.branch_checkout_task = Some(receiver);
                    ctx.request_repaint_after(std::time::Duration::from_millis(80));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending_branch_checkout = None;
                    self.loading_repo = false;
                    self.error = Some("Branch checkout stopped unexpectedly".to_owned());
                    ctx.request_repaint();
                }
            }
        }

        if let Some(receiver) = self.remote_git_task.take() {
            match receiver.try_recv() {
                Ok((root, Ok(()))) => {
                    self.error = None;
                    self.last_notice = None;
                    self.load_repository_uncached(root);
                    ctx.request_repaint();
                }
                Ok((_, Err(error))) => {
                    self.loading_repo = false;
                    self.error = Some(error.to_string());
                    self.last_notice = None;
                    ctx.request_repaint();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.remote_git_task = Some(receiver);
                    ctx.request_repaint_after(std::time::Duration::from_millis(80));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loading_repo = false;
                    self.error = Some("Remote Git action stopped unexpectedly".to_owned());
                    self.last_notice = None;
                    ctx.request_repaint();
                }
            }
        }

        self.poll_merge_tool_task(ctx);

        if let Some(receiver) = self.repo_source_task.take() {
            match receiver.try_recv() {
                Ok(Ok(path)) => {
                    self.last_notice = None;
                    self.load_repository(path);
                    ctx.request_repaint();
                }
                Ok(Err(error)) => {
                    self.error = Some(error.to_string());
                    self.last_notice = None;
                    ctx.request_repaint();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.repo_source_task = Some(receiver);
                    ctx.request_repaint_after(std::time::Duration::from_millis(80));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.error = Some("Repository action stopped unexpectedly".to_owned());
                    self.last_notice = None;
                    ctx.request_repaint();
                }
            }
        }

        if let Some(receiver) = self.clone_url_task.take() {
            match receiver.try_recv() {
                Ok((url, Ok(()))) => {
                    if self.clone_url.trim() == url {
                        self.clone_url_status = CloneUrlStatus::Valid;
                    }
                    ctx.request_repaint();
                }
                Ok((url, Err(error))) => {
                    if self.clone_url.trim() == url {
                        let message = error.to_string();
                        self.clone_url_status = CloneUrlStatus::Invalid(message);
                        self.clone_destination.clear();
                    }
                    ctx.request_repaint();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.clone_url_task = Some(receiver);
                    ctx.request_repaint_after(Duration::from_millis(80));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.clone_url_status =
                        CloneUrlStatus::Invalid("Remote URL validation stopped".to_owned());
                    self.clone_destination.clear();
                    ctx.request_repaint();
                }
            }
        }

        if let Some(receiver) = self.file_search_task.take() {
            match receiver.try_recv() {
                Ok((query, Ok(hashes))) => {
                    self.file_search_started_at = None;
                    if self.search_dimension == SearchDimension::Files
                        && self.search_view_query.trim().to_lowercase() == query
                    {
                        self.file_search_query = query;
                        self.file_search_hashes = hashes.into_iter().collect();
                        self.search_selected_commit =
                            self.search_filtered_commit_indices().first().copied();
                        self.search_selected_file_path = None;
                        self.search_selected_diff_rows.clear();
                        if !self.select_first_search_changed_file_if_cached() {
                            self.request_selected_search_details();
                        }
                    }
                    ctx.request_repaint();
                }
                Ok((query, Err(error))) => {
                    self.file_search_started_at = None;
                    if self.search_view_query.trim().to_lowercase() == query {
                        self.error = Some(error.to_string());
                    }
                    ctx.request_repaint();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    if self
                        .file_search_started_at
                        .is_some_and(|started| started.elapsed() > FILE_SEARCH_TIMEOUT)
                    {
                        self.file_search_started_at = None;
                        self.error = Some("File search timed out".to_owned());
                        ctx.request_repaint();
                    } else {
                        self.file_search_task = Some(receiver);
                        ctx.request_repaint_after(Duration::from_millis(80));
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.file_search_started_at = None;
                    self.error = Some("File search stopped unexpectedly".to_owned());
                    ctx.request_repaint();
                }
            }
        }

        if let Some(receiver) = self.details_task.take() {
            match receiver.try_recv() {
                Ok(Ok(details)) => {
                    let should_autoselect_history = self.selected_commit_hash()
                        == Some(details.hash.as_str())
                        && self.selected_file_path.is_none();
                    let should_autoselect_search = self.selected_search_commit_hash()
                        == Some(details.hash.as_str())
                        && self.search_selected_file_path.is_none();
                    let first_file = details.files.first().map(|file| file.diff_path.clone());
                    self.loading_details_hash = None;
                    self.details_cache.insert(details.hash.clone(), details);
                    self.history_rows_cache.clear();
                    if let Some(path) = first_file {
                        if should_autoselect_history {
                            self.select_changed_file_for_diff(path.clone());
                        }
                        if should_autoselect_search {
                            self.select_search_changed_file_for_diff(path);
                        }
                    }
                    ctx.request_repaint();
                }
                Ok(Err(error)) => {
                    self.loading_details_hash = None;
                    self.error = Some(error.to_string());
                    ctx.request_repaint();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.details_task = Some(receiver);
                    ctx.request_repaint_after(std::time::Duration::from_millis(80));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loading_details_hash = None;
                    self.error = Some("Commit details loader stopped unexpectedly".to_owned());
                    ctx.request_repaint();
                }
            }
        }

        if let Some(receiver) = self.diff_task.take() {
            match receiver.try_recv() {
                Ok(Ok(diff)) => {
                    self.loading_diff_key = None;
                    self.diff_cache.insert(diff.key.clone(), diff);
                    ctx.request_repaint();
                }
                Ok(Err(error)) => {
                    self.loading_diff_key = None;
                    self.error = Some(error.to_string());
                    ctx.request_repaint();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.diff_task = Some(receiver);
                    ctx.request_repaint_after(std::time::Duration::from_millis(80));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loading_diff_key = None;
                    self.error = Some("Diff loader stopped unexpectedly".to_owned());
                    ctx.request_repaint();
                }
            }
        }
    }

    fn request_selected_details(&mut self) {
        self.request_commit_details_for(self.selected_commit);
    }

    fn request_selected_search_details(&mut self) {
        self.request_commit_details_for(self.search_selected_commit);
    }

    fn request_commit_details_for(&mut self, selected_commit: Option<usize>) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let Some(commit) = selected_commit.and_then(|index| snapshot.commits.get(index)) else {
            return;
        };

        if self.details_cache.contains_key(&commit.hash)
            || self.loading_details_hash.as_deref() == Some(commit.hash.as_str())
        {
            return;
        }

        let root = snapshot.root.clone();
        let hash = commit.hash.clone();
        let (sender, receiver) = mpsc::channel();
        self.details_task = Some(receiver);
        self.loading_details_hash = Some(hash.clone());

        thread::spawn(move || {
            let _ = sender.send(git::load_commit_details(root, &hash));
        });
    }

    fn selected_commit_hash(&self) -> Option<&str> {
        let snapshot = self.snapshot.as_ref()?;
        let commit = self
            .selected_commit
            .and_then(|index| snapshot.commits.get(index))
            .or_else(|| snapshot.commits.first())?;
        Some(commit.hash.as_str())
    }

    fn selected_search_commit_hash(&self) -> Option<&str> {
        let snapshot = self.snapshot.as_ref()?;
        let commit = self
            .search_selected_commit
            .and_then(|index| snapshot.commits.get(index))?;
        Some(commit.hash.as_str())
    }

    fn toggle_cherry_pick_hash(&mut self, hash: String) {
        if !self.selected_cherry_pick_hashes.insert(hash.clone()) {
            self.selected_cherry_pick_hashes.remove(&hash);
        }
    }

    fn selected_cherry_pick_commits_in_apply_order(&self) -> Vec<Commit> {
        let Some(snapshot) = &self.snapshot else {
            return Vec::new();
        };
        snapshot
            .commits
            .iter()
            .rev()
            .filter(|commit| self.selected_cherry_pick_hashes.contains(&commit.hash))
            .cloned()
            .collect()
    }

    fn confirm_selected_cherry_picks(&mut self) {
        let commits = self.selected_cherry_pick_commits_in_apply_order();
        if commits.is_empty() {
            return;
        }
        self.pending_commit_action = Some(CommitActionDialog::ConfirmCherryPickBatch {
            hashes: commits.iter().map(|commit| commit.hash.clone()).collect(),
            short_hashes: commits
                .iter()
                .map(|commit| commit.short_hash.clone())
                .collect(),
        });
    }

    fn clear_cherry_pick_selection(&mut self) {
        self.history_cherry_pick_mode = false;
        self.selected_cherry_pick_hashes.clear();
    }

    fn select_changed_file_for_diff(&mut self, path: String) {
        self.selected_file_path = Some(path);
        self.selected_worktree_file = None;
        self.history_diff_display_mode = DiffDisplayMode::Blocks;
        self.selected_diff_rows.clear();
        self.request_selected_file_diff();
    }

    fn select_first_history_changed_file_if_cached(&mut self) -> bool {
        let Some(hash) = self.selected_commit_hash().map(str::to_owned) else {
            return false;
        };
        let Some(path) = self
            .details_cache
            .get(&hash)
            .and_then(|details| details.files.first())
            .map(|file| file.diff_path.clone())
        else {
            return false;
        };
        self.select_changed_file_for_diff(path);
        true
    }

    fn select_search_changed_file_for_diff(&mut self, path: String) {
        self.search_selected_file_path = Some(path);
        self.search_diff_display_mode = DiffDisplayMode::Blocks;
        self.search_selected_diff_rows.clear();
        self.request_selected_search_file_diff();
    }

    fn select_first_search_changed_file_if_cached(&mut self) -> bool {
        let Some(hash) = self.selected_search_commit_hash().map(str::to_owned) else {
            return false;
        };
        let Some(path) = self
            .details_cache
            .get(&hash)
            .and_then(|details| details.files.first())
            .map(|file| file.diff_path.clone())
        else {
            return false;
        };
        self.select_search_changed_file_for_diff(path);
        true
    }

    fn request_selected_file_diff(&mut self) {
        let Some(hash) = self.selected_commit_hash().map(str::to_owned) else {
            return;
        };
        let Some(path) = self.selected_file_path.clone() else {
            return;
        };
        self.request_file_diff(hash, path);
    }

    fn request_selected_search_file_diff(&mut self) {
        let Some(hash) = self.selected_search_commit_hash().map(str::to_owned) else {
            return;
        };
        let Some(path) = self.search_selected_file_path.clone() else {
            return;
        };
        self.request_file_diff(hash, path);
    }

    fn request_file_diff(&mut self, hash: String, path: String) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };

        let key = git::diff_key(&hash, &path);
        if self.diff_cache.contains_key(&key) || self.loading_diff_key.as_deref() == Some(&key) {
            return;
        }

        let root = snapshot.root.clone();
        let (sender, receiver) = mpsc::channel();
        self.diff_task = Some(receiver);
        self.loading_diff_key = Some(key);

        thread::spawn(move || {
            let _ = sender.send(git::load_file_diff(root, &hash, &path));
        });
    }

    fn request_selected_worktree_diff(&mut self) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let Some(selected) = self.selected_worktree_file.clone() else {
            return;
        };

        let key = git::worktree_diff_key(&selected.path, selected.staged);
        if self.diff_cache.contains_key(&key) || self.loading_diff_key.as_deref() == Some(&key) {
            return;
        }

        let root = snapshot.root.clone();
        let (sender, receiver) = mpsc::channel();
        self.diff_task = Some(receiver);
        self.loading_diff_key = Some(key);
        let untracked = selected.untracked;

        thread::spawn(move || {
            let _ = sender.send(git::load_worktree_diff(
                root,
                &selected.path,
                selected.staged,
                untracked,
            ));
        });
    }

    fn start_file_change_search(&mut self) {
        let query = self.search_view_query.trim().to_lowercase();
        if query.is_empty() {
            self.file_search_task = None;
            self.file_search_started_at = None;
            self.file_search_query.clear();
            self.file_search_hashes.clear();
            self.search_selected_commit = None;
            self.search_selected_file_path = None;
            self.search_selected_diff_rows.clear();
            return;
        }
        let Some(root) = self.snapshot.as_ref().map(|snapshot| snapshot.root.clone()) else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        self.file_search_task = Some(receiver);
        self.file_search_started_at = Some(Instant::now());
        self.error = None;
        thread::spawn(move || {
            let result = git::search_commits_by_changed_file(root, &query);
            let _ = sender.send((query, result));
        });
    }

    fn search_filtered_commit_indices(&self) -> Vec<usize> {
        let Some(snapshot) = &self.snapshot else {
            return Vec::new();
        };
        let query = self.search_view_query.trim().to_lowercase();
        if query.is_empty() {
            return (0..snapshot.commits.len()).collect();
        }

        snapshot
            .commits
            .iter()
            .enumerate()
            .filter_map(|(index, commit)| {
                let matches = history_commit_matches_search(
                    commit,
                    self.search_dimension,
                    &query,
                    &self.details_cache,
                    &self.file_search_query,
                    &self.file_search_hashes,
                );
                matches.then_some(index)
            })
            .collect()
    }

    fn refresh_history_rows_cache(&mut self) -> usize {
        let Some(snapshot) = &self.snapshot else {
            self.history_rows_cache.clear();
            return 0;
        };
        let key = HistoryRowsCacheKey {
            scope: self.history_branch_scope,
            sort_order: self.history_sort_order,
            search_dimension: SearchDimension::Message,
            search: self.search.trim().to_owned(),
            commit_count: snapshot.commits.len(),
            first_hash: snapshot
                .commits
                .first()
                .map(|commit| commit.hash.clone())
                .unwrap_or_default(),
            last_hash: snapshot
                .commits
                .last()
                .map(|commit| commit.hash.clone())
                .unwrap_or_default(),
            details_cache_len: self.details_cache.len(),
            file_search_query: String::new(),
            file_search_hash_count: 0,
        };
        self.history_rows_cache
            .refresh(key, snapshot, &self.details_cache, "", &HashSet::new());
        self.history_rows_cache.len()
    }

    fn tr(&self, key: &'static str) -> &'static str {
        i18n::t(self.language, key)
    }

    fn top_bar_height(&self) -> f32 {
        if self.repository_source_active() {
            TITLE_BAR_HEIGHT + TOP_BAR_ROW_HEIGHT
        } else {
            TOP_BAR_HEIGHT
        }
    }

    fn set_theme_mode(&mut self, mode: theme::ThemeMode) {
        if self.theme_mode != mode {
            self.theme_mode = mode;
            self.save_app_settings();
        }
    }

    fn set_theme_accent(&mut self, accent: theme::ThemeAccent) {
        if self.theme_accent != accent {
            self.theme_accent = accent;
            self.save_app_settings();
        }
    }

    fn set_language(&mut self, language: Language) {
        if self.language != language {
            self.language = language;
            self.save_app_settings();
        }
    }

    fn save_app_settings(&self) {
        AppSettings {
            theme: self.theme_mode.into(),
            theme_accent: self.theme_accent.into(),
            language: self.language.into(),
            remote_accounts: self.remote_accounts.clone(),
        }
        .save();
    }

    fn save_repo_tabs(&self) {
        RepoTabsState::from_app(self).save();
    }

    fn load_commit_state_for_active_repo(&mut self) {
        self.commit_state = self
            .active_repo_root()
            .as_deref()
            .map(RepoCommitStateStore::state_for)
            .unwrap_or_default();
    }

    fn save_commit_state_for_active_repo(&self) {
        if let Some(root) = self.active_repo_root() {
            RepoCommitStateStore::save_for(&root, &self.commit_state);
        }
    }

    fn add_commit_message_history(&mut self, message: String) {
        let message = message.trim().to_owned();
        if message.is_empty() {
            return;
        }
        self.commit_state
            .message_history
            .retain(|entry| entry != &message);
        self.commit_state.message_history.insert(0, message);
        self.commit_state.message_history.truncate(24);
        self.save_commit_state_for_active_repo();
    }

    fn remove_commit_message_history(&mut self, index: usize) {
        if index < self.commit_state.message_history.len() {
            self.commit_state.message_history.remove(index);
            self.save_commit_state_for_active_repo();
        }
    }

    fn toggle_push_immediately(&mut self) {
        self.commit_state.push_immediately = !self.commit_state.push_immediately;
        self.save_commit_state_for_active_repo();
    }

    fn toggle_amend(&mut self) {
        self.commit_state.amend = !self.commit_state.amend;
        self.save_commit_state_for_active_repo();
    }

    fn commit_current_message(&mut self, staged_count: usize) {
        if self.loading_repo || self.remote_git_busy() {
            return;
        }
        if (staged_count == 0 && !self.commit_state.amend) || self.commit_message.trim().is_empty()
        {
            return;
        }

        let message = self.commit_message.trim().to_owned();
        let options = git::CommitOptions {
            amend: self.commit_state.amend,
            no_verify: self.commit_state.no_verify,
            gpg_sign: self.commit_state.gpg_sign,
        };
        let push_immediately = self.commit_state.push_immediately;
        let push_target = self.snapshot.as_ref().and_then(|snapshot| {
            let branch = snapshot.branch.clone();
            if branch.is_empty() {
                None
            } else {
                Some((
                    snapshot
                        .remotes
                        .first()
                        .map(|remote| remote.name.clone())
                        .unwrap_or_else(|| "origin".to_owned()),
                    branch,
                    snapshot.upstream.is_some(),
                ))
            }
        });
        self.add_commit_message_history(message.clone());
        self.start_remote_git_action(move |root| {
            git::commit_with_options(root, &message, options)?;
            if push_immediately {
                if let Some((remote, branch, has_upstream)) = push_target {
                    if has_upstream {
                        git::push(root)
                    } else {
                        git::push_set_upstream(root, &remote, &branch)
                    }
                } else {
                    git::push(root)
                }
            } else {
                Ok(())
            }
        });
        self.commit_message.clear();
    }

    fn show_toast(&mut self, message: impl Into<String>) {
        self.toast_notice = Some((message.into(), Instant::now() + Duration::from_secs(1)));
    }

    fn toast_overlay(&mut self, ctx: &egui::Context) {
        let Some((message, until)) = self.toast_notice.clone() else {
            return;
        };
        let now = Instant::now();
        if now >= until {
            self.toast_notice = None;
            return;
        }

        ctx.request_repaint_after(until - now);
        egui::Area::new(egui::Id::new("transient_toast"))
            .anchor(
                Align2::CENTER_TOP,
                Vec2::new(0.0, self.top_bar_height() + 14.0),
            )
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(38, 48, 62, 232))
                    .stroke(Stroke::new(
                        1.0,
                        Color32::from_rgba_unmultiplied(255, 255, 255, 38),
                    ))
                    .corner_radius(CornerRadius::same(4))
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.label(RichText::new(message).size(12.0).color(Color32::WHITE));
                    });
            });
    }
}

impl App for GitAgentApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::apply(ctx, self.theme_mode, self.theme_accent);
        self.poll_tasks(ctx);
        self.maybe_start_clone_url_validation(ctx);
        self.handle_global_shortcuts(ctx);
        self.flush_pending_toolbar_single_click(ctx);
        if ctx.input(|input| input.modifiers.ctrl && input.key_pressed(egui::Key::Comma)) {
            self.settings_tab = SettingsTab::General;
            self.settings_open = true;
        }

        egui::TopBottomPanel::top("top_bar")
            .exact_height(self.top_bar_height())
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(theme::panel()).stroke(Stroke::NONE))
            .show(ctx, |ui| self.top_bar(ctx, ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::bg())
                    .inner_margin(central_panel_margin(self.repository_source_active())),
            )
            .show(ctx, |ui| self.main_layout(ui));

        self.commit_action_modal(ctx);
        self.worktree_action_modal(ctx);
        self.fetch_action_modal(ctx);
        self.pull_action_modal(ctx);
        self.push_action_modal(ctx);
        self.stash_action_modal(ctx);
        self.branch_action_modal(ctx);
        self.tag_action_modal(ctx);
        self.settings_modal(ctx);
        self.repo_settings_modal(ctx);
        self.repo_remote_action_modal(ctx);
        self.error_modal(ctx);
        self.toast_overlay(ctx);
    }
}

impl GitAgentApp {
    fn error_modal(&mut self, ctx: &egui::Context) {
        let Some(error) = self.error.clone() else {
            return;
        };

        let mut close_requested = false;
        let screen = ctx.screen_rect();
        let size = Vec2::new(
            (screen.width() * 0.54).clamp(460.0, 760.0),
            (screen.height() * 0.42).clamp(260.0, 480.0),
        );
        let mut message = error;

        compact_action_dialog(ctx, self.tr("dialog.error.title"), size.x, |ui| {
            safe_set_min_size(ui, Vec2::new((size.x - 24.0).max(420.0), 220.0));
            ui.label(
                RichText::new(self.tr("dialog.error.message"))
                    .small()
                    .color(theme::muted()),
            );
            ui.add_space(8.0);
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height((size.y - 120.0).max(120.0))
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut message)
                            .font(FontId::monospace(12.0))
                            .desired_width(f32::INFINITY)
                            .desired_rows(10)
                            .interactive(false),
                    );
                });
            ui.add_space(10.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(self.tr("dialog.close")).clicked() {
                    close_requested = true;
                }
                if ui.button(self.tr("menu.copy")).clicked() {
                    ui.ctx().copy_text(message.clone());
                }
            });
        });

        if close_requested {
            self.error = None;
        }
    }

    fn main_layout(&mut self, ui: &mut Ui) {
        let full = ui.available_rect_before_wrap();
        let details_visible = view_uses_side_details(self.active_view);
        self.layout_prefs.clamp();
        let layout = main_layout_rects(
            full,
            self.layout_prefs.sidebar_pct,
            self.layout_prefs.details_pct,
            details_visible,
        );
        let full_width = layout.content.width();
        if self.repository_source_active() {
            exact_panel_at_rect(
                ui,
                layout.content,
                theme::panel(),
                CONTENT_PANEL_INSET_X,
                repository_source_panel_y_margin(),
                |ui| {
                    self.repository_source_view(ui);
                },
            );
            ui.allocate_rect(full, Sense::hover());
            return;
        }

        let mut sidebar_width = layout.sidebar.width();
        let mut details_width = layout.details.width();
        let min_sidebar = 200.0;
        let min_details = if details_visible { 320.0 } else { 0.0 };
        let sidebar_rect = layout.sidebar;
        let center_rect = layout.center;
        let details_rect = layout.details;
        let sidebar_center_gap = layout.sidebar_center_gap;
        let center_details_gap = layout.center_details_gap;

        exact_panel_at_rect(
            ui,
            sidebar_rect,
            theme::panel(),
            LAYOUT_GAP,
            LAYOUT_GAP,
            |ui| {
                self.sidebar(ui);
            },
        );
        exact_panel_at_rect(
            ui,
            center_rect,
            theme::panel(),
            CONTENT_PANEL_INSET_X,
            CONTENT_PANEL_INSET_Y,
            |ui| match self.active_view {
                MainView::Workspace => self.workspace_view(ui),
                MainView::History => self.history_view(ui),
                MainView::Search => self.search_view(ui),
                MainView::Branches => self.branches_view(ui),
                MainView::Tags => self.tags_view(ui),
                MainView::Stashes => self.stashes_view(ui),
            },
        );
        if details_visible {
            exact_panel_at_rect(
                ui,
                details_rect,
                theme::panel(),
                CONTENT_PANEL_INSET_X,
                CONTENT_PANEL_INSET_Y,
                |ui| {
                    self.details(ui);
                },
            );
        }
        paint_layout_debug_rect(ui, layout.content, "main.content", Color32::YELLOW);
        paint_layout_debug_rect(ui, layout.sidebar, "main.sidebar", Color32::LIGHT_BLUE);
        paint_layout_debug_rect(ui, layout.center, "main.center", Color32::LIGHT_GREEN);
        if details_visible {
            paint_layout_debug_rect(ui, layout.details, "main.details", Color32::LIGHT_RED);
        }
        log_layout_debug_once(
            &MAIN_LAYOUT_DEBUG_LOGGED,
            "main",
            &[
                ("content", layout.content),
                ("sidebar", layout.sidebar),
                ("center", layout.center),
                ("details", layout.details),
            ],
        );
        if let Some(delta) = vertical_resize_delta(ui, sidebar_center_gap, "sidebar_center_resize")
        {
            sidebar_width = (sidebar_width + delta).clamp(min_sidebar, full_width * 0.34);
            self.layout_prefs.sidebar_pct = sidebar_width / full_width;
            self.layout_prefs.clamp();
            self.layout_prefs.save();
        }
        if details_visible {
            if let Some(delta) =
                vertical_resize_delta(ui, center_details_gap, "center_details_resize")
            {
                details_width = (details_width - delta).clamp(min_details, full_width * 0.46);
                self.layout_prefs.details_pct = details_width / full_width;
                self.layout_prefs.clamp();
                self.layout_prefs.save();
            }
        }
        ui.allocate_rect(full, Sense::hover());
    }

    fn top_bar(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        self.top_bar_panel(ctx, ui);
    }

    fn top_bar_drag_region(
        &mut self,
        ctx: &egui::Context,
        ui: &mut Ui,
        rect: Rect,
        id_salt: &'static str,
    ) -> egui::Response {
        let response = ui.interact(rect, ui.id().with(id_salt), Sense::click_and_drag());
        if response.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        response
    }

    fn custom_title_bar(
        &mut self,
        ctx: &egui::Context,
        ui: &mut Ui,
        has_repo: bool,
        has_remote: bool,
    ) {
        let rect = ui.max_rect();
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, theme::panel());

        let controls_width = 128.0;
        let drag_rect = custom_title_drag_rect(rect, controls_width);
        let drag_response =
            self.top_bar_drag_region(ctx, ui, drag_rect, "custom_title_drag_region");
        if drag_response.double_clicked() {
            let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(8.0);
                app_title_logo(ui);
                self.desktop_menu_bar(ui, has_repo, has_remote);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(8.0);
                    if window_control_button(ui, "\u{00d7}", true).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
                    if window_control_button(
                        ui,
                        if maximized { "\u{2750}" } else { "\u{25a1}" },
                        false,
                    )
                    .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    }
                    if window_control_button(ui, "\u{2212}", false).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                });
            });
        });
    }

    fn desktop_menu_bar(&mut self, ui: &mut Ui, has_repo: bool, has_remote: bool) {
        let repo_action_busy = self.loading_repo || self.remote_git_busy();
        ui.horizontal_centered(|ui| {
            ui.add_space(8.0);
            menu_button(ui, menu_label(self.language, "file"), |ui| {
                if ui.button(self.tr("action.open")).clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.load_repository(path);
                    }
                    ui.close_menu();
                }
                if ui
                    .add_enabled(has_repo, egui::Button::new(self.tr("action.refresh")))
                    .clicked()
                {
                    self.refresh();
                    ui.close_menu();
                }
            });
            menu_button(ui, menu_label(self.language, "edit"), |ui| {
                ui.add_enabled(false, egui::Button::new(menu_label(self.language, "undo")));
                ui.add_enabled(false, egui::Button::new(menu_label(self.language, "redo")));
            });
            menu_button(ui, menu_label(self.language, "view"), |ui| {
                let light = self.theme_mode == theme::ThemeMode::Light;
                if ui
                    .selectable_label(!light, menu_label(self.language, "dark_theme"))
                    .clicked()
                {
                    self.set_theme_mode(theme::ThemeMode::Dark);
                    ui.close_menu();
                }
                if ui
                    .selectable_label(light, menu_label(self.language, "light_theme"))
                    .clicked()
                {
                    self.set_theme_mode(theme::ThemeMode::Light);
                    ui.close_menu();
                }
            });
            menu_button(ui, menu_label(self.language, "repo"), |ui| {
                if ui
                    .add_enabled(
                        !repo_action_busy && has_remote,
                        egui::Button::new(self.tr("action.fetch")),
                    )
                    .clicked()
                {
                    self.fetch_all();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        !repo_action_busy && has_remote,
                        egui::Button::new(self.tr("action.pull")),
                    )
                    .clicked()
                {
                    self.pull_current();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        !repo_action_busy && has_remote,
                        egui::Button::new(self.tr("action.push")),
                    )
                    .clicked()
                {
                    self.push_current();
                    ui.close_menu();
                }
            });
            menu_button(ui, menu_label(self.language, "actions"), |ui| {
                if ui
                    .add_enabled(has_repo, egui::Button::new(self.tr("branch.local")))
                    .clicked()
                {
                    self.active_view = MainView::Branches;
                    ui.close_menu();
                }
                if ui
                    .add_enabled(has_repo, egui::Button::new(self.tr("tag.title")))
                    .clicked()
                {
                    self.active_view = MainView::Tags;
                    ui.close_menu();
                }
                if ui
                    .add_enabled(has_repo, egui::Button::new(self.tr("stash.title")))
                    .clicked()
                {
                    self.active_view = MainView::Stashes;
                    ui.close_menu();
                }
            });
            menu_button(ui, menu_label(self.language, "tools"), |ui| {
                ui.add_enabled(
                    false,
                    egui::Button::new(menu_label(self.language, "ssh_agent")),
                );
                ui.add_enabled(
                    false,
                    egui::Button::new(menu_label(self.language, "process_viewer")),
                );
                if ui.button(menu_label(self.language, "options")).clicked() {
                    self.settings_tab = SettingsTab::General;
                    self.settings_open = true;
                    ui.close_menu();
                }
            });
            menu_button(ui, menu_label(self.language, "help"), |ui| {
                ui.add_enabled(false, egui::Button::new(menu_label(self.language, "about")));
            });
        });
    }

    fn top_bar_panel(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        let has_repo = self.active_repo_tab.is_some() && self.snapshot.is_some();
        let has_remote = self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| !snapshot.remotes.is_empty());
        let mut switch_to = None;
        let mut close_repo_tab = None;
        let mut close_source_tab = false;
        let mut reorder_repo_tab = None;

        let full = ui.max_rect();
        let top_y = full.top();
        let top_bar_height = self.top_bar_height();
        let title_row = Rect::from_min_max(
            full.left_top(),
            Pos2::new(full.right(), top_y + TITLE_BAR_HEIGHT),
        );
        let tab_row = Rect::from_min_max(
            Pos2::new(full.left(), title_row.bottom()),
            Pos2::new(full.right(), title_row.bottom() + TOP_BAR_ROW_HEIGHT),
        );
        let tool_row = Rect::from_min_max(
            Pos2::new(full.left(), tab_row.bottom()),
            Pos2::new(full.right(), top_y + top_bar_height),
        );
        let source_active = self.repository_source_active();
        let tab_strip_row = repo_tab_strip_rect(tab_row, source_active);
        let top_island_rect = top_island_rect(full, title_row, tool_row, source_active);
        ui.painter()
            .rect_filled(full, CornerRadius::ZERO, theme::bg());
        ui.painter().add(
            egui::Frame::new()
                .fill(theme::panel_soft())
                .corner_radius(CornerRadius::same(6))
                .shadow(panel_shadow())
                .paint(top_island_rect),
        );
        paint_layout_debug_rect(ui, top_island_rect, "top.island", Color32::LIGHT_BLUE);
        paint_layout_debug_rect(ui, tab_strip_row, "top.tabs", Color32::LIGHT_GREEN);
        let tool_row_panel_rect = (!source_active).then(|| {
            Rect::from_min_max(
                Pos2::new(
                    full.left() + TOP_BAR_PANEL_X_INSET,
                    tab_row.bottom() - TOP_BAR_TAB_TOOL_JOIN_OVERLAP,
                ),
                Pos2::new(
                    full.right() - TOP_BAR_PANEL_X_INSET,
                    tool_row.bottom() - 4.0,
                ),
            )
        });
        let tool_content_row = tool_row_panel_rect
            .map(|rect| Rect::from_min_max(rect.left_top(), tool_row.right_bottom()));
        if let Some(tool_row_panel_rect) = tool_row_panel_rect {
            ui.painter().add(
                egui::Frame::new()
                    .fill(theme::panel())
                    .corner_radius(tool_row_corners())
                    .paint(tool_row_panel_rect),
            );
            paint_layout_debug_rect(ui, tool_row_panel_rect, "top.toolbar", Color32::LIGHT_RED);
        }

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(title_row), |ui| {
            self.custom_title_bar(ctx, ui, has_repo, has_remote);
        });
        if source_active {
            let title_gap_drag_rect = Rect::from_min_max(
                Pos2::new(full.left(), title_row.bottom()),
                Pos2::new(full.right(), tab_strip_row.top()),
            );
            self.top_bar_drag_region(ctx, ui, title_gap_drag_rect, "source_title_gap_drag_region");
        }

        let tab_left = Rect::from_min_max(
            tab_strip_row.left_top(),
            Pos2::new(
                (tab_strip_row.right() - TOP_BAR_GLOBAL_WIDTH)
                    .max(tab_strip_row.left() + TOP_BAR_MIN_TABS_WIDTH),
                tab_strip_row.bottom(),
            ),
        );
        let tab_right = Rect::from_min_max(
            Pos2::new(tab_left.right(), tab_strip_row.top()),
            tab_strip_row.right_bottom(),
        );
        self.top_bar_drag_region(ctx, ui, tab_right, "tab_right_drag_region");
        if source_active {
            self.top_bar_drag_region(ctx, ui, tab_left, "source_tab_left_drag_region");
        }
        let repo_tab_names = self
            .repo_tabs
            .iter()
            .map(|tab| tab.name.clone())
            .collect::<Vec<_>>();
        let active_repo_tab = self.active_repo_tab;
        let source_tab_open = self.source_tab_open;
        let new_tab_label = self.tr("repo.source.new_tab").to_owned();
        let close_tab_label = self.tr("repo.source.close_tab").to_owned();
        let more_label = self.tr("common.more").to_owned();
        let loading_repo = self.loading_repo;
        let mut activate_source_tab = false;
        let mut open_source_tab = false;

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(tab_left), |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = REPO_TAB_ITEM_GAP;
                ui.add_space(REPO_TAB_STRIP_LEFT_PADDING);
                let repo_tab_widths = repo_tab_names
                    .iter()
                    .map(|name| repo_tab_width(ui, name))
                    .collect::<Vec<_>>();
                let source_width = source_tab_open.then(|| repo_tab_width(ui, &new_tab_label));
                let visibility = repo_tab_visibility(
                    &repo_tab_widths,
                    source_width,
                    active_repo_tab,
                    source_active,
                    tab_left.width(),
                );

                if visibility.has_leading_overflow() {
                    repo_tab_overflow_menu(
                        ui,
                        "leading",
                        &more_label,
                        &visibility.leading_overflow_items,
                        &repo_tab_names,
                        active_repo_tab,
                        source_active,
                        &new_tab_label,
                        &mut switch_to,
                        &mut activate_source_tab,
                    );
                }
                for item in visibility.visible_items.iter().copied() {
                    match item {
                        RepoTabVisibilityItem::Repo(index) => {
                            let interaction = repo_tab_with_close(
                                ui,
                                UiIcon::Folder,
                                active_repo_tab == Some(index),
                                &repo_tab_names[index],
                                &close_tab_label,
                            );
                            if interaction.close_clicked {
                                close_repo_tab = Some(index);
                            } else if interaction.response.drag_started() {
                                self.repo_tab_drag.dragging_index = Some(index);
                            } else if let Some(from) = self.repo_tab_drag.dragging_index {
                                if from != index && interaction.response.hovered() {
                                    reorder_repo_tab = Some((from, index));
                                }
                            } else if interaction.tab_clicked {
                                switch_to = Some(index);
                            }
                        }
                        RepoTabVisibilityItem::Source if source_tab_open => {
                            let interaction = repo_tab_with_close(
                                ui,
                                UiIcon::Folder,
                                source_active,
                                &new_tab_label,
                                &close_tab_label,
                            );
                            if interaction.close_clicked {
                                close_source_tab = true;
                            } else if interaction.tab_clicked {
                                activate_source_tab = true;
                            }
                        }
                        RepoTabVisibilityItem::Source => {}
                    }
                }
                if visibility.has_trailing_overflow() {
                    repo_tab_overflow_menu(
                        ui,
                        "trailing",
                        &more_label,
                        &visibility.trailing_overflow_items,
                        &repo_tab_names,
                        active_repo_tab,
                        source_active,
                        &new_tab_label,
                        &mut switch_to,
                        &mut activate_source_tab,
                    );
                }
                if icon_button(ui, UiIcon::Plus, &new_tab_label, !loading_repo).clicked() {
                    open_source_tab = true;
                }
            });
        });

        if !ui.input(|input| input.pointer.primary_down()) {
            self.repo_tab_drag.dragging_index = None;
        }

        let global_action_row = tab_right.translate(Vec2::new(0.0, TOP_BAR_GLOBAL_ACTION_Y_OFFSET));
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(global_action_row), |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                if toolbar_button(ui, "settings", self.tr("repo.settings"), has_repo).clicked() {
                    self.repo_settings_tab = SettingsTab::RepoRemotes;
                    self.repo_settings_open = true;
                }
                if toolbar_button(
                    ui,
                    "resource-manager",
                    self.tr("repo.resource_manager"),
                    has_repo,
                )
                .clicked()
                {
                    self.open_resource_manager();
                }
                if toolbar_button(ui, "terminal", self.tr("repo.command_mode"), has_repo).clicked()
                {
                    self.open_command_mode();
                }
                if toolbar_button(ui, "remote", self.tr("repo.remote"), has_repo).clicked() {
                    self.open_remote_url();
                }
                if toolbar_button(ui, "git-flow", self.tr("repo.git_flow"), has_repo).clicked() {
                    self.open_git_workflow();
                }
            });
        });

        if let Some(tool_content_row) = tool_content_row {
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(tool_content_row), |ui| {
                let repo_toolbar_loading = self.repo_toolbar_loading_busy();
                let repo_action_busy = self.loading_repo || self.remote_git_busy();
                let branch_actions_enabled = !self.branch_actions_busy();
                let upstream_counts = upstream_sync_counts(self.snapshot.as_ref());
                let pull_label = toolbar_sync_label(
                    self.tr("action.pull"),
                    upstream_pull_badge(Some(upstream_counts)),
                );
                let push_label = toolbar_sync_label(
                    self.tr("action.push"),
                    upstream_push_badge(Some(upstream_counts)),
                );
                if repo_toolbar_loading {
                    repo_toolbar_loading_indicator(ui);
                } else {
                    ScrollArea::horizontal()
                        .id_salt("repo_toolbar_strip")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.horizontal_centered(|ui| {
                                ui.add_space(16.0);
                                if toolbar_button(ui, "commit", self.tr("commit.panel"), true)
                                    .clicked()
                                {
                                    self.active_view = MainView::Workspace;
                                }
                                let pull_response = toolbar_button(
                                    ui,
                                    "pull",
                                    &pull_label,
                                    !repo_action_busy && has_repo && has_remote,
                                );
                                self.handle_repo_toolbar_action_response(
                                    ctx,
                                    pull_response,
                                    RepoToolbarAction::Pull,
                                );
                                let push_response = toolbar_button(
                                    ui,
                                    "push",
                                    &push_label,
                                    !repo_action_busy && has_repo && has_remote,
                                );
                                self.handle_repo_toolbar_action_response(
                                    ctx,
                                    push_response,
                                    RepoToolbarAction::Push,
                                );
                                let fetch_response = toolbar_button(
                                    ui,
                                    "fetch",
                                    self.tr("action.fetch"),
                                    !repo_action_busy && has_repo && has_remote,
                                );
                                self.handle_repo_toolbar_action_response(
                                    ctx,
                                    fetch_response,
                                    RepoToolbarAction::Fetch,
                                );
                                ui.add_space(LAYOUT_GAP as f32);
                                if toolbar_button(
                                    ui,
                                    "branch",
                                    self.tr("branch.title"),
                                    has_repo && branch_actions_enabled,
                                )
                                .clicked()
                                {
                                    self.handle_branch_action(BranchMenuAction::Create);
                                }
                                if toolbar_button(ui, "tag", self.tr("tag.title"), has_repo)
                                    .clicked()
                                {
                                    self.active_view = MainView::Tags;
                                }
                                if toolbar_button(ui, "stash", self.tr("stash.title"), has_repo)
                                    .clicked()
                                {
                                    self.active_view = MainView::Stashes;
                                }
                                if let Some(notice) = &self.last_notice {
                                    ui.label(RichText::new(notice).color(theme::accent()));
                                }
                            });
                        });
                }
            });
        }

        if let Some(index) = close_repo_tab {
            self.close_repo_tab(index);
            switch_to = None;
            reorder_repo_tab = None;
        }
        if close_source_tab {
            self.source_tab_open = false;
            if self.active_repo_tab.is_none() && !self.repo_tabs.is_empty() {
                switch_to = Some(self.repo_tabs.len() - 1);
            }
            self.save_repo_tabs();
        }
        if activate_source_tab {
            self.active_repo_tab = None;
            self.refresh_known_repositories();
            self.save_repo_tabs();
        }
        if open_source_tab {
            self.open_repository_source_tab();
        }
        if let Some((from, to)) = reorder_repo_tab {
            self.active_repo_tab =
                reorder_repo_tabs(&mut self.repo_tabs, self.active_repo_tab, from, to);
            self.repo_tab_drag.dragging_index = Some(to);
            self.save_repo_tabs();
            switch_to = None;
        }
        if let Some(index) = switch_to {
            self.switch_repo_tab(index);
        }
    }

    fn repository_source_view(&mut self, ui: &mut Ui) {
        let mut load_path = None;
        let mut create_path = None;
        let source_busy = self.repo_source_task.is_some();

        ScrollArea::vertical()
            .id_salt("repository_source_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new(self.tr("repo.source.title"))
                            .size(28.0)
                            .strong()
                            .color(theme::text()),
                    );
                });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    if source_tab_button(
                        ui,
                        self.repo_source_tab == RepoSourceTab::Local,
                        UiIcon::Workspace,
                        self.tr("repo.source.local"),
                    )
                    .clicked()
                    {
                        self.repo_source_tab = RepoSourceTab::Local;
                    }
                    if source_tab_button(
                        ui,
                        self.repo_source_tab == RepoSourceTab::Remote,
                        UiIcon::Folder,
                        self.tr("repo.source.remote"),
                    )
                    .clicked()
                    {
                        self.repo_source_tab = RepoSourceTab::Remote;
                    }
                    ui.add_space(20.0);
                    if toolbar_button(ui, "clone", self.tr("repo.source.clone"), !source_busy)
                        .clicked()
                    {
                        self.repo_source_tab = RepoSourceTab::Remote;
                    }
                    if toolbar_button(ui, "add", self.tr("repo.source.add"), !source_busy).clicked()
                    {
                        load_path = rfd::FileDialog::new().pick_folder();
                    }
                    if toolbar_button(ui, "create", self.tr("repo.source.create"), !source_busy)
                        .clicked()
                    {
                        create_path = rfd::FileDialog::new().pick_folder();
                    }
                    if source_busy {
                        ui.spinner();
                    }
                });

                ui.add_space(18.0);
                match self.repo_source_tab {
                    RepoSourceTab::Local => self.local_repository_source_page(ui, &mut load_path),
                    RepoSourceTab::Remote => self.remote_repository_source_page(ui),
                }
            });

        if let Some(path) = load_path {
            self.load_repository(path);
        }
        if let Some(path) = create_path {
            self.create_repo_path = path.display().to_string();
            self.create_from_source_tab();
        }
    }

    fn local_repository_source_page(&mut self, ui: &mut Ui, load_path: &mut Option<PathBuf>) {
        let search_hint = self.tr("repo.source.search");
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            themed_text_edit_selection(ui);
            ui.add_sized(
                [ui.available_width().min(800.0), 30.0],
                themed_singleline_text_edit(&mut self.repo_source_search, search_hint),
            );
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.label(
                RichText::new(self.tr("repo.source.local_repositories"))
                    .strong()
                    .color(theme::text()),
            );
        });
        ui.add_space(6.0);

        let repositories = self.filtered_known_repositories();
        if repositories.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(RichText::new(self.tr("repo.source.empty")).color(theme::muted()));
            });
            return;
        }

        for repository in repositories {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                if known_repository_row(ui, &repository).clicked() {
                    *load_path = Some(repository.root.clone());
                }
            });
        }
    }

    fn remote_repository_source_page(&mut self, ui: &mut Ui) {
        let url_label = self.tr("repo.source.clone_url");
        let destination_label = self.tr("repo.source.destination");
        let clone_label = self.tr("repo.source.clone");
        let browse_label = self.tr("repo.source.browse");
        let busy = self.repo_source_task.is_some();
        let url_valid = self.clone_url_is_valid();

        ui.add_space(2.0);
        form_row(ui, url_label, |ui| {
            let response = ui.add_sized(
                [ui.available_width().min(680.0), 28.0],
                TextEdit::singleline(&mut self.clone_url),
            );
            if response.changed() {
                self.on_clone_url_changed();
            }
            match &self.clone_url_status {
                CloneUrlStatus::Checking => {
                    ui.spinner();
                    ui.label(RichText::new(self.tr("repo.source.checking")).color(theme::muted()));
                }
                CloneUrlStatus::Valid => {
                    ui.label(RichText::new(self.tr("repo.source.valid")).color(theme::accent()));
                }
                CloneUrlStatus::Invalid(message) => {
                    ui.label(RichText::new(self.tr("repo.source.invalid")).color(theme::warning()))
                        .on_hover_text(message);
                }
                CloneUrlStatus::Pending => {
                    ui.label(RichText::new(self.tr("repo.source.pending")).color(theme::muted()));
                }
                CloneUrlStatus::Empty => {}
            }
        });
        form_row(ui, destination_label, |ui| {
            ui.horizontal(|ui| {
                ui.add_enabled_ui(url_valid, |ui| {
                    ui.add_sized(
                        [ui.available_width().min(560.0), 28.0],
                        TextEdit::singleline(&mut self.clone_destination),
                    );
                });
                if ui
                    .add_enabled(url_valid, egui::Button::new(browse_label))
                    .clicked()
                {
                    if let Some(parent) = rfd::FileDialog::new().pick_folder() {
                        let repo_name = repo_name_from_url(&self.clone_url)
                            .unwrap_or_else(|| "repository".to_owned());
                        self.clone_destination = parent.join(repo_name).display().to_string();
                    }
                }
            });
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(168.0);
            if ui
                .add_enabled(
                    !busy && url_valid && !self.clone_destination.trim().is_empty(),
                    egui::Button::new(clone_label),
                )
                .clicked()
            {
                self.clone_from_source_tab();
            }
        });
    }

    fn filtered_known_repositories(&self) -> Vec<KnownRepository> {
        let query = self.repo_source_search.trim().to_lowercase();
        self.known_repositories
            .iter()
            .filter(|repository| {
                query.is_empty()
                    || repository.name.to_lowercase().contains(&query)
                    || repository
                        .root
                        .display()
                        .to_string()
                        .to_lowercase()
                        .contains(&query)
            })
            .cloned()
            .collect()
    }

    fn refresh_known_repositories(&mut self) {
        let mut repositories = Vec::new();
        for tab in &self.repo_tabs {
            add_known_repository(&mut repositories, tab.root.clone());
        }

        if let Ok(current_dir) = env::current_dir() {
            add_known_repository(&mut repositories, current_dir.clone());
            if let Some(parent) = current_dir.parent() {
                scan_repository_children(parent, &mut repositories);
            }
        }

        repositories.sort_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.root.cmp(&right.root))
        });
        self.known_repositories = repositories;
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        ScrollArea::vertical()
            .id_salt("sidebar_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| self.sidebar_content(ui));
    }

    fn sidebar_content(&mut self, ui: &mut Ui) {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new(self.tr("repo.title"))
                    .strong()
                    .color(theme::text()),
            );
        });
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            if let Some(snapshot) = &self.snapshot {
                ui.add(
                    egui::Label::new(
                        RichText::new(snapshot.root.display().to_string())
                            .small()
                            .color(theme::muted()),
                    )
                    .truncate(),
                );
            } else {
                ui.label(RichText::new(self.tr("repo.none")).color(theme::muted()));
            }
        });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.spacing_mut().item_spacing.x = 6.0;
            let card_width = sidebar_nav_card_width(ui.available_width() - 10.0, 3);
            if sidebar_nav_card(
                ui,
                card_width,
                self.active_view == MainView::Workspace,
                UiIcon::Workspace,
                self.tr("worktree.title"),
            )
            .clicked()
            {
                self.active_view = MainView::Workspace;
            }
            if sidebar_nav_card(
                ui,
                card_width,
                self.active_view == MainView::History,
                UiIcon::History,
                self.tr("nav.history"),
            )
            .clicked()
            {
                self.active_view = MainView::History;
            }
            if sidebar_nav_card(
                ui,
                card_width,
                self.active_view == MainView::Search,
                UiIcon::Search,
                self.tr("commit.search"),
            )
            .clicked()
            {
                self.active_view = MainView::Search;
            }
        });

        ui.add_space(16.0);

        let branch_actions_enabled = !self.branch_actions_busy();
        let pending_branch_checkout = self.pending_branch_checkout.clone();
        if let Some(snapshot) = &self.snapshot {
            ui.add_space(8.0);

            let remote_branch_names = snapshot
                .branches
                .iter()
                .filter(|branch| branch.remote)
                .map(|branch| branch.name.clone())
                .collect::<Vec<_>>();
            let remote_names = snapshot
                .remotes
                .iter()
                .map(|remote| remote.name.clone())
                .collect::<Vec<_>>();
            let mut branch_action = None;
            let mut tag_action = None;
            let mut stash_action = None;
            let mut sidebar_state_changed = false;

            let branch_create_label = self.tr("branch.create");
            let branches_open_before = self.branches_open;
            let (branches_visible, create_branch_clicked) = tree_header_with_action_enabled(
                ui,
                &mut self.branches_open,
                UiIcon::Branch,
                i18n::t(self.language, "branch.local"),
                UiIcon::Plus,
                branch_create_label,
                branch_actions_enabled,
            );
            if create_branch_clicked && branch_actions_enabled {
                branch_action = Some(BranchMenuAction::Create);
            }
            if self.branches_open != branches_open_before {
                sidebar_state_changed = true;
            }
            if branches_visible {
                let local_branches = snapshot
                    .branches
                    .iter()
                    .filter(|branch| !branch.remote)
                    .collect::<Vec<_>>();
                let local_branches_by_name = local_branches
                    .iter()
                    .map(|branch| (branch.name.as_str(), *branch))
                    .collect::<HashMap<_, _>>();
                for node in local_branch_tree(&local_branches).iter().take(18) {
                    local_branch_tree_rows(
                        ui,
                        node,
                        0,
                        &local_branches_by_name,
                        pending_branch_checkout.as_deref(),
                        &remote_branch_names,
                        &remote_names,
                        self.language,
                        &mut self.local_branch_collapsed_groups,
                        branch_actions_enabled,
                        &mut branch_action,
                    );
                }
            }

            let tag_create_label = self.tr("tag.create");
            let tags_open_before = self.tags_open;
            let (tags_visible, create_tag_clicked) = tree_header_with_action(
                ui,
                &mut self.tags_open,
                UiIcon::Tag,
                i18n::t(self.language, "tag.title"),
                UiIcon::Plus,
                tag_create_label,
            );
            if create_tag_clicked {
                tag_action = Some(TagMenuAction::Create);
            }
            if self.tags_open != tags_open_before {
                sidebar_state_changed = true;
            }
            if tags_visible {
                if snapshot.tags.is_empty() {
                    tree_empty(ui, self.tr("tag.none"));
                } else {
                    for tag in snapshot.tags.iter().take(12) {
                        tag_row(ui, tag, self.language, &mut tag_action);
                    }
                }
            }

            let remotes_open_before = self.remotes_open;
            let remotes_visible = tree_header(
                ui,
                &mut self.remotes_open,
                UiIcon::Folder,
                i18n::t(self.language, "remote.title"),
            );
            if self.remotes_open != remotes_open_before {
                sidebar_state_changed = true;
            }
            if remotes_visible {
                let remote_branches = snapshot
                    .branches
                    .iter()
                    .filter(|branch| branch.remote)
                    .collect::<Vec<_>>();
                if remote_branches.is_empty() {
                    tree_empty(ui, remote_empty_label(self.language, snapshot));
                } else {
                    for node in remote_branch_tree(&remote_branches).iter().take(8) {
                        remote_branch_tree_rows(
                            ui,
                            node,
                            0,
                            self.language,
                            &mut self.remote_branch_collapsed_groups,
                            branch_actions_enabled,
                            &mut branch_action,
                        );
                    }
                }
            }

            let stashes_open_before = self.stashes_open;
            let stashes_visible = tree_header(
                ui,
                &mut self.stashes_open,
                UiIcon::Stash,
                i18n::t(self.language, "stash.title"),
            );
            if self.stashes_open != stashes_open_before {
                sidebar_state_changed = true;
            }
            if stashes_visible {
                ui.horizontal(|ui| {
                    ui.add_space(26.0);
                    if ui
                        .add_enabled(
                            !snapshot.status.is_empty(),
                            egui::Button::new(self.tr("stash.create")),
                        )
                        .clicked()
                    {
                        stash_action = Some(StashMenuAction::Create);
                    }
                });
                if snapshot.stashes.is_empty() {
                    tree_empty(ui, self.tr("stash.none"));
                } else {
                    for stash in snapshot.stashes.iter().take(8) {
                        stash_row(ui, stash, self.language, &mut stash_action);
                    }
                }
            }

            if let Some(action) = stash_action {
                self.handle_stash_action(action);
            }
            if branch_actions_enabled {
                if let Some(action) = branch_action {
                    self.handle_branch_action(action);
                }
            }
            if let Some(action) = tag_action {
                self.handle_tag_action(action);
            }
            if sidebar_state_changed {
                self.save_sidebar_tree_state_for_active_repo();
            }
        }
    }

    fn workspace_view(&mut self, ui: &mut Ui) {
        ui.add_space(WORKSPACE_HEADER_TOP_GAP);
        let Some(snapshot) = &self.snapshot else {
            empty_state(ui, self.loading_repo, self.language);
            return;
        };

        let staged = snapshot.staged.clone();
        let unstaged = snapshot.unstaged.clone();
        let conflict_files = worktree_conflict_files(snapshot);
        let status_count = snapshot.status.len();
        let mut worktree_action = None;
        let mut selected_worktree_after_draw = None;
        let worktree_selection = self.worktree_selection.clone();

        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new(self.tr("worktree.title"))
                    .size(WORKSPACE_HEADER_TITLE_SIZE)
                    .color(theme::text()),
            );
            ui.label(
                RichText::new(format!("{status_count}"))
                    .small()
                    .color(theme::muted()),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(12.0);
                let display_toggle_label = match self.worktree_display_mode {
                    WorktreeDisplayMode::Flat => self.tr("worktree.view_tree"),
                    WorktreeDisplayMode::Tree => self.tr("worktree.view_flat"),
                };
                if worktree_header_action_button(ui, None, display_toggle_label, true).clicked() {
                    self.worktree_display_mode = self.worktree_display_mode.toggle();
                }
                if let Some(conflict_file) = selected_or_first_conflict(
                    &conflict_files,
                    self.selected_worktree_file.as_ref(),
                ) {
                    if worktree_header_action_button(
                        ui,
                        Some(UiIcon::Warning),
                        self.tr("worktree.resolve_conflicts"),
                        true,
                    )
                    .clicked()
                    {
                        worktree_action = Some(WorktreeMenuAction::ResolveConflict {
                            path: conflict_file.path.clone(),
                        });
                    }
                }
                if worktree_header_action_button(
                    ui,
                    None,
                    self.tr("worktree.stage_all"),
                    !unstaged.is_empty(),
                )
                .clicked()
                {
                    worktree_action = Some(WorktreeMenuAction::StageAll);
                }
                if worktree_header_action_button(
                    ui,
                    None,
                    self.tr("worktree.unstage_all"),
                    !staged.is_empty(),
                )
                .clicked()
                {
                    worktree_action = Some(WorktreeMenuAction::UnstageAll);
                }
            });
        });
        ui.add_space(WORKSPACE_HEADER_BOTTOM_GAP);

        if status_count == 0 {
            clean_worktree_state(
                ui,
                self.tr("worktree.clean"),
                self.tr("worktree.clean_detail"),
            );
            return;
        }

        let show_diff_panel = self.selected_worktree_file.is_some() && ui.available_width() > 720.0;
        if show_diff_panel {
            let available_size = ui.available_size();
            let gap = LAYOUT_GAP as f32;
            let max_diff_width = (available_size.x - 360.0).max(280.0);
            let diff_width = (available_size.x * self.layout_prefs.workspace_diff_pct)
                .clamp(280.0, max_diff_width);
            let left_width = (available_size.x - diff_width - gap).max(320.0);
            let workspace_content_rect = ui.allocate_exact_size(available_size, Sense::hover()).0;
            let left_rect = Rect::from_min_size(
                workspace_content_rect.left_top(),
                Vec2::new(left_width, available_size.y),
            );
            let workspace_splitter_rect = Rect::from_min_size(
                Pos2::new(left_rect.right(), workspace_content_rect.top()),
                Vec2::new(gap, available_size.y),
            );
            let right_rect = Rect::from_min_max(
                Pos2::new(
                    workspace_splitter_rect.right(),
                    workspace_content_rect.top(),
                ),
                workspace_content_rect.right_bottom(),
            );

            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                ui.set_clip_rect(left_rect);
                self.workspace_main_panel(
                    ui,
                    &staged,
                    &unstaged,
                    &worktree_selection,
                    &mut worktree_action,
                    &mut selected_worktree_after_draw,
                );
            });
            if let Some(delta) =
                vertical_resize_delta(ui, workspace_splitter_rect, "workspace_diff_resize")
            {
                self.layout_prefs.workspace_diff_pct =
                    ((diff_width - delta) / available_size.x).clamp(0.28, 0.52);
                self.layout_prefs.save();
            }
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(right_rect), |ui| {
                ui.set_clip_rect(right_rect);
                safe_set_min_size(ui, right_rect.size());
                self.worktree_diff_viewer(ui);
            });
        } else {
            self.workspace_main_panel(
                ui,
                &staged,
                &unstaged,
                &worktree_selection,
                &mut worktree_action,
                &mut selected_worktree_after_draw,
            );
        }

        if let Some(action) = worktree_action {
            self.handle_worktree_action(action);
        }
        if let Some(selected) = selected_worktree_after_draw {
            let files = if selected.file.staged {
                &staged
            } else {
                &unstaged
            };
            self.worktree_selection.apply(
                files,
                &selected.file.path,
                selected.file.staged,
                selected.modifiers,
            );
            self.selected_worktree_file = Some(selected.file);
            self.selected_file_path = None;
            self.selected_diff_rows.clear();
            self.request_selected_worktree_diff();
        }
    }

    fn workspace_main_panel(
        &mut self,
        ui: &mut Ui,
        staged: &[WorktreeFile],
        unstaged: &[WorktreeFile],
        worktree_selection: &WorktreeSelectionState,
        worktree_action: &mut Option<WorktreeMenuAction>,
        selected_worktree_after_draw: &mut Option<WorktreeRowClick>,
    ) {
        let body_size = ui.available_size();
        let (body_rect, _) = ui.allocate_exact_size(body_size, Sense::hover());
        let layout = workspace_main_layout(
            body_rect,
            self.layout_prefs.workspace_list_pct,
            self.layout_prefs.workspace_staged_pct,
        );

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(layout.staged_rect), |ui| {
            worktree_table(
                ui,
                self.tr("worktree.staged"),
                staged,
                true,
                layout.staged_rect.height(),
                self.language,
                worktree_selection,
                self.worktree_display_mode,
                &mut self.worktree_collapsed_dirs,
                worktree_action,
                selected_worktree_after_draw,
            );
        });
        if let Some(delta) = horizontal_resize_delta(
            ui,
            layout.staged_unstaged_splitter_rect,
            "workspace_staged_unstaged_resize",
        ) {
            let table_total =
                (layout.staged_rect.height() + layout.unstaged_rect.height()).max(160.0);
            self.layout_prefs.workspace_staged_pct =
                ((layout.staged_rect.height() + delta) / table_total).clamp(0.24, 0.76);
            self.layout_prefs.save();
        }
        ui.allocate_new_ui(
            egui::UiBuilder::new().max_rect(layout.unstaged_rect),
            |ui| {
                worktree_table(
                    ui,
                    self.tr("worktree.unstaged"),
                    unstaged,
                    false,
                    layout.unstaged_rect.height(),
                    self.language,
                    worktree_selection,
                    self.worktree_display_mode,
                    &mut self.worktree_collapsed_dirs,
                    worktree_action,
                    selected_worktree_after_draw,
                );
            },
        );

        if let Some(delta) = horizontal_resize_delta(
            ui,
            layout.list_commit_splitter_rect,
            "workspace_list_commit_resize",
        ) {
            self.layout_prefs.workspace_list_pct =
                ((layout.list_commit_splitter_rect.top() - body_rect.top() + delta)
                    / body_rect.height())
                .clamp(0.42, 0.74);
            self.layout_prefs.save();
        }
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(layout.commit_rect), |ui| {
            self.commit_panel(ui, staged.len(), layout.commit_rect.height());
        });
        paint_layout_debug_rect(
            ui,
            layout.staged_rect,
            "workspace.staged",
            Color32::LIGHT_BLUE,
        );
        paint_layout_debug_rect(
            ui,
            layout.unstaged_rect,
            "workspace.unstaged",
            Color32::LIGHT_GREEN,
        );
        paint_layout_debug_rect(ui, layout.commit_rect, "workspace.commit", Color32::YELLOW);
        log_layout_debug_once(
            &WORKSPACE_LAYOUT_DEBUG_LOGGED,
            "workspace",
            &[
                ("body", body_rect),
                ("staged", layout.staged_rect),
                ("unstaged", layout.unstaged_rect),
                ("commit", layout.commit_rect),
            ],
        );
    }

    fn search_view(&mut self, ui: &mut Ui) {
        if self.snapshot.is_none() {
            empty_state(ui, self.loading_repo, self.language);
            return;
        }

        let filtered_indices = self.search_filtered_commit_indices();
        let commit_count = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.commits.len())
            .unwrap_or_default();
        let rows = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                filtered_indices
                    .iter()
                    .filter_map(|index| {
                        snapshot
                            .commits
                            .get(*index)
                            .cloned()
                            .map(|commit| (*index, commit))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut should_request_details = false;
        let available = ui.available_size();
        let (results_height, _details_height) = master_detail_split_heights(available.y);

        ui.allocate_ui(Vec2::new(available.x, results_height), |ui| {
            egui::Frame::new().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(self.tr("commit.search"))
                            .size(22.0)
                            .strong()
                            .color(theme::text()),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!(
                            "{} {}",
                            commit_count,
                            self.tr("commit.stats_loaded")
                        ))
                        .color(theme::muted()),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!(
                            "{} {}",
                            rows.len(),
                            self.tr("commit.stats_visible")
                        ))
                        .color(theme::muted()),
                    );
                });

                ui.add_space(10.0);
                let (filter_rect, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::hover());
                let dropdown_width = 120.0;
                let gap = 10.0;
                let control_height = 28.0;
                let control_top = filter_rect.top() + 2.0;
                let file_search_busy = self.file_search_task.is_some();
                let is_file_search = self.search_dimension == SearchDimension::Files;
                let dropdown_rect = Rect::from_min_size(
                    Pos2::new(filter_rect.right() - dropdown_width, control_top),
                    Vec2::new(dropdown_width, control_height),
                );
                let search_rect = Rect::from_min_size(
                    Pos2::new(filter_rect.left(), control_top),
                    Vec2::new(
                        (filter_rect.width() - dropdown_width - gap).max(180.0),
                        control_height,
                    ),
                );
                let submit_rect = Rect::from_min_size(
                    Pos2::new(search_rect.right() - control_height, search_rect.top()),
                    Vec2::splat(control_height),
                );
                let text_rect = if is_file_search {
                    Rect::from_min_max(
                        search_rect.min,
                        Pos2::new(submit_rect.left() - 4.0, search_rect.bottom()),
                    )
                } else {
                    search_rect
                };
                let search_hint = self.tr("commit.search");
                let response = ui
                    .add_enabled_ui(!file_search_busy, |ui| {
                        ui.put(
                            text_rect,
                            TextEdit::singleline(&mut self.search_view_query)
                                .hint_text(search_hint)
                                .desired_width(text_rect.width())
                                .vertical_align(Align::Center),
                        )
                    })
                    .inner;
                let enter_pressed = ui.input(|input| input.key_pressed(egui::Key::Enter));
                let search_submitted =
                    enter_pressed && (response.has_focus() || response.lost_focus());
                if let Some(dimension) = search_dimension_dropdown(
                    ui,
                    dropdown_rect,
                    self.language,
                    self.search_dimension,
                    !file_search_busy,
                ) {
                    self.search_dimension = dimension;
                    if self.search_dimension == SearchDimension::Files
                        && self.file_search_query != self.search_view_query.trim().to_lowercase()
                    {
                        self.file_search_hashes.clear();
                    }
                    self.search_selected_commit =
                        self.search_filtered_commit_indices().first().copied();
                    self.search_selected_file_path = None;
                    self.search_selected_diff_rows.clear();
                    should_request_details = !self.select_first_search_changed_file_if_cached();
                }
                if response.changed() {
                    if self.search_dimension == SearchDimension::Files {
                        self.file_search_query.clear();
                        self.file_search_hashes.clear();
                        self.file_search_task = None;
                        self.file_search_started_at = None;
                    } else {
                        self.search_selected_commit =
                            self.search_filtered_commit_indices().first().copied();
                        should_request_details = true;
                    }
                    self.search_selected_file_path = None;
                    self.search_selected_diff_rows.clear();
                    if self.search_dimension != SearchDimension::Files {
                        should_request_details = !self.select_first_search_changed_file_if_cached();
                    }
                }
                let search_button_clicked = is_file_search
                    && search_submit_button(
                        ui,
                        submit_rect,
                        file_search_busy,
                        !self.search_view_query.trim().is_empty(),
                    )
                    .clicked();
                if is_file_search && (search_submitted || search_button_clicked) {
                    self.start_file_change_search();
                }

                ui.add_space(10.0);
                if rows.is_empty() {
                    empty_list_panel(ui, self.tr("commit.no_matches"));
                    return;
                }

                search_table_header(ui, self.language);
                let mut clicked_commit = None;
                let mut hash_copied = false;
                ScrollArea::vertical()
                    .id_salt("search_results_scroll")
                    .auto_shrink([false, false])
                    .show_rows(ui, HISTORY_TABLE_ROW_HEIGHT, rows.len(), |ui, range| {
                        for row_index in range {
                            let (commit_index, commit) = &rows[row_index];
                            let (response, copied_hash) = search_commit_row(
                                ui,
                                commit,
                                self.search_selected_commit == Some(*commit_index),
                            );
                            hash_copied |= copied_hash;
                            if response.clicked() {
                                clicked_commit = Some(*commit_index);
                            }
                        }
                    });
                if let Some(index) = clicked_commit {
                    self.search_selected_commit = Some(index);
                    self.search_selected_file_path = None;
                    self.search_selected_diff_rows.clear();
                    should_request_details = !self.select_first_search_changed_file_if_cached();
                }
                if hash_copied {
                    self.show_toast(self.tr("status.hash_copied"));
                }
            });
        });
        ui.add_space(LAYOUT_GAP as f32);
        self.search_bottom_pane(ui);

        if should_request_details {
            self.request_selected_search_details();
        }
    }

    fn history_view(&mut self, ui: &mut Ui) {
        if self.snapshot.is_none() {
            empty_state(ui, self.loading_repo, self.language);
            return;
        }

        let available = ui.available_size();
        let min_top_height = history_top_min_height();
        let max_top_height = if available.y > HISTORY_BOTTOM_MIN_HEIGHT + LAYOUT_GAP as f32 {
            available.y - HISTORY_BOTTOM_MIN_HEIGHT - LAYOUT_GAP as f32
        } else {
            (available.y * 0.58).max(min_top_height)
        };
        let top_height = if self.layout_prefs.history_top_pct > 0.0 {
            (available.y * self.layout_prefs.history_top_pct).clamp(min_top_height, max_top_height)
        } else {
            (available.y * 0.54).clamp(min_top_height, max_top_height)
        };

        ui.allocate_ui(Vec2::new(available.x, top_height), |ui| {
            ui.set_min_height(top_height);
            ui.set_max_height(top_height);
            ui.set_clip_rect(ui.max_rect());
            self.history_commit_table(ui);
        });
        if let Some(delta) = history_table_splitter(ui, available.x) {
            let next_height = (top_height + delta).clamp(min_top_height, max_top_height);
            self.layout_prefs.history_top_pct = next_height / available.y.max(1.0);
            self.layout_prefs.clamp();
            self.layout_prefs.save();
            ui.ctx().request_repaint();
        }
        self.history_bottom_pane(ui);
    }

    fn history_commit_table(&mut self, ui: &mut Ui) {
        let visible_row_count = self.refresh_history_rows_cache();
        let commit_count = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.commits.len())
            .unwrap_or_default();
        let graph_width = history_graph_width(
            ui.available_width(),
            self.layout.lanes.max(1),
            &self.layout_prefs,
        );
        let lane_count = self.layout.lanes.max(1);
        let mut should_request_details = false;

        let (toolbar_rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
        let control_top = toolbar_rect.top() + 2.0;
        let control_height = 24.0;
        let mut control_left = toolbar_rect.left();
        let branch_rect = Rect::from_min_size(
            Pos2::new(control_left, control_top),
            Vec2::new(102.0, control_height),
        );
        if let Some(scope) =
            history_branch_scope_dropdown(ui, branch_rect, self.language, self.history_branch_scope)
        {
            self.set_history_branch_scope(scope);
        }
        control_left = branch_rect.right() + 8.0;

        let remote_label = if self.language == Language::Chinese {
            "\u{663e}\u{793a}\u{8fdc}\u{7aef}\u{5206}\u{652f}"
        } else {
            "Show remote branches"
        };
        let remote_rect = Rect::from_min_size(
            Pos2::new(control_left, control_top),
            Vec2::new(134.0, control_height),
        );
        if history_toolbar_checkbox_at(
            ui,
            remote_rect,
            &mut self.history_show_remote_refs,
            remote_label,
        )
        .changed()
        {
            ui.ctx().request_repaint();
        }
        control_left = remote_rect.right() + 8.0;

        let sort_rect = Rect::from_min_size(
            Pos2::new(control_left, control_top),
            Vec2::new(112.0, control_height),
        );
        if let Some(order) =
            history_sort_order_dropdown(ui, sort_rect, self.language, self.history_sort_order)
        {
            self.set_history_sort_order(order);
        }
        control_left = sort_rect.right() + 8.0;

        if self.history_cherry_pick_mode {
            let confirm_rect = Rect::from_min_size(
                Pos2::new(control_left, control_top),
                Vec2::new(80.0, control_height),
            );
            let selected_count = self.selected_cherry_pick_hashes.len();
            if history_toolbar_action_button_at(
                ui,
                confirm_rect,
                self.tr("commit.cherry_pick_confirm"),
                selected_count > 0,
            )
            .clicked()
            {
                self.confirm_selected_cherry_picks();
            }
            control_left = confirm_rect.right() + 6.0;

            let cancel_rect = Rect::from_min_size(
                Pos2::new(control_left, control_top),
                Vec2::new(70.0, control_height),
            );
            if history_toolbar_action_button_at(ui, cancel_rect, self.tr("dialog.cancel"), true)
                .clicked()
            {
                self.clear_cherry_pick_selection();
            }
            control_left = cancel_rect.right() + 8.0;

            let selected_label = format!(
                "{} {}",
                selected_count,
                self.tr("commit.cherry_pick_selected")
            );
            let label_rect = Rect::from_min_size(
                Pos2::new(control_left, control_top),
                Vec2::new(116.0, control_height),
            );
            history_toolbar_label_at(ui, label_rect, &selected_label);
            control_left = label_rect.right() + 8.0;
        } else {
            let cherry_pick_label = self.tr("commit.cherry_pick_batch");
            let cherry_rect = Rect::from_min_size(
                Pos2::new(control_left, control_top),
                Vec2::new(30.0, control_height),
            );
            if history_toolbar_icon_button_at(
                ui,
                cherry_rect,
                UiIcon::CherryPick,
                cherry_pick_label,
            )
            .clicked()
            {
                self.history_cherry_pick_mode = true;
            }
            control_left = cherry_rect.right() + 8.0;
        }

        let search_hint = if self.language == Language::Chinese {
            "\u{641c}\u{7d22}\u{63d0}\u{4ea4}"
        } else {
            "Search commits"
        };
        let search_width = 260.0_f32.min((toolbar_rect.right() - control_left - 8.0).max(120.0));
        let search_rect = Rect::from_min_size(
            Pos2::new(toolbar_rect.right() - search_width, control_top),
            Vec2::new(search_width, control_height),
        );
        let response = ui.put(
            search_rect,
            TextEdit::singleline(&mut self.search).hint_text(search_hint),
        );
        if response.changed() {
            self.selected_commit = self.history_rows_cache.first_index();
            should_request_details = true;
        }

        ui.add_space(4.0);
        if history_table_header(
            ui,
            self.language,
            graph_width,
            &mut self.layout_prefs,
            self.history_cherry_pick_mode,
        ) {
            self.layout_prefs.save();
        }

        if commit_count == 0 {
            no_commits_state(ui, self.language);
            return;
        }

        if visible_row_count == 0 {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new(self.tr("commit.no_matches")).color(theme::muted()));
            });
            return;
        }

        let body_height = ui.available_height().max(0.0);
        let (body_rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), body_height), Sense::hover());
        let mut clicked_commit = None;
        let mut menu_action = None;
        let mut hash_copied = false;
        let mut toggle_cherry_pick_hash = None;
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(body_rect), |ui| {
            safe_set_min_size(ui, body_rect.size());
            ui.set_max_height(body_rect.height());
            ui.set_clip_rect(body_rect);
            ScrollArea::vertical()
                .id_salt((
                    "history_commit_graph_scroll",
                    self.history_branch_scope,
                    self.history_sort_order,
                    self.search.as_str(),
                    visible_row_count,
                ))
                .auto_shrink([false, false])
                .show_viewport(ui, |ui, viewport| {
                    let row_range = history_virtual_row_range(
                        viewport,
                        HISTORY_TABLE_ROW_HEIGHT,
                        visible_row_count,
                    );
                    ui.set_min_height(visible_row_count as f32 * HISTORY_TABLE_ROW_HEIGHT);
                    if row_range.start > 0 {
                        ui.add_space(row_range.start as f32 * HISTORY_TABLE_ROW_HEIGHT);
                        ui.skip_ahead_auto_ids(row_range.start);
                    }
                    if let Some(snapshot) = &self.snapshot {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for row_index in row_range.clone() {
                            let Some(commit_index) = self.history_rows_cache.index_at(row_index)
                            else {
                                continue;
                            };
                            let Some(commit) = snapshot.commits.get(commit_index) else {
                                continue;
                            };
                            let row = self.layout.rows.get(commit_index);
                            let is_selected = self.selected_commit == Some(commit_index);
                            let cherry_pick_selected =
                                self.selected_cherry_pick_hashes.contains(&commit.hash);
                            let (response, copied_hash, select_for_cherry_pick) =
                                history_commit_table_row(
                                    ui,
                                    commit,
                                    row,
                                    graph_width,
                                    lane_count,
                                    &self.layout_prefs,
                                    self.language,
                                    is_selected,
                                    self.history_show_remote_refs,
                                    self.history_cherry_pick_mode,
                                    cherry_pick_selected,
                                );
                            hash_copied |= copied_hash;
                            if select_for_cherry_pick {
                                toggle_cherry_pick_hash = Some(commit.hash.clone());
                            } else if response.clicked() {
                                clicked_commit = Some(commit_index);
                            }
                            response.context_menu(|ui| {
                                menu_action = commit_context_menu(ui, commit, self.language);
                            });
                        }
                    }
                    let remaining_rows = visible_row_count.saturating_sub(row_range.end);
                    if remaining_rows > 0 {
                        ui.add_space(remaining_rows as f32 * HISTORY_TABLE_ROW_HEIGHT);
                    }
                });
        });

        if let Some(hash) = toggle_cherry_pick_hash {
            self.toggle_cherry_pick_hash(hash);
        }

        if let Some(commit_index) = clicked_commit {
            self.selected_commit = Some(commit_index);
            self.selected_file_path = None;
            self.selected_diff_rows.clear();
            if !self.select_first_history_changed_file_if_cached() {
                self.request_selected_details();
            }
        }

        if let Some(action) = menu_action {
            self.handle_commit_menu_action(action);
        }

        if hash_copied {
            self.show_toast(self.tr("status.hash_copied"));
        }

        if should_request_details {
            self.request_selected_details();
        }
    }

    fn history_bottom_pane(&mut self, ui: &mut Ui) {
        let available = ui.available_size();
        let left_width = (available.x * 0.43).clamp(300.0, 560.0);
        let gap = LAYOUT_GAP as f32;
        let right_width = (available.x - left_width - gap).max(260.0);

        let (rect, _) = ui.allocate_exact_size(available, Sense::hover());
        let left_rect = Rect::from_min_size(rect.left_top(), Vec2::new(left_width, rect.height()));
        let right_rect = Rect::from_min_size(
            Pos2::new(left_rect.right() + gap, rect.top()),
            Vec2::new(right_width, rect.height()),
        );

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(left_rect), |ui| {
            source_tree_panel_frame().show(ui, |ui| {
                safe_set_min_size(ui, frame_inner_size(left_width, available.y, 8, 8));
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    self.history_commit_summary(ui);
                });
            });
        });
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(right_rect), |ui| {
            source_tree_panel_frame().show(ui, |ui| {
                safe_set_min_size(ui, frame_inner_size(right_width, available.y, 8, 8));
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    self.history_diff_pane(ui);
                });
            });
        });
    }

    fn search_bottom_pane(&mut self, ui: &mut Ui) {
        let available = ui.available_size();
        let left_width = (available.x * 0.43).clamp(300.0, 560.0);
        let gap = LAYOUT_GAP as f32;
        let right_width = (available.x - left_width - gap).max(260.0);

        let (rect, _) = ui.allocate_exact_size(available, Sense::hover());
        let left_rect = Rect::from_min_size(rect.left_top(), Vec2::new(left_width, rect.height()));
        let right_rect = Rect::from_min_size(
            Pos2::new(left_rect.right() + gap, rect.top()),
            Vec2::new(right_width, rect.height()),
        );

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(left_rect), |ui| {
            source_tree_panel_frame().show(ui, |ui| {
                safe_set_min_size(ui, frame_inner_size(left_width, available.y, 8, 8));
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    self.search_commit_summary(ui);
                });
            });
        });
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(right_rect), |ui| {
            source_tree_panel_frame().show(ui, |ui| {
                safe_set_min_size(ui, frame_inner_size(right_width, available.y, 8, 8));
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    self.search_diff_pane(ui);
                });
            });
        });
    }

    fn selected_commit_for_history(&self) -> Option<Commit> {
        let snapshot = self.snapshot.as_ref()?;
        self.selected_commit
            .and_then(|index| snapshot.commits.get(index))
            .or_else(|| snapshot.commits.first())
            .cloned()
    }

    fn selected_commit_for_search(&self) -> Option<Commit> {
        let snapshot = self.snapshot.as_ref()?;
        self.search_selected_commit
            .and_then(|index| snapshot.commits.get(index))
            .cloned()
    }

    fn history_commit_summary(&mut self, ui: &mut Ui) {
        let Some(commit) = self.selected_commit_for_history() else {
            ui.label(RichText::new(self.tr("commit.none")).color(theme::muted()));
            return;
        };

        let details_height = ui.available_height().clamp(96.0, 132.0);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), details_height),
            Layout::top_down(Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                source_tree_meta_line(
                    ui,
                    if self.language == Language::Chinese {
                        "\u{63d0}\u{4ea4}"
                    } else {
                        "Commit"
                    },
                    &format!("{} [{}]", commit.hash, commit.short_hash),
                );
                if !commit.parents.is_empty() {
                    let parents = commit
                        .parents
                        .iter()
                        .map(|parent| parent.chars().take(8).collect::<String>())
                        .collect::<Vec<_>>()
                        .join(", ");
                    source_tree_meta_line(
                        ui,
                        if self.language == Language::Chinese {
                            "\u{7236}\u{63d0}\u{4ea4}"
                        } else {
                            "Parent"
                        },
                        &parents,
                    );
                }
                source_tree_meta_line(ui, self.tr("commit.author"), &commit.author);
                let when = if commit.date.is_empty() {
                    commit.relative_time.as_str()
                } else {
                    commit.date.as_str()
                };
                source_tree_meta_line(ui, self.tr("commit.when"), when);
                source_tree_meta_line(
                    ui,
                    if self.language == Language::Chinese {
                        "\u{63d0}\u{4ea4}\u{8005}"
                    } else {
                        "Committer"
                    },
                    &commit.author,
                );
                ui.add_space(6.0);
                ui.add(
                    egui::Label::new(RichText::new(&commit.subject).color(theme::text())).wrap(),
                );
            },
        );

        ui.separator();
        self.history_file_table(ui, &commit);
    }

    fn search_commit_summary(&mut self, ui: &mut Ui) {
        let Some(commit) = self.selected_commit_for_search() else {
            ui.label(RichText::new(self.tr("commit.none")).color(theme::muted()));
            return;
        };

        let details_height = ui.available_height().clamp(96.0, 132.0);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), details_height),
            Layout::top_down(Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                source_tree_meta_line(
                    ui,
                    if self.language == Language::Chinese {
                        "\u{63d0}\u{4ea4}"
                    } else {
                        "Commit"
                    },
                    &format!("{} [{}]", commit.hash, commit.short_hash),
                );
                if !commit.parents.is_empty() {
                    let parents = commit
                        .parents
                        .iter()
                        .map(|parent| parent.chars().take(8).collect::<String>())
                        .collect::<Vec<_>>()
                        .join(", ");
                    source_tree_meta_line(
                        ui,
                        if self.language == Language::Chinese {
                            "\u{7236}\u{63d0}\u{4ea4}"
                        } else {
                            "Parent"
                        },
                        &parents,
                    );
                }
                source_tree_meta_line(ui, self.tr("commit.author"), &commit.author);
                let when = if commit.date.is_empty() {
                    commit.relative_time.as_str()
                } else {
                    commit.date.as_str()
                };
                source_tree_meta_line(ui, self.tr("commit.when"), when);
                source_tree_meta_line(
                    ui,
                    if self.language == Language::Chinese {
                        "\u{63d0}\u{4ea4}\u{8005}"
                    } else {
                        "Committer"
                    },
                    &commit.author,
                );
                ui.add_space(6.0);
                ui.add(
                    egui::Label::new(RichText::new(&commit.subject).color(theme::text())).wrap(),
                );
            },
        );

        ui.separator();
        self.search_file_table(ui, &commit);
    }

    fn history_file_table(&mut self, ui: &mut Ui, commit: &Commit) {
        let mut clicked_file = None;
        history_file_table_header(ui, self.language);

        if self.loading_details_hash.as_deref() == Some(commit.hash.as_str()) {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(self.tr("commit.loading_files")).color(theme::muted()));
            });
        } else if let Some(details) = self.details_cache.get(&commit.hash).cloned() {
            if details.files.is_empty() {
                ui.label(RichText::new(self.tr("commit.no_changes")).color(theme::muted()));
            } else {
                ScrollArea::vertical()
                    .id_salt("history_changed_files_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for file in &details.files {
                            let selected =
                                self.selected_file_path.as_deref() == Some(file.diff_path.as_str());
                            if history_file_table_row(ui, &file.status, &file.path, selected)
                                .clicked()
                            {
                                clicked_file = Some(file.diff_path.clone());
                            }
                        }
                    });
            }
        } else {
            ui.label(RichText::new(self.tr("commit.select_to_load_files")).color(theme::muted()));
        }

        if let Some(path) = clicked_file {
            self.select_changed_file_for_diff(path);
        }
    }

    fn search_file_table(&mut self, ui: &mut Ui, commit: &Commit) {
        let mut clicked_file = None;
        history_file_table_header(ui, self.language);

        if self.loading_details_hash.as_deref() == Some(commit.hash.as_str()) {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(self.tr("commit.loading_files")).color(theme::muted()));
            });
        } else if let Some(details) = self.details_cache.get(&commit.hash).cloned() {
            if details.files.is_empty() {
                ui.label(RichText::new(self.tr("commit.no_changes")).color(theme::muted()));
            } else {
                ScrollArea::vertical()
                    .id_salt("search_changed_files_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for file in &details.files {
                            let selected = self.search_selected_file_path.as_deref()
                                == Some(file.diff_path.as_str());
                            if history_file_table_row(ui, &file.status, &file.path, selected)
                                .clicked()
                            {
                                clicked_file = Some(file.diff_path.clone());
                            }
                        }
                    });
            }
        } else {
            ui.label(RichText::new(self.tr("commit.select_to_load_files")).color(theme::muted()));
        }

        if let Some(path) = clicked_file {
            self.select_search_changed_file_for_diff(path);
        }
    }

    fn history_diff_pane(&mut self, ui: &mut Ui) {
        let Some(commit) = self.selected_commit_for_history() else {
            ui.label(RichText::new(self.tr("commit.none")).color(theme::muted()));
            return;
        };

        let (header_rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
        ui.painter()
            .rect_filled(header_rect, CornerRadius::ZERO, theme::panel_soft());
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(header_rect), |ui| {
            let rollback_label = if self.language == Language::Chinese {
                "\u{56de}\u{6eda}\u{533a}\u{5757}"
            } else {
                "Hunks"
            };
            let blocks_label = self.tr("diff.blocks");
            let full_file_label = self.tr("diff.full_file");
            let switch_w = 122.0;
            let rollback_w = 78.0;
            let gap = 6.0;
            let top = header_rect.center().y - 10.0;
            let rollback_rect = Rect::from_min_size(
                Pos2::new(header_rect.right() - rollback_w - 6.0, top),
                Vec2::new(rollback_w, 20.0),
            );
            let switch_rect = Rect::from_min_size(
                Pos2::new(rollback_rect.left() - gap - switch_w, top),
                Vec2::new(switch_w, 20.0),
            );

            header_action_button_at(ui, rollback_rect, rollback_label);
            diff_display_mode_switch(
                ui,
                switch_rect,
                &mut self.history_diff_display_mode,
                blocks_label,
                full_file_label,
            );

            let icon_right = if self.selected_file_path.is_some() {
                draw_file_status_icon(
                    ui,
                    Rect::from_center_size(
                        Pos2::new(header_rect.left() + 15.0, header_rect.center().y),
                        Vec2::splat(15.0),
                    ),
                    "M",
                    false,
                );
                header_rect.left() + 24.0
            } else {
                header_rect.left() + 8.0
            };
            let path_rect = Rect::from_min_max(
                Pos2::new(icon_right, header_rect.top()),
                Pos2::new(switch_rect.left() - gap, header_rect.bottom()),
            );
            if let Some(path) = &self.selected_file_path {
                draw_elided_path_label(ui, path_rect, path);
            }
        });
        self.diff_viewer(ui, &commit.hash, self.history_diff_display_mode);
    }

    fn search_diff_pane(&mut self, ui: &mut Ui) {
        let Some(commit) = self.selected_commit_for_search() else {
            ui.label(RichText::new(self.tr("commit.none")).color(theme::muted()));
            return;
        };

        let (header_rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
        ui.painter()
            .rect_filled(header_rect, CornerRadius::ZERO, theme::panel_soft());
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(header_rect), |ui| {
            let rollback_label = if self.language == Language::Chinese {
                "\u{56de}\u{6eda}\u{533a}\u{5757}"
            } else {
                "Hunks"
            };
            let blocks_label = self.tr("diff.blocks");
            let full_file_label = self.tr("diff.full_file");
            let switch_w = 122.0;
            let rollback_w = 78.0;
            let gap = 6.0;
            let top = header_rect.center().y - 10.0;
            let rollback_rect = Rect::from_min_size(
                Pos2::new(header_rect.right() - rollback_w - 6.0, top),
                Vec2::new(rollback_w, 20.0),
            );
            let switch_rect = Rect::from_min_size(
                Pos2::new(rollback_rect.left() - gap - switch_w, top),
                Vec2::new(switch_w, 20.0),
            );

            header_action_button_at(ui, rollback_rect, rollback_label);
            diff_display_mode_switch(
                ui,
                switch_rect,
                &mut self.search_diff_display_mode,
                blocks_label,
                full_file_label,
            );

            let icon_right = if self.search_selected_file_path.is_some() {
                draw_file_status_icon(
                    ui,
                    Rect::from_center_size(
                        Pos2::new(header_rect.left() + 15.0, header_rect.center().y),
                        Vec2::splat(15.0),
                    ),
                    "M",
                    false,
                );
                header_rect.left() + 24.0
            } else {
                header_rect.left() + 8.0
            };
            let path_rect = Rect::from_min_max(
                Pos2::new(icon_right, header_rect.top()),
                Pos2::new(switch_rect.left() - gap, header_rect.bottom()),
            );
            if let Some(path) = &self.search_selected_file_path {
                draw_elided_path_label(ui, path_rect, path);
            }
        });
        self.search_diff_viewer(ui, &commit.hash, self.search_diff_display_mode);
    }

    fn branches_view(&mut self, ui: &mut Ui) {
        let Some(snapshot) = &self.snapshot else {
            empty_state(ui, self.loading_repo, self.language);
            return;
        };

        let local = snapshot
            .branches
            .iter()
            .filter(|branch| !branch.remote)
            .cloned()
            .collect::<Vec<_>>();
        let remote = snapshot
            .branches
            .iter()
            .filter(|branch| branch.remote)
            .cloned()
            .collect::<Vec<_>>();
        let remote_branch_names = remote
            .iter()
            .map(|branch| branch.name.clone())
            .collect::<Vec<_>>();
        let remote_names = snapshot
            .remotes
            .iter()
            .map(|remote| remote.name.clone())
            .collect::<Vec<_>>();
        let branch_actions_enabled = !self.branch_actions_busy();
        let pending_branch_checkout = self.pending_branch_checkout.clone();
        let mut action = None;

        content_panel_frame(theme::bg()).show(ui, |ui| {
            resource_header(
                ui,
                self.tr("branch.local"),
                &format!("{} local  {} remote", local.len(), remote.len()),
                |ui| {
                    if ui
                        .add_enabled(
                            branch_actions_enabled,
                            egui::Button::new(self.tr("branch.create")),
                        )
                        .clicked()
                    {
                        action = Some(BranchMenuAction::Create);
                    }
                },
            );
            branch_table_header(ui, self.language);
            ScrollArea::vertical()
                .id_salt("branches_table_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for branch in &local {
                        let current =
                            branch_current_for_display(branch, pending_branch_checkout.as_deref());
                        branch_table_row(
                            ui,
                            current,
                            false,
                            &branch.name,
                            branch
                                .upstream
                                .as_ref()
                                .map(|upstream| upstream.name.as_str()),
                            &remote_branch_names,
                            &remote_names,
                            self.language,
                            branch_actions_enabled,
                            &mut action,
                        );
                    }
                    for branch in &remote {
                        branch_table_row(
                            ui,
                            branch.current,
                            true,
                            &branch.name,
                            branch
                                .upstream
                                .as_ref()
                                .map(|upstream| upstream.name.as_str()),
                            &remote_branch_names,
                            &remote_names,
                            self.language,
                            branch_actions_enabled,
                            &mut action,
                        );
                    }
                    if local.is_empty() && remote.is_empty() {
                        empty_list_panel(ui, self.tr("branch.none"));
                    }
                });
        });

        if branch_actions_enabled {
            if let Some(action) = action {
                self.handle_branch_action(action);
            }
        }
    }

    fn tags_view(&mut self, ui: &mut Ui) {
        let Some(snapshot) = &self.snapshot else {
            empty_state(ui, self.loading_repo, self.language);
            return;
        };

        let tags = snapshot.tags.clone();
        let mut action = None;

        content_panel_frame(theme::bg()).show(ui, |ui| {
            resource_header(
                ui,
                self.tr("tag.title"),
                &format!("{} tags", tags.len()),
                |ui| {
                    if ui.button(self.tr("tag.create")).clicked() {
                        action = Some(TagMenuAction::Create);
                    }
                },
            );
            tag_table_header(ui, self.language);
            ScrollArea::vertical()
                .id_salt("tags_table_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if tags.is_empty() {
                        empty_list_panel(ui, self.tr("tag.none"));
                    } else {
                        for tag in &tags {
                            tag_table_row(ui, tag, self.language, &mut action);
                        }
                    }
                });
        });

        if let Some(action) = action {
            self.handle_tag_action(action);
        }
    }

    fn stashes_view(&mut self, ui: &mut Ui) {
        let Some(snapshot) = &self.snapshot else {
            empty_state(ui, self.loading_repo, self.language);
            return;
        };

        let stashes = snapshot.stashes.clone();
        let can_stash = !snapshot.status.is_empty();
        let mut action = None;

        content_panel_frame(theme::bg()).show(ui, |ui| {
            resource_header(
                ui,
                self.tr("stash.title"),
                &format!("{} stashes", stashes.len()),
                |ui| {
                    if ui
                        .add_enabled(can_stash, egui::Button::new(self.tr("stash.create")))
                        .clicked()
                    {
                        action = Some(StashMenuAction::Create);
                    }
                },
            );
            stash_table_header(ui, self.language);
            ScrollArea::vertical()
                .id_salt("stashes_table_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if stashes.is_empty() {
                        empty_list_panel(ui, self.tr("stash.none"));
                    } else {
                        for stash in &stashes {
                            stash_table_row(ui, stash, self.language, &mut action);
                        }
                    }
                });
        });

        if let Some(action) = action {
            self.handle_stash_action(action);
        }
    }

    fn handle_commit_menu_action(&mut self, action: CommitMenuAction) {
        match action {
            CommitMenuAction::Checkout { hash, short_hash } => {
                self.pending_commit_action =
                    Some(CommitActionDialog::ConfirmCheckout { hash, short_hash });
            }
            CommitMenuAction::CreateBranch { hash, short_hash } => {
                self.pending_commit_action = Some(CommitActionDialog::CreateBranch {
                    hash,
                    short_hash: short_hash.clone(),
                    name: format!("branch-{short_hash}"),
                    checkout: true,
                });
            }
            CommitMenuAction::CreateTag { hash, short_hash } => {
                let remote = self.default_remote_name();
                self.pending_commit_action = Some(CommitActionDialog::CreateTag {
                    hash,
                    short_hash: short_hash.clone(),
                    name: format!("v-{short_hash}"),
                    push_after_create: false,
                    remote,
                });
            }
            CommitMenuAction::CherryPick { hash, short_hash } => {
                self.pending_commit_action =
                    Some(CommitActionDialog::ConfirmCherryPick { hash, short_hash });
            }
            CommitMenuAction::Revert { hash, short_hash } => {
                self.pending_commit_action =
                    Some(CommitActionDialog::ConfirmRevert { hash, short_hash });
            }
            CommitMenuAction::Reset { hash, short_hash } => {
                self.pending_commit_action = Some(CommitActionDialog::ConfirmReset {
                    hash,
                    short_hash,
                    mode: ResetMode::Mixed,
                });
            }
            CommitMenuAction::CompareWithWorktree { hash, short_hash }
            | CommitMenuAction::ExternalDiff { hash, short_hash } => {
                self.open_commit_diff_tool(hash, short_hash);
            }
            CommitMenuAction::OpenRemote { hash } => {
                self.open_commit_remote_url(&hash);
            }
        }
    }

    fn handle_worktree_action(&mut self, action: WorktreeMenuAction) {
        match action {
            WorktreeMenuAction::Stage { path } => {
                self.execute_git_action(move |root| git::stage_path(root, &path));
            }
            WorktreeMenuAction::StageAll => {
                self.execute_git_action(|root| git::stage_all(root));
            }
            WorktreeMenuAction::Unstage { path } => {
                self.execute_git_action(move |root| git::unstage_path(root, &path));
            }
            WorktreeMenuAction::UnstageAll => {
                self.execute_git_action(|root| git::unstage_all(root));
            }
            WorktreeMenuAction::Discard { path, untracked } => {
                self.pending_worktree_action =
                    Some(WorktreeActionDialog::ConfirmDiscard { path, untracked });
            }
            WorktreeMenuAction::ResolveConflict { path } => {
                self.pending_worktree_action = Some(WorktreeActionDialog::ResolveConflicts {
                    selected_path: Some(path),
                });
            }
            WorktreeMenuAction::AddToGitIgnore { pattern } => {
                self.execute_git_action(move |root| git::add_to_gitignore(root, &pattern));
            }
        }
    }

    fn poll_merge_tool_task(&mut self, ctx: &egui::Context) {
        if let Some(receiver) = self.merge_tool_task.take() {
            match receiver.try_recv() {
                Ok((root, Ok(_success))) => {
                    self.last_notice = None;
                    self.load_repository_uncached(root);
                    ctx.request_repaint();
                }
                Ok((_, Err(error))) => {
                    self.error = Some(error.to_string());
                    self.last_notice = None;
                    ctx.request_repaint();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.merge_tool_task = Some(receiver);
                    ctx.request_repaint_after(Duration::from_millis(80));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.last_notice = None;
                    ctx.request_repaint();
                }
            }
        }
    }

    fn open_conflict_merge_tool(&mut self, path: &str) {
        let Some(root) = self.snapshot.as_ref().map(|snapshot| snapshot.root.clone()) else {
            return;
        };

        let (base, local, remote) = match git::conflict_file_versions(&root, path) {
            Ok(versions) => versions,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };

        let temp_dir = env::temp_dir().join("git-agent-conflicts").join(format!(
            "{}-{}",
            std::process::id(),
            conflict_temp_name(path)
        ));
        if let Err(error) = fs::create_dir_all(&temp_dir) {
            self.error = Some(error.to_string());
            return;
        }

        let base_path = temp_dir.join("base.txt");
        let local_path = temp_dir.join("local.txt");
        let remote_path = temp_dir.join("remote.txt");
        let output_path = root.join(path);
        for (target, text) in [
            (&base_path, base),
            (&local_path, local),
            (&remote_path, remote),
        ] {
            if let Err(error) = fs::write(target, text) {
                self.error = Some(error.to_string());
                return;
            }
        }

        let merge_exe = match env::current_exe() {
            Ok(exe) => exe.with_file_name(if cfg!(windows) {
                "git-agent-merge.exe"
            } else {
                "git-agent-merge"
            }),
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };

        let mut child = match Command::new(&merge_exe)
            .current_dir(&root)
            .arg("--base")
            .arg(&base_path)
            .arg("--local")
            .arg(&local_path)
            .arg("--remote")
            .arg(&remote_path)
            .arg("--output")
            .arg(&output_path)
            .arg("--repo-root")
            .arg(&root)
            .arg("--stage")
            .arg("--theme")
            .arg(merge_theme_arg(self.theme_mode))
            .arg("--language")
            .arg(merge_language_arg(self.language))
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                self.error = Some(format!("{}: {error}", merge_exe.display()));
                return;
            }
        };

        let root_for_reload = root.clone();
        let (sender, receiver) = mpsc::channel();
        self.merge_tool_task = Some(receiver);
        thread::spawn(move || {
            let result = child
                .wait()
                .map(|status| status.success())
                .map_err(Into::into);
            let _ = sender.send((root_for_reload, result));
        });
    }

    fn handle_stash_action(&mut self, action: StashMenuAction) {
        match action {
            StashMenuAction::Create => {
                self.pending_stash_action = Some(StashActionDialog::Create {
                    message: String::new(),
                });
            }
            StashMenuAction::Apply { selector } => {
                self.execute_git_action(move |root| git::stash_apply(root, &selector));
            }
            StashMenuAction::Pop { selector } => {
                self.execute_git_action(move |root| git::stash_pop(root, &selector));
            }
            StashMenuAction::Drop { selector, message } => {
                self.pending_stash_action =
                    Some(StashActionDialog::ConfirmDrop { selector, message });
            }
        }
    }

    fn handle_branch_action(&mut self, action: BranchMenuAction) {
        match action {
            BranchMenuAction::Create => {
                self.pending_branch_action = Some(BranchActionDialog::Create {
                    name: String::new(),
                    checkout: true,
                });
            }
            BranchMenuAction::Checkout { name } => {
                self.request_branch_checkout(name);
            }
            BranchMenuAction::CheckoutRemote { remote_branch } => {
                let local_branch = remote_branch
                    .split_once('/')
                    .map(|(_, branch)| branch.to_owned())
                    .unwrap_or_else(|| remote_branch.clone());
                self.pending_branch_action = Some(BranchActionDialog::CheckoutRemote {
                    remote_branch,
                    local_branch,
                });
            }
            BranchMenuAction::MergeIntoCurrent { name } => {
                self.active_view = MainView::Workspace;
                self.commit_message.clear();
                self.start_remote_git_action(move |root| git::merge_branch(root, &name));
            }
            BranchMenuAction::RebaseCurrentOnto { name } => {
                self.start_remote_git_action(move |root| git::rebase_current_onto(root, &name));
            }
            BranchMenuAction::FetchTracked { remote_branch } => {
                self.start_remote_git_action(move |root| {
                    git::fetch_remote_branch(root, &remote_branch)
                });
            }
            BranchMenuAction::PullTracked { name } => {
                if self
                    .snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.branch == name)
                {
                    self.open_pull_dialog(Some(name));
                }
            }
            BranchMenuAction::PushTracked { name } => {
                self.open_push_dialog(Some(name), None);
            }
            BranchMenuAction::PushToRemote { name, remote } => {
                self.open_push_dialog(Some(name), Some(remote));
            }
            BranchMenuAction::TrackRemote {
                name,
                remote_branch,
            } => {
                self.start_remote_git_action(move |root| {
                    if let Some(remote_branch) = remote_branch {
                        git::set_branch_upstream(root, &name, &remote_branch)
                    } else {
                        git::unset_branch_upstream(root, &name)
                    }
                });
            }
            BranchMenuAction::CompareWithCurrent { name } => {
                self.open_branch_compare_url(&name);
            }
            BranchMenuAction::Rename { name } => {
                self.pending_branch_action = Some(BranchActionDialog::Rename {
                    old_name: name.clone(),
                    new_name: name,
                });
            }
            BranchMenuAction::Delete { name } => {
                self.pending_branch_action =
                    Some(BranchActionDialog::ConfirmDelete { name, force: false });
            }
            BranchMenuAction::DeleteRemote { remote_branch } => {
                self.pending_branch_action =
                    Some(BranchActionDialog::ConfirmDeleteRemote { remote_branch });
            }
            BranchMenuAction::CreatePullRequest { name } => {
                self.open_branch_pull_request_url(&name);
            }
        }
    }

    fn handle_tag_action(&mut self, action: TagMenuAction) {
        match action {
            TagMenuAction::Create => {
                let remote = self.default_remote_name();
                self.pending_tag_action = Some(TagActionDialog::Create {
                    name: String::new(),
                    push_after_create: false,
                    remote,
                });
            }
            TagMenuAction::Checkout { name } => {
                self.execute_git_action(move |root| git::checkout_tag(root, &name));
            }
            TagMenuAction::Push { name } => {
                let remote = self.default_remote_name();
                self.pending_tag_action = Some(TagActionDialog::Push { name, remote });
            }
            TagMenuAction::Delete { name } => {
                self.pending_tag_action = Some(TagActionDialog::ConfirmDelete { name });
            }
        }
    }

    fn details(&mut self, ui: &mut Ui) {
        ScrollArea::vertical()
            .id_salt("side_details_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| self.details_content(ui));
    }

    fn details_content(&mut self, ui: &mut Ui) {
        ui.add_space(18.0);
        panel_heading(ui, self.tr("commit.details"));
        ui.add_space(8.0);

        let Some(snapshot) = &self.snapshot else {
            ui.label(RichText::new(self.tr("repo.none")).color(theme::muted()));
            return;
        };

        let selected = self
            .selected_commit
            .and_then(|index| snapshot.commits.get(index))
            .or_else(|| snapshot.commits.first())
            .cloned();

        if let Some(commit) = selected {
            ui.label(
                RichText::new(&commit.subject)
                    .size(20.0)
                    .strong()
                    .color(theme::text()),
            );
            ui.add_space(12.0);
            detail_line(ui, self.tr("commit.hash"), &commit.hash);
            detail_line(ui, self.tr("commit.author"), &commit.author);
            detail_line(ui, self.tr("commit.when"), &commit.relative_time);
            detail_line(
                ui,
                self.tr("commit.parents"),
                &commit.parents.len().to_string(),
            );
            ui.add_space(18.0);
            panel_heading_inline(ui, self.tr("commit.changed_files"));
            ui.add_space(6.0);

            if self.loading_details_hash.as_deref() == Some(commit.hash.as_str()) {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(self.tr("commit.loading_files")).color(theme::muted()));
                });
            } else if let Some(details) = self.details_cache.get(&commit.hash) {
                if details.files.is_empty() {
                    ui.label(RichText::new(self.tr("commit.no_changes")).color(theme::muted()));
                } else {
                    let mut clicked_file = None;
                    ScrollArea::vertical()
                        .id_salt("side_details_files_scroll")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for file in &details.files {
                                let selected = self.selected_file_path.as_deref()
                                    == Some(file.diff_path.as_str());
                                if file_change_row(ui, &file.status, &file.path, selected).clicked()
                                {
                                    clicked_file = Some(file.diff_path.clone());
                                }
                            }
                        });
                    if let Some(path) = clicked_file {
                        self.select_changed_file_for_diff(path);
                    }
                }
            } else {
                ui.label(
                    RichText::new(self.tr("commit.select_to_load_files")).color(theme::muted()),
                );
            }

            ui.add_space(14.0);
            panel_heading_inline(ui, self.tr("commit.diff"));
            ui.add_space(6.0);
            self.diff_viewer(ui, &commit.hash, self.history_diff_display_mode);
        } else {
            ui.label(RichText::new(self.tr("commit.none")).color(theme::muted()));
        }

        ui.add_space(18.0);
        self.worktree_diff_viewer(ui);
    }

    fn commit_panel(&mut self, ui: &mut Ui, staged_count: usize, panel_height: f32) {
        let (panel_rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), panel_height),
            Sense::hover(),
        );
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(panel_rect), |ui| {
            let card_rect = panel_rect;
            ui.set_clip_rect(workspace_card_clip_rect(panel_rect));
            workspace_card_frame(12, 10).show(ui, |ui| {
                safe_set_min_size(
                    ui,
                    frame_inner_size(ui.available_width(), panel_height, 12, 10),
                );
                ui.horizontal(|ui| {
                    panel_heading_inline(ui, self.tr("commit.panel"));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.menu_button(self.tr("commit.options"), |ui| {
                            self.commit_options_menu(ui);
                        });
                        self.commit_history_icon_menu(ui);
                    });
                });
                ui.add_space(8.0);
                let message_hint = self.tr("commit.message");
                let message_height = commit_message_editor_height(ui.available_height());
                let commit_message_input = ui.make_persistent_id("commit_message_input");
                let message_response = commit_message_editor_ui(
                    ui,
                    &mut self.commit_message,
                    commit_message_input,
                    message_hint,
                    Vec2::new(ui.available_width(), message_height),
                );
                if self.focus_commit_message {
                    message_response.request_focus();
                    self.focus_commit_message = false;
                }
                let mut shortcut_commit = false;
                if message_response.has_focus() {
                    let (commit_shortcut, toggle_push_immediately, toggle_amend) =
                        ui.input(|input| {
                            let commit_shortcut =
                                input.modifiers.ctrl && input.key_pressed(egui::Key::Enter);
                            (
                                commit_shortcut,
                                input.modifiers.ctrl
                                    && !input.modifiers.shift
                                    && input.key_pressed(egui::Key::P),
                                input.modifiers.ctrl
                                    && !input.modifiers.shift
                                    && input.key_pressed(egui::Key::L),
                            )
                        });
                    shortcut_commit = commit_shortcut;
                    if toggle_push_immediately {
                        self.toggle_push_immediately();
                    }
                    if toggle_amend {
                        self.toggle_amend();
                    }
                }
                ui.add_space(
                    (ui.available_height() - COMMIT_BUTTON_ROW_HEIGHT)
                        .max(COMMIT_MESSAGE_BOTTOM_GAP),
                );
                let can_commit = (staged_count > 0 || self.commit_state.amend)
                    && !self.commit_message.trim().is_empty()
                    && !self.loading_repo
                    && !self.remote_git_busy();
                let commit_clicked = self.commit_action_row(ui, can_commit);
                if commit_clicked {
                    self.commit_current_message(staged_count);
                }
                if shortcut_commit {
                    self.commit_current_message(staged_count);
                }
            });
            paint_workspace_card_inset_shadow(ui, card_rect);
        });
    }

    fn commit_action_row(&mut self, ui: &mut Ui, can_commit: bool) -> bool {
        let row_width = ui.available_width();
        let (row_rect, _) = ui.allocate_exact_size(
            Vec2::new(row_width, COMMIT_BUTTON_ROW_HEIGHT),
            Sense::hover(),
        );
        let button_size = COMMIT_SUBMIT_BUTTON_SIZE;
        let content_rect = row_rect.shrink2(Vec2::new(8.0, 2.0));
        let button_rect = Rect::from_min_size(
            Pos2::new(
                content_rect.right() - button_size.x,
                content_rect.center().y - button_size.y / 2.0 - 1.0,
            ),
            button_size,
        );
        let checkbox_rect = Rect::from_min_max(
            content_rect.min,
            Pos2::new(
                (button_rect.left() - 14.0).max(content_rect.left()),
                content_rect.bottom(),
            ),
        );
        let push_label = self.tr("commit.push_immediately").to_owned();
        let amend_label = self.tr("commit.amend").to_owned();
        let mut changed = false;

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(checkbox_rect), |ui| {
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                changed |=
                    commit_checkbox(ui, &mut self.commit_state.push_immediately, &push_label)
                        .changed();
                ui.add_space(18.0);
                changed |=
                    commit_checkbox(ui, &mut self.commit_state.amend, &amend_label).changed();
            });
        });

        if changed {
            self.save_commit_state_for_active_repo();
        }

        let commit_button_text = self.tr("commit.button.short");
        commit_submit_button(ui, button_rect, commit_button_text, can_commit).clicked()
    }

    fn commit_history_icon_menu(&mut self, ui: &mut Ui) {
        let response = icon_button(ui, UiIcon::History, self.tr("commit.history"), true);
        let popup_id = ui.make_persistent_id("commit_history_popup");
        if response.clicked() {
            ui.memory_mut(|memory| memory.toggle_popup(popup_id));
        }
        egui::popup::popup_below_widget(
            ui,
            popup_id,
            &response,
            egui::popup::PopupCloseBehavior::CloseOnClick,
            |ui| {
                self.commit_history_menu(ui);
            },
        );
    }

    fn commit_history_menu(&mut self, ui: &mut Ui) {
        if self.commit_state.message_history.is_empty() {
            ui.label(RichText::new(self.tr("commit.history_empty")).color(theme::muted()));
            return;
        }

        let entries = self.commit_state.message_history.clone();
        let mut remove_index = None;
        for (index, message) in entries.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(false, truncate_middle(message, 72).as_str())
                    .clicked()
                {
                    self.commit_message = message.clone();
                    ui.close_menu();
                }
                if ui.button("\u{00d7}").clicked() {
                    remove_index = Some(index);
                }
            });
        }
        if let Some(index) = remove_index {
            self.remove_commit_message_history(index);
        }
    }

    fn commit_options_menu(&mut self, ui: &mut Ui) {
        let mut changed = false;
        let amend_label = self.tr("commit.amend");
        let no_verify_label = self.tr("commit.no_verify");
        let gpg_sign_label = self.tr("commit.gpg_sign");
        changed |= commit_checkbox(ui, &mut self.commit_state.amend, amend_label).changed();
        changed |= commit_checkbox(ui, &mut self.commit_state.no_verify, no_verify_label).changed();
        changed |= commit_checkbox(ui, &mut self.commit_state.gpg_sign, gpg_sign_label).changed();
        if changed {
            self.save_commit_state_for_active_repo();
        }
    }

    fn diff_viewer(&mut self, ui: &mut Ui, hash: &str, mode: DiffDisplayMode) {
        let Some(path) = self.selected_file_path.clone() else {
            ui.label(RichText::new(self.tr("commit.select_file")).color(theme::muted()));
            return;
        };
        let key = git::diff_key(hash, &path);

        if self.loading_diff_key.as_deref() == Some(key.as_str()) {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(self.tr("diff.loading")).color(theme::muted()));
            });
            return;
        }

        let Some(diff) = self.diff_cache.get(&key) else {
            ui.label(RichText::new(self.tr("diff.queued")).color(theme::muted()));
            return;
        };

        if diff.text.trim().is_empty() {
            ui.label(RichText::new(self.tr("diff.empty")).color(theme::muted()));
            return;
        }
        let diff_text = diff.text.clone();
        let is_truncated = diff_text.lines().count() > 1_200;
        let truncated_label = self.tr("diff.truncated");

        let diff_response = diff_panel_frame().show(ui, |ui| {
            let max_height = ui.available_height().max(160.0);
            ScrollArea::both()
                .id_salt((
                    "commit_diff_scroll",
                    hash,
                    path,
                    diff_display_mode_salt(mode),
                ))
                .max_height(max_height)
                .show(ui, |ui| {
                    render_unified_diff(
                        ui,
                        &diff_text,
                        mode,
                        self.language,
                        &mut self.selected_diff_rows,
                    );
                    if is_truncated {
                        ui.label(RichText::new(truncated_label).color(theme::muted()));
                    }
                });
        });
        let diff_rect = diff_response.response.rect;
        paint_workspace_card_inset_shadow(ui, diff_rect);
    }

    fn search_diff_viewer(&mut self, ui: &mut Ui, hash: &str, mode: DiffDisplayMode) {
        let Some(path) = self.search_selected_file_path.clone() else {
            ui.label(RichText::new(self.tr("commit.select_file")).color(theme::muted()));
            return;
        };
        let key = git::diff_key(hash, &path);

        if self.loading_diff_key.as_deref() == Some(key.as_str()) {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(self.tr("diff.loading")).color(theme::muted()));
            });
            return;
        }

        let Some(diff) = self.diff_cache.get(&key) else {
            ui.label(RichText::new(self.tr("diff.queued")).color(theme::muted()));
            return;
        };

        if diff.text.trim().is_empty() {
            ui.label(RichText::new(self.tr("diff.empty")).color(theme::muted()));
            return;
        }
        let diff_text = diff.text.clone();
        let is_truncated = diff_text.lines().count() > 1_200;
        let truncated_label = self.tr("diff.truncated");

        let diff_response = diff_panel_frame().show(ui, |ui| {
            let max_height = ui.available_height().max(160.0);
            ScrollArea::both()
                .id_salt((
                    "search_commit_diff_scroll",
                    hash,
                    path,
                    diff_display_mode_salt(mode),
                ))
                .max_height(max_height)
                .show(ui, |ui| {
                    render_unified_diff(
                        ui,
                        &diff_text,
                        mode,
                        self.language,
                        &mut self.search_selected_diff_rows,
                    );
                    if is_truncated {
                        ui.label(RichText::new(truncated_label).color(theme::muted()));
                    }
                });
        });
        let diff_rect = diff_response.response.rect;
        paint_workspace_card_inset_shadow(ui, diff_rect);
    }

    fn worktree_diff_viewer(&self, ui: &mut Ui) {
        let Some(selected) = &self.selected_worktree_file else {
            return;
        };

        let key = git::worktree_diff_key(&selected.path, selected.staged);
        if self.loading_diff_key.as_deref() == Some(key.as_str()) {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(self.tr("diff.loading")).color(theme::muted()));
            });
            return;
        }

        let Some(diff) = self.diff_cache.get(&key) else {
            ui.label(RichText::new(self.tr("diff.queued")).color(theme::muted()));
            return;
        };

        if diff.text.trim().is_empty() {
            ui.label(RichText::new(self.tr("diff.empty")).color(theme::muted()));
            return;
        }

        let available_diff_size = safe_ui_size(ui.available_size());
        let diff_response = worktree_diff_panel_frame().show(ui, |ui| {
            let inner_size = safe_ui_size(ui.available_size());
            safe_set_min_size(ui, inner_size);
            let available_diff_height = available_diff_size
                .y
                .max(safe_ui_length(ui.available_height()));
            let max_diff_height = available_diff_height.max(160.0);
            ScrollArea::both()
                .id_salt(("worktree_diff_scroll", key))
                .max_height(max_diff_height)
                .show(ui, |ui| {
                    let mut selected_rows = Vec::new();
                    render_unified_diff(
                        ui,
                        &diff.text,
                        DiffDisplayMode::Full,
                        self.language,
                        &mut selected_rows,
                    );
                    if diff.text.lines().count() > 1_200 {
                        ui.label(RichText::new(self.tr("diff.truncated")).color(theme::muted()));
                    }
                });
        });
        let diff_rect = diff_response.response.rect;
        paint_workspace_card_inset_shadow(ui, diff_rect);
    }

    fn pull_action_modal(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.pending_pull_action.take() else {
            return;
        };

        let mut keep_open = true;
        let mut close_after = false;
        let mut execute: Option<Box<dyn FnOnce(&std::path::Path) -> anyhow::Result<()> + Send>> =
            None;
        let actions_enabled = !self.branch_actions_busy();
        let remotes = self.remote_names();
        if !remotes.iter().any(|remote| remote == &dialog.remote) {
            dialog.remote = remotes
                .first()
                .cloned()
                .unwrap_or_else(|| "origin".to_owned());
        }
        let remote_branches = self.remote_branch_names_for(&dialog.remote);
        if !remote_branches
            .iter()
            .any(|branch| branch == &dialog.remote_branch)
        {
            dialog.remote_branch = remote_branches.first().cloned().unwrap_or_default();
        }
        let remote_url = self.remote_url_for_name(&dialog.remote).unwrap_or_default();

        compact_action_dialog(ctx, self.tr("pull.title"), PULL_DIALOG_WIDTH, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(self.tr("pull.remote"))
                        .small()
                        .color(theme::muted()),
                );
                egui::ComboBox::from_id_salt("pull_remote_selector")
                    .width(ui.available_width())
                    .selected_text(dialog.remote.as_str())
                    .show_ui(ui, |ui| {
                        for remote in &remotes {
                            if ui
                                .selectable_value(&mut dialog.remote, remote.clone(), remote)
                                .clicked()
                            {
                                let branches = self.remote_branch_names_for(&dialog.remote);
                                dialog.remote_branch =
                                    branches.first().cloned().unwrap_or_default();
                            }
                        }
                    });
            });
            ui.add_space(4.0);
            themed_text_edit_selection(ui);
            let mut remote_url_display = remote_url.clone();
            ui.add_enabled(
                false,
                themed_singleline_text_edit(&mut remote_url_display, ""),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(self.tr("pull.remote_branch"))
                        .small()
                        .color(theme::muted()),
                );
                egui::ComboBox::from_id_salt("pull_remote_branch_selector")
                    .width((ui.available_width() - 76.0).max(180.0))
                    .selected_text(dialog.remote_branch.as_str())
                    .show_ui(ui, |ui| {
                        for branch in &remote_branches {
                            ui.selectable_value(&mut dialog.remote_branch, branch.clone(), branch);
                        }
                    });
                if ui
                    .add_enabled(actions_enabled, egui::Button::new(self.tr("pull.refresh")))
                    .clicked()
                    && !dialog.remote.trim().is_empty()
                {
                    let remote = dialog.remote.trim().to_owned();
                    execute = Some(Box::new(move |root| git::fetch_remote(root, &remote)));
                }
            });
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(self.tr("pull.local_branch"))
                        .small()
                        .color(theme::muted()),
                );
                ui.label(RichText::new(dialog.local_branch.as_str()).color(theme::text()));
            });
            ui.add_space(8.0);
            ui.label(
                RichText::new(self.tr("pull.options"))
                    .small()
                    .color(theme::muted()),
            );
            ui.group(|ui| {
                ui.checkbox(&mut dialog.commit_merge, self.tr("pull.commit_merge"));
                ui.checkbox(&mut dialog.include_tags, self.tr("pull.include_tags"));
                ui.checkbox(
                    &mut dialog.force_merge_commit,
                    self.tr("pull.force_merge_commit"),
                );
                ui.checkbox(&mut dialog.rebase, self.tr("pull.rebase"));
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(self.tr("dialog.cancel")).clicked() {
                        close_after = true;
                    }
                    let default_action_enabled = actions_enabled
                        && !dialog.remote.trim().is_empty()
                        && !dialog.remote_branch.trim().is_empty();
                    let submit_requested = dialog_default_submit_requested(ui);
                    if ui
                        .add_enabled(
                            default_action_enabled,
                            egui::Button::new(self.tr("action.pull")),
                        )
                        .clicked()
                        || (submit_requested && default_action_enabled)
                    {
                        let remote = dialog.remote.trim().to_owned();
                        let remote_branch = dialog.remote_branch.trim().to_owned();
                        let options = git::PullOptions {
                            commit_merge: dialog.commit_merge,
                            include_tags: dialog.include_tags,
                            force_merge_commit: dialog.force_merge_commit,
                            rebase: dialog.rebase,
                        };
                        execute = Some(Box::new(move |root| {
                            git::pull_from_remote(root, &remote, &remote_branch, options)
                        }));
                        close_after = true;
                    }
                });
            });
        });

        if let Some(action) = execute {
            self.execute_git_action(action);
        }
        if close_after {
            keep_open = false;
        }
        if keep_open {
            self.pending_pull_action = Some(dialog);
        }
    }

    fn push_action_modal(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.pending_push_action.take() else {
            return;
        };

        let mut keep_open = true;
        let mut close_after = false;
        let mut execute: Option<Box<dyn FnOnce(&std::path::Path) -> anyhow::Result<()> + Send>> =
            None;
        let actions_enabled = !self.branch_actions_busy();
        let remotes = self.remote_names();
        if !remotes.iter().any(|remote| remote == &dialog.remote) {
            dialog.remote = remotes
                .first()
                .cloned()
                .unwrap_or_else(|| "origin".to_owned());
            let branches = self.remote_branch_names_for(&dialog.remote);
            Self::update_push_rows_for_remote(&mut dialog.rows, &dialog.remote, &branches);
        }
        let remote_branches = self.remote_branch_names_for(&dialog.remote);
        let remote_url = self.remote_url_for_name(&dialog.remote).unwrap_or_default();
        let title = format!(
            "{}: {}",
            self.tr("push.title"),
            self.active_repo_display_name()
        );

        compact_action_dialog(ctx, &title, PUSH_DIALOG_WIDTH, |ui| {
            let mut remote_url_display = remote_url.clone();
            push_remote_form_row(ui, self.tr("push.remote"), &mut remote_url_display, |ui| {
                egui::ComboBox::from_id_salt("push_remote_selector")
                    .width(PUSH_REMOTE_FORM_SELECTOR_WIDTH)
                    .selected_text(dialog.remote.as_str())
                    .show_ui(ui, |ui| {
                        for remote in &remotes {
                            if ui
                                .selectable_value(&mut dialog.remote, remote.clone(), remote)
                                .clicked()
                            {
                                let branches = self.remote_branch_names_for(&dialog.remote);
                                Self::update_push_rows_for_remote(
                                    &mut dialog.rows,
                                    &dialog.remote,
                                    &branches,
                                );
                            }
                        }
                    });
            });

            ui.add_space(8.0);
            ui.label(
                RichText::new(self.tr("push.branches"))
                    .small()
                    .color(theme::muted()),
            );
            egui::Frame::new()
                .fill(theme::panel_soft())
                .stroke(Stroke::new(1.0, theme::inset_shadow()))
                .inner_margin(egui::Margin::symmetric(6, 6))
                .show(ui, |ui| {
                    let table_width = ui.available_width();
                    push_branch_table_header(
                        ui,
                        table_width,
                        self.tr("push.select"),
                        self.tr("push.local_branch"),
                        self.tr("push.remote_branch"),
                        self.tr("push.track"),
                    );
                    for row in &mut dialog.rows {
                        push_branch_table_row(ui, table_width, row, &remote_branches);
                    }
                });

            ui.add_space(8.0);
            let mut all_selected =
                !dialog.rows.is_empty() && dialog.rows.iter().all(|row| row.selected);
            if ui
                .checkbox(&mut all_selected, self.tr("push.select_all"))
                .changed()
            {
                for row in &mut dialog.rows {
                    row.selected = all_selected;
                    if row.selected && row.remote_branch.trim().is_empty() {
                        row.remote_branch = row.local_branch.clone();
                    }
                }
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.checkbox(&mut dialog.push_tags, self.tr("push.push_tags"));
                ui.checkbox(&mut dialog.force, self.tr("push.force"));
            });

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(self.tr("dialog.cancel")).clicked() {
                        close_after = true;
                    }
                    let selected_push_branches = Self::selected_push_branches(&dialog);
                    let selected_count = dialog.rows.iter().filter(|row| row.selected).count();
                    let has_blank_selected_branch = dialog
                        .rows
                        .iter()
                        .any(|row| row.selected && row.remote_branch.trim().is_empty());
                    let default_action_enabled = actions_enabled
                        && !dialog.remote.trim().is_empty()
                        && !has_blank_selected_branch
                        && (selected_count > 0 || dialog.push_tags);
                    let submit_requested = dialog_default_submit_requested(ui);
                    if ui
                        .add_enabled(
                            default_action_enabled,
                            egui::Button::new(self.tr("action.push")),
                        )
                        .clicked()
                        || (submit_requested && default_action_enabled)
                    {
                        let remote = dialog.remote.trim().to_owned();
                        let branches = selected_push_branches;
                        let options = git::PushOptions {
                            push_tags: dialog.push_tags,
                            force: dialog.force,
                        };
                        execute = Some(Box::new(move |root| {
                            git::push_selected(root, &remote, &branches, options)
                        }));
                        close_after = true;
                    }
                });
            });
        });

        if let Some(action) = execute {
            self.execute_git_action(action);
        }
        if close_after {
            keep_open = false;
        }
        if keep_open {
            self.pending_push_action = Some(dialog);
        }
    }

    fn commit_action_modal(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.pending_commit_action.take() else {
            return;
        };

        let mut keep_open = true;
        let mut close_after = false;
        let mut execute: Option<Box<dyn FnOnce(&std::path::Path) -> anyhow::Result<()> + Send>> =
            None;
        let remotes = self.remote_names();

        let title = match &dialog {
            CommitActionDialog::CreateBranch { .. } => self.tr("branch.create"),
            CommitActionDialog::CreateTag { .. } => self.tr("menu.create_tag"),
            CommitActionDialog::ConfirmCheckout { .. } => self.tr("menu.checkout_commit"),
            CommitActionDialog::ConfirmCherryPick { .. } => self.tr("menu.cherry_pick"),
            CommitActionDialog::ConfirmCherryPickBatch { .. } => {
                self.tr("commit.cherry_pick_batch")
            }
            CommitActionDialog::ConfirmRevert { .. } => self.tr("menu.revert"),
            CommitActionDialog::ConfirmReset { .. } => self.tr("menu.reset"),
        };

        compact_action_dialog(ctx, title, ACTION_DIALOG_WIDTH, |ui| match &mut dialog {
            CommitActionDialog::CreateBranch {
                hash,
                short_hash,
                name,
                checkout,
            } => {
                ui.label(
                    RichText::new(format!("{} {short_hash}", self.tr("commit.create_from")))
                        .color(theme::muted()),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(self.tr("branch.name"))
                        .small()
                        .color(theme::muted()),
                );
                ui.add(TextEdit::singleline(name));
                ui.checkbox(checkout, self.tr("branch.checkout"));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let submit_requested = dialog_default_submit_requested(ui);
                    if (ui.button(self.tr("dialog.create")).clicked() || submit_requested)
                        && !name.trim().is_empty()
                    {
                        let branch_name = name.trim().to_owned();
                        let hash = hash.clone();
                        let checkout = *checkout;
                        execute = Some(Box::new(move |root| {
                            git::create_branch(root, &branch_name, &hash, checkout)
                        }));
                        close_after = true;
                    }
                    if ui.button(self.tr("dialog.cancel")).clicked() {
                        close_after = true;
                    }
                });
            }
            CommitActionDialog::CreateTag {
                hash,
                short_hash,
                name,
                push_after_create,
                remote,
            } => {
                ui.label(
                    RichText::new(format!("{} {short_hash}", self.tr("commit.tag_commit")))
                        .color(theme::muted()),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(self.tr("tag.name"))
                        .small()
                        .color(theme::muted()),
                );
                themed_text_edit_selection(ui);
                ui.add(themed_singleline_text_edit(name, ""));
                ui.checkbox(push_after_create, self.tr("tag.push_after_create"));
                if *push_after_create {
                    tag_remote_selector(ui, self.language, &remotes, remote);
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let submit_requested = dialog_default_submit_requested(ui);
                    if (ui.button(self.tr("dialog.create")).clicked() || submit_requested)
                        && !name.trim().is_empty()
                    {
                        let tag_name = name.trim().to_owned();
                        let hash = hash.clone();
                        let push_after_create = *push_after_create;
                        let remote = remote.trim().to_owned();
                        execute = Some(Box::new(move |root| {
                            git::create_tag(root, &tag_name, &hash)?;
                            if push_after_create && !remote.is_empty() {
                                git::push_tag(root, &remote, &tag_name)?;
                            }
                            Ok(())
                        }));
                        close_after = true;
                    }
                    if ui.button(self.tr("dialog.cancel")).clicked() {
                        close_after = true;
                    }
                });
            }
            CommitActionDialog::ConfirmCheckout { hash, short_hash } => {
                ui.label(
                    RichText::new(format!(
                        "{} {short_hash}?",
                        self.tr("commit.checkout_confirm")
                    ))
                    .color(theme::text()),
                );
                ui.label(RichText::new(self.tr("commit.detached_warning")).color(theme::warning()));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let submit_requested = dialog_default_submit_requested(ui);
                    if ui.button(self.tr("dialog.checkout")).clicked() || submit_requested {
                        let hash = hash.clone();
                        execute = Some(Box::new(move |root| git::checkout_commit(root, &hash)));
                        close_after = true;
                    }
                    if ui.button(self.tr("dialog.cancel")).clicked() {
                        close_after = true;
                    }
                });
            }
            CommitActionDialog::ConfirmCherryPick { hash, short_hash } => {
                ui.label(RichText::new(self.tr("commit.confirm_cherry_pick")).color(theme::text()));
                ui.label(
                    RichText::new(short_hash.as_str())
                        .monospace()
                        .color(theme::muted()),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let submit_requested = dialog_default_submit_requested(ui);
                    if ui.button(self.tr("menu.cherry_pick")).clicked() || submit_requested {
                        let hash = hash.clone();
                        execute = Some(Box::new(move |root| git::cherry_pick_commit(root, &hash)));
                        close_after = true;
                    }
                    if ui.button(self.tr("dialog.cancel")).clicked() {
                        close_after = true;
                    }
                });
            }
            CommitActionDialog::ConfirmCherryPickBatch {
                hashes,
                short_hashes,
            } => {
                ui.label(
                    RichText::new(self.tr("commit.confirm_cherry_pick_batch")).color(theme::text()),
                );
                ui.label(
                    RichText::new(format!(
                        "{} {}",
                        hashes.len(),
                        self.tr("commit.cherry_pick_selected")
                    ))
                    .color(theme::muted()),
                );
                ui.label(
                    RichText::new(short_hashes.join(", "))
                        .monospace()
                        .color(theme::muted()),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let submit_requested = dialog_default_submit_requested(ui);
                    if ui.button(self.tr("menu.cherry_pick")).clicked() || submit_requested {
                        let hashes = hashes.clone();
                        execute = Some(Box::new(move |root| {
                            git::cherry_pick_commits(root, &hashes)
                        }));
                        close_after = true;
                    }
                    if ui.button(self.tr("dialog.cancel")).clicked() {
                        close_after = true;
                    }
                });
            }
            CommitActionDialog::ConfirmRevert { hash, short_hash } => {
                ui.label(RichText::new(self.tr("commit.confirm_revert")).color(theme::text()));
                ui.label(
                    RichText::new(short_hash.as_str())
                        .monospace()
                        .color(theme::muted()),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let submit_requested = dialog_default_submit_requested(ui);
                    if ui.button(self.tr("menu.revert")).clicked() || submit_requested {
                        let hash = hash.clone();
                        execute = Some(Box::new(move |root| git::revert_commit(root, &hash)));
                        close_after = true;
                    }
                    if ui.button(self.tr("dialog.cancel")).clicked() {
                        close_after = true;
                    }
                });
            }
            CommitActionDialog::ConfirmReset {
                hash,
                short_hash,
                mode,
            } => {
                ui.label(RichText::new(self.tr("commit.confirm_reset")).color(theme::warning()));
                ui.label(
                    RichText::new(short_hash.as_str())
                        .monospace()
                        .color(theme::muted()),
                );
                ui.add_space(8.0);
                ui.radio_value(mode, ResetMode::Soft, self.tr("reset.soft"));
                ui.radio_value(mode, ResetMode::Mixed, self.tr("reset.mixed"));
                ui.radio_value(mode, ResetMode::Hard, self.tr("reset.hard"));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let submit_requested = dialog_default_submit_requested(ui);
                    if ui.button(self.tr("menu.reset")).clicked() || submit_requested {
                        let hash = hash.clone();
                        let mode = *mode;
                        execute = Some(Box::new(move |root| {
                            git::reset_to_commit(root, &hash, mode)
                        }));
                        close_after = true;
                    }
                    if ui.button(self.tr("dialog.cancel")).clicked() {
                        close_after = true;
                    }
                });
            }
        });

        if let Some(action) = execute {
            self.execute_git_action(action);
            self.clear_cherry_pick_selection();
        }

        if close_after {
            keep_open = false;
        }
        if keep_open {
            self.pending_commit_action = Some(dialog);
        }
    }

    fn worktree_action_modal(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.pending_worktree_action.take() else {
            return;
        };

        let mut keep_open = true;
        let mut close_after = false;
        let mut execute: Option<Box<dyn FnOnce(&std::path::Path) -> anyhow::Result<()> + Send>> =
            None;

        match dialog {
            WorktreeActionDialog::ConfirmDiscard { path, untracked } => {
                compact_action_dialog(ctx, "Discard changes", ACTION_DIALOG_WIDTH, |ui| {
                    ui.label(RichText::new(&path).monospace().color(theme::text()));
                    ui.label(
                        RichText::new(if untracked {
                            "This will delete the untracked file or directory."
                        } else {
                            "This will restore the path from HEAD."
                        })
                        .color(theme::warning()),
                    );
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let submit_requested = dialog_default_submit_requested(ui);
                        if ui.button("Discard").clicked() || submit_requested {
                            let path = path.clone();
                            execute = Some(Box::new(move |root| {
                                if untracked {
                                    git::clean_untracked_path(root, &path)
                                } else {
                                    git::discard_path(root, &path)
                                }
                            }));
                            close_after = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_after = true;
                        }
                    });
                });

                if let Some(action) = execute {
                    self.execute_git_action(action);
                }
                if close_after {
                    keep_open = false;
                }
                if keep_open {
                    self.pending_worktree_action =
                        Some(WorktreeActionDialog::ConfirmDiscard { path, untracked });
                }
            }
            WorktreeActionDialog::ResolveConflicts { selected_path } => {
                let conflict_files = self
                    .snapshot
                    .as_ref()
                    .map(worktree_conflict_files)
                    .unwrap_or_default();
                let mut selected_path =
                    selected_path.or_else(|| conflict_files.first().map(|file| file.path.clone()));
                let mut accept_side = None;
                let mut merge_path = None;

                let modal_rect = conflict_resolution_modal_rect(ctx);
                egui::Area::new(egui::Id::new("conflict_resolution_modal"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(modal_rect.min)
                    .show(ctx, |ui| {
                        safe_set_min_size(ui, CONFLICT_MODAL_SIZE);
                        ui.set_max_size(CONFLICT_MODAL_SIZE);
                        soft_panel_frame(conflict_resolution_dialog_background(), 12, 12).show(
                            ui,
                            |ui| {
                                safe_set_min_size(ui, CONFLICT_MODAL_INNER_SIZE);
                                ui.set_max_size(CONFLICT_MODAL_INNER_SIZE);
                                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(CONFLICT_MODAL_INNER_SIZE.x, 32.0),
                                        Layout::right_to_left(Align::Center),
                                        |ui| {
                                            if window_control_button(ui, "\u{00d7}", true).clicked()
                                            {
                                                close_after = true;
                                            }
                                        },
                                    );
                                    ui.add_space(8.0);
                                    let content_height = ui.available_height();
                                    let panel_size =
                                        Vec2::new(CONFLICT_LIST_PANEL_SIZE.x, content_height);
                                    let action_panel_size =
                                        Vec2::new(CONFLICT_ACTION_PANEL_SIZE.x, content_height);
                                    ui.horizontal(|ui| {
                                        ui.allocate_ui_with_layout(
                                            panel_size,
                                            Layout::top_down(Align::Min),
                                            |ui| {
                                                conflict_resolution_list_panel(
                                                    ui,
                                                    panel_size,
                                                    &conflict_files,
                                                    self.language,
                                                    &mut selected_path,
                                                );
                                            },
                                        );
                                        ui.add_space(CONFLICT_MODAL_PANEL_GAP);
                                        ui.allocate_ui_with_layout(
                                            action_panel_size,
                                            Layout::top_down(Align::Min),
                                            |ui| {
                                                if let Some(action) =
                                                    conflict_resolution_actions_panel(
                                                        ui,
                                                        action_panel_size,
                                                        self.language,
                                                        selected_path.is_some(),
                                                    )
                                                {
                                                    match action {
                                                        ConflictResolutionDialogAction::Accept(
                                                            side,
                                                        ) => {
                                                            if let Some(path) =
                                                                selected_path.clone()
                                                            {
                                                                accept_side = Some((path, side));
                                                                close_after = true;
                                                            }
                                                        }
                                                        ConflictResolutionDialogAction::Merge => {
                                                            merge_path = selected_path.clone();
                                                            close_after = true;
                                                        }
                                                    }
                                                }
                                            },
                                        );
                                    });
                                });
                            },
                        );
                    });

                if let Some((path, side)) = accept_side {
                    self.execute_git_action(move |root| {
                        git::accept_conflict_side(root, &path, side)
                    });
                }
                if let Some(path) = merge_path {
                    self.open_conflict_merge_tool(&path);
                }
                if close_after {
                    keep_open = false;
                }
                if keep_open {
                    self.pending_worktree_action =
                        Some(WorktreeActionDialog::ResolveConflicts { selected_path });
                }
            }
        }
    }

    fn stash_action_modal(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.pending_stash_action.take() else {
            return;
        };

        let mut keep_open = true;
        let mut close_after = false;
        let mut execute: Option<Box<dyn FnOnce(&std::path::Path) -> anyhow::Result<()> + Send>> =
            None;

        match &mut dialog {
            StashActionDialog::Create { message } => {
                compact_action_dialog(ctx, self.tr("stash.create"), ACTION_DIALOG_WIDTH, |ui| {
                    let hint = self.tr("stash.message");
                    ui.add_sized(
                        [ui.available_width(), 34.0],
                        TextEdit::singleline(message).hint_text(hint),
                    );
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let submit_requested = dialog_default_submit_requested(ui);
                        if ui.button(self.tr("stash.create")).clicked() || submit_requested {
                            let message = message.trim().to_owned();
                            execute = Some(Box::new(move |root| git::stash_push(root, &message)));
                            close_after = true;
                        }
                        if ui.button(self.tr("dialog.cancel")).clicked() {
                            close_after = true;
                        }
                    });
                });
            }
            StashActionDialog::ConfirmDrop { selector, message } => {
                compact_action_dialog(ctx, self.tr("stash.drop"), ACTION_DIALOG_WIDTH, |ui| {
                    ui.label(RichText::new(self.tr("stash.confirm_drop")).color(theme::text()));
                    ui.label(RichText::new(message.as_str()).color(theme::muted()));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let submit_requested = dialog_default_submit_requested(ui);
                        if ui.button(self.tr("stash.drop")).clicked() || submit_requested {
                            let selector = selector.clone();
                            execute = Some(Box::new(move |root| git::stash_drop(root, &selector)));
                            close_after = true;
                        }
                        if ui.button(self.tr("dialog.cancel")).clicked() {
                            close_after = true;
                        }
                    });
                });
            }
        }

        if let Some(action) = execute {
            self.execute_git_action(action);
        }
        if close_after {
            keep_open = false;
        }
        if keep_open {
            self.pending_stash_action = Some(dialog);
        }
    }

    fn branch_action_modal(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.pending_branch_action.take() else {
            return;
        };

        let mut keep_open = true;
        let mut close_after = false;
        let mut execute: Option<Box<dyn FnOnce(&std::path::Path) -> anyhow::Result<()> + Send>> =
            None;
        let mut checkout_requested: Option<(String, bool)> = None;
        let branch_actions_enabled = !self.branch_actions_busy();

        match &mut dialog {
            BranchActionDialog::Create { name, checkout } => {
                compact_action_dialog(ctx, self.tr("branch.create"), ACTION_DIALOG_WIDTH, |ui| {
                    ui.label(
                        RichText::new(self.tr("branch.name"))
                            .small()
                            .color(theme::muted()),
                    );
                    ui.add(TextEdit::singleline(name));
                    ui.checkbox(checkout, self.tr("branch.checkout"));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let default_action_enabled =
                            branch_actions_enabled && !name.trim().is_empty();
                        let submit_requested = dialog_default_submit_requested(ui);
                        if ui
                            .add_enabled(
                                default_action_enabled,
                                egui::Button::new(self.tr("dialog.create")),
                            )
                            .clicked()
                            || (submit_requested && default_action_enabled)
                        {
                            let branch_name = name.trim().to_owned();
                            let checkout = *checkout;
                            execute = Some(Box::new(move |root| {
                                git::create_branch_from_head(root, &branch_name, checkout)
                            }));
                            close_after = true;
                        }
                        if ui.button(self.tr("dialog.cancel")).clicked() {
                            close_after = true;
                        }
                    });
                });
            }
            BranchActionDialog::ConfirmCheckout {
                name,
                discard_changes,
            } => {
                compact_action_dialog(
                    ctx,
                    self.tr("branch.confirm_checkout_title"),
                    ACTION_DIALOG_WIDTH,
                    |ui| {
                        ui.horizontal(|ui| {
                            let (icon_rect, _) =
                                ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                            paint_ui_icon(
                                ui,
                                icon_rect.shrink(2.0),
                                UiIcon::Warning,
                                theme::warning(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{} \"{}\"?",
                                    self.tr("branch.confirm_checkout"),
                                    name
                                ))
                                .color(theme::text()),
                            );
                        });
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.checkbox(discard_changes, self.tr("branch.discard_before_checkout"));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button(self.tr("dialog.cancel")).clicked() {
                                    close_after = true;
                                }
                                let submit_requested = dialog_default_submit_requested(ui);
                                if ui
                                    .add_enabled(
                                        branch_actions_enabled,
                                        egui::Button::new(self.tr("dialog.ok")),
                                    )
                                    .clicked()
                                    || (submit_requested && branch_actions_enabled)
                                {
                                    checkout_requested = Some((name.clone(), *discard_changes));
                                    close_after = true;
                                }
                            });
                        });
                    },
                );
            }
            BranchActionDialog::CheckoutRemote {
                remote_branch,
                local_branch,
            } => {
                compact_action_dialog(
                    ctx,
                    self.tr("branch.sync_remote"),
                    ACTION_DIALOG_WIDTH,
                    |ui| {
                        ui.label(RichText::new(remote_branch.as_str()).color(theme::text()));
                        ui.label(
                            RichText::new(self.tr("branch.local_alias"))
                                .small()
                                .color(theme::muted()),
                        );
                        ui.add(TextEdit::singleline(local_branch));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            let default_action_enabled =
                                branch_actions_enabled && !local_branch.trim().is_empty();
                            let submit_requested = dialog_default_submit_requested(ui);
                            if ui
                                .add_enabled(
                                    default_action_enabled,
                                    egui::Button::new(self.tr("dialog.checkout")),
                                )
                                .clicked()
                                || (submit_requested && default_action_enabled)
                            {
                                let remote_branch = remote_branch.clone();
                                let local_branch = local_branch.trim().to_owned();
                                execute = Some(Box::new(move |root| {
                                    git::checkout_remote_branch(root, &remote_branch, &local_branch)
                                }));
                                close_after = true;
                            }
                            if ui.button(self.tr("dialog.cancel")).clicked() {
                                close_after = true;
                            }
                        });
                    },
                );
            }
            BranchActionDialog::Rename { old_name, new_name } => {
                compact_action_dialog(
                    ctx,
                    self.tr("branch.rename_title"),
                    ACTION_DIALOG_WIDTH,
                    |ui| {
                        ui.label(RichText::new(old_name.as_str()).color(theme::text()));
                        ui.label(
                            RichText::new(self.tr("branch.new_name"))
                                .small()
                                .color(theme::muted()),
                        );
                        ui.add(TextEdit::singleline(new_name));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            let default_action_enabled =
                                branch_actions_enabled && !new_name.trim().is_empty();
                            let submit_requested = dialog_default_submit_requested(ui);
                            if ui
                                .add_enabled(
                                    default_action_enabled,
                                    egui::Button::new(self.tr("dialog.ok")),
                                )
                                .clicked()
                                || (submit_requested && default_action_enabled)
                            {
                                let old_name = old_name.clone();
                                let new_name = new_name.trim().to_owned();
                                execute = Some(Box::new(move |root| {
                                    git::rename_branch(root, &old_name, &new_name)
                                }));
                                close_after = true;
                            }
                            if ui.button(self.tr("dialog.cancel")).clicked() {
                                close_after = true;
                            }
                        });
                    },
                );
            }
            BranchActionDialog::ConfirmDelete { name, force } => {
                compact_action_dialog(ctx, self.tr("branch.delete"), ACTION_DIALOG_WIDTH, |ui| {
                    ui.label(RichText::new(self.tr("branch.confirm_delete")).color(theme::text()));
                    ui.label(RichText::new(name.as_str()).color(theme::warning()));
                    ui.checkbox(force, self.tr("branch.force_delete"));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let submit_requested = dialog_default_submit_requested(ui);
                        if ui
                            .add_enabled(
                                branch_actions_enabled,
                                egui::Button::new(self.tr("branch.delete")),
                            )
                            .clicked()
                            || (submit_requested && branch_actions_enabled)
                        {
                            let name = name.clone();
                            let force = *force;
                            execute =
                                Some(Box::new(move |root| git::delete_branch(root, &name, force)));
                            close_after = true;
                        }
                        if ui.button(self.tr("dialog.cancel")).clicked() {
                            close_after = true;
                        }
                    });
                });
            }
            BranchActionDialog::ConfirmDeleteRemote { remote_branch } => {
                compact_action_dialog(
                    ctx,
                    self.tr("branch.delete_remote"),
                    ACTION_DIALOG_WIDTH,
                    |ui| {
                        ui.label(
                            RichText::new(self.tr("branch.confirm_delete_remote"))
                                .color(theme::text()),
                        );
                        ui.label(RichText::new(remote_branch.as_str()).color(theme::warning()));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            let submit_requested = dialog_default_submit_requested(ui);
                            if ui
                                .add_enabled(
                                    branch_actions_enabled,
                                    egui::Button::new(self.tr("branch.delete_remote")),
                                )
                                .clicked()
                                || (submit_requested && branch_actions_enabled)
                            {
                                let remote_branch = remote_branch.clone();
                                execute = Some(Box::new(move |root| {
                                    git::delete_remote_branch(root, &remote_branch)
                                }));
                                close_after = true;
                            }
                            if ui.button(self.tr("dialog.cancel")).clicked() {
                                close_after = true;
                            }
                        });
                    },
                );
            }
        }

        if let Some((name, discard_changes)) = checkout_requested {
            self.start_branch_checkout(name, discard_changes);
        }
        if let Some(action) = execute {
            self.execute_git_action(action);
        }
        if close_after {
            keep_open = false;
        }
        if keep_open {
            self.pending_branch_action = Some(dialog);
        }
    }

    fn tag_action_modal(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.pending_tag_action.take() else {
            return;
        };

        let mut keep_open = true;
        let mut close_after = false;
        let mut execute: Option<Box<dyn FnOnce(&std::path::Path) -> anyhow::Result<()> + Send>> =
            None;
        let remotes = self.remote_names();

        match &mut dialog {
            TagActionDialog::Create {
                name,
                push_after_create,
                remote,
            } => {
                compact_action_dialog(ctx, self.tr("tag.create"), ACTION_DIALOG_WIDTH, |ui| {
                    ui.label(
                        RichText::new(self.tr("tag.name"))
                            .small()
                            .color(theme::muted()),
                    );
                    themed_text_edit_selection(ui);
                    ui.add(themed_singleline_text_edit(name, ""));
                    ui.checkbox(push_after_create, self.tr("tag.push_after_create"));
                    if *push_after_create {
                        tag_remote_selector(ui, self.language, &remotes, remote);
                    }
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let submit_requested = dialog_default_submit_requested(ui);
                        if (ui.button(self.tr("dialog.create")).clicked() || submit_requested)
                            && !name.trim().is_empty()
                        {
                            let name = name.trim().to_owned();
                            let push_after_create = *push_after_create;
                            let remote = remote.trim().to_owned();
                            execute = Some(Box::new(move |root| {
                                git::create_tag_at_head(root, &name)?;
                                if push_after_create && !remote.is_empty() {
                                    git::push_tag(root, &remote, &name)?;
                                }
                                Ok(())
                            }));
                            close_after = true;
                        }
                        if ui.button(self.tr("dialog.cancel")).clicked() {
                            close_after = true;
                        }
                    });
                });
            }
            TagActionDialog::Push { name, remote } => {
                compact_action_dialog(ctx, self.tr("tag.push"), ACTION_DIALOG_WIDTH, |ui| {
                    ui.label(RichText::new(name.as_str()).color(theme::text()));
                    ui.add_space(8.0);
                    tag_remote_selector(ui, self.language, &remotes, remote);
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let submit_requested = dialog_default_submit_requested(ui);
                        if (ui.button(self.tr("tag.push")).clicked() || submit_requested)
                            && !remote.trim().is_empty()
                        {
                            let name = name.clone();
                            let remote = remote.trim().to_owned();
                            execute =
                                Some(Box::new(move |root| git::push_tag(root, &remote, &name)));
                            close_after = true;
                        }
                        if ui.button(self.tr("dialog.cancel")).clicked() {
                            close_after = true;
                        }
                    });
                });
            }
            TagActionDialog::ConfirmDelete { name } => {
                compact_action_dialog(ctx, self.tr("tag.delete"), ACTION_DIALOG_WIDTH, |ui| {
                    ui.label(RichText::new(self.tr("tag.confirm_delete")).color(theme::text()));
                    ui.label(RichText::new(name.as_str()).color(theme::warning()));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let submit_requested = dialog_default_submit_requested(ui);
                        if ui.button(self.tr("tag.delete")).clicked() || submit_requested {
                            let name = name.clone();
                            execute = Some(Box::new(move |root| git::delete_tag(root, &name)));
                            close_after = true;
                        }
                        if ui.button(self.tr("dialog.cancel")).clicked() {
                            close_after = true;
                        }
                    });
                });
            }
        }

        if let Some(action) = execute {
            self.execute_git_action(action);
        }
        if close_after {
            keep_open = false;
        }
        if keep_open {
            self.pending_tag_action = Some(dialog);
        }
    }

    fn settings_modal(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }

        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.settings_open = false;
            return;
        }

        let mut close_requested = false;
        let screen = ctx.screen_rect();
        let size = Vec2::new(
            (screen.width() * 0.46).clamp(420.0, 620.0),
            (screen.height() * 0.46).clamp(300.0, 420.0),
        );
        let rect = Rect::from_min_size(
            Pos2::new(
                screen.center().x - size.x / 2.0,
                screen.center().y - size.y / 2.0,
            ),
            size,
        );

        egui::Window::new(self.tr("settings.title"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_rect(rect)
            .frame(dialog_window_frame())
            .show(ctx, |ui| {
                settings_dialog_panel_frame().show(ui, |ui| {
                    ui.set_width(size.x);
                    settings_dialog_title_row(ui, self.tr("options.title"), size.x, |ui| {
                        if window_control_button(ui, "\u{00d7}", true).clicked() {
                            close_requested = true;
                        }
                    });
                    settings_dialog_body_frame().show(ui, |ui| {
                        ui.set_width(size.x - 28.0);
                        let content_height = safe_ui_length(
                            size.y - SETTINGS_DIALOG_TITLE_HEIGHT - SETTINGS_FOOTER_HEIGHT - 64.0,
                        );
                        soft_panel_frame(theme::panel(), 16, 12).show(ui, |ui| {
                            safe_set_min_size(
                                ui,
                                frame_inner_size(size.x - 28.0, content_height, 16, 12),
                            );
                            self.global_settings_page(ui);
                        });
                        ui.add_space(10.0);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button(self.tr("dialog.ok")).clicked() {
                                close_requested = true;
                            }
                            if ui.button(self.tr("dialog.cancel")).clicked() {
                                close_requested = true;
                            }
                        });
                    });
                });
            });

        self.settings_open = self.settings_open && !close_requested;
    }

    fn repo_settings_modal(&mut self, ctx: &egui::Context) {
        if !self.repo_settings_open {
            return;
        }

        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.repo_settings_open = false;
            return;
        }

        let mut close_requested = false;
        let screen = ctx.screen_rect();
        let size = Vec2::new(
            (screen.width() * 0.52).clamp(520.0, REPO_SETTINGS_DIALOG_WIDTH),
            repo_settings_dialog_height(self.repo_settings_tab).min(screen.height() * 0.9),
        );
        let rect = Rect::from_min_size(
            Pos2::new(
                screen.center().x - size.x / 2.0,
                screen.center().y - size.y / 2.0,
            ),
            size,
        );

        egui::Window::new(self.tr("repo.settings.title"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_rect(rect)
            .frame(dialog_window_frame())
            .show(ctx, |ui| {
                settings_dialog_panel_frame().show(ui, |ui| {
                    ui.set_width(size.x);
                    settings_dialog_title_row(ui, self.tr("repo.settings.title"), size.x, |ui| {
                        if window_control_button(ui, "\u{00d7}", true).clicked() {
                            close_requested = true;
                        }
                    });
                    settings_dialog_body_frame().show(ui, |ui| {
                        ui.set_width(size.x - 28.0);
                        ui.add_space(6.0);
                        repo_settings_tab_strip(ui, &mut self.repo_settings_tab, self.language);
                        ui.add_space(12.0);
                        let content_width = repo_settings_content_width(size.x);
                        ScrollArea::vertical()
                            .id_salt("repo_settings_content_scroll")
                            .max_height(repo_settings_content_max_height(self.repo_settings_tab))
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.set_min_width(content_width);
                                ui.vertical(|ui| match self.repo_settings_tab {
                                    SettingsTab::RepoRemotes => self.repo_remotes_settings_page(ui),
                                    SettingsTab::RepoAdvanced => {
                                        self.repo_advanced_settings_page(ui)
                                    }
                                    SettingsTab::General => {}
                                });
                            });
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui
                                .button(self.tr("repo.settings.edit_config_file"))
                                .clicked()
                            {
                                self.open_repo_config_file();
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button(self.tr("dialog.ok")).clicked() {
                                    close_requested = true;
                                }
                                if ui.button(self.tr("dialog.cancel")).clicked() {
                                    close_requested = true;
                                }
                            });
                        });
                    });
                });
            });

        self.repo_settings_open = self.repo_settings_open && !close_requested;
    }

    fn global_settings_page(&mut self, ui: &mut Ui) {
        settings_section_title(ui, "Global Options");
        settings_field(ui, "Theme", |ui| {
            if settings_choice_button(
                ui,
                self.theme_mode == theme::ThemeMode::Light,
                menu_label(self.language, "light_theme"),
                88.0,
            )
            .clicked()
            {
                self.set_theme_mode(theme::ThemeMode::Light);
            }
            if settings_choice_button(
                ui,
                self.theme_mode == theme::ThemeMode::Dark,
                menu_label(self.language, "dark_theme"),
                88.0,
            )
            .clicked()
            {
                self.set_theme_mode(theme::ThemeMode::Dark);
            }
        });
        ui.add_space(8.0);
        settings_field(ui, settings_theme_accent_title(self.language), |ui| {
            for accent in theme::all_accents() {
                if settings_accent_button(
                    ui,
                    self.theme_accent == accent,
                    theme_accent_label(self.language, accent),
                    theme::accent_color(accent),
                )
                .clicked()
                {
                    self.set_theme_accent(accent);
                }
            }
        });
        ui.add_space(8.0);
        settings_field(ui, "Language", |ui| {
            if settings_choice_button(
                ui,
                self.language == Language::Chinese,
                "\u{4e2d}\u{6587}",
                72.0,
            )
            .clicked()
            {
                self.set_language(Language::Chinese);
            }
            if settings_choice_button(ui, self.language == Language::English, "English", 72.0)
                .clicked()
            {
                self.set_language(Language::English);
            }
        });
        ui.add_space(12.0);
        self.global_remote_accounts_settings(ui);
    }

    fn global_remote_accounts_settings(&mut self, ui: &mut Ui) {
        let language = self.language;
        settings_section_title(ui, i18n::t(language, "settings.remote_accounts"));
        let mut remove_account = None;
        for index in 0..self.remote_accounts.len() {
            let name = self.remote_accounts[index].name.clone();
            let host = self.remote_accounts[index].host.clone();
            ui.horizontal(|ui| {
                ui.add_sized(
                    [SETTINGS_REMOTE_ACCOUNT_INPUT_WIDTH, 22.0],
                    egui::Label::new(RichText::new(name).color(theme::text())),
                );
                ui.add_sized(
                    [SETTINGS_REMOTE_ACCOUNT_INPUT_WIDTH, 22.0],
                    egui::Label::new(RichText::new(host).color(theme::muted())),
                );
                if self.remote_accounts.len() > 1
                    && ui
                        .button(i18n::t(language, "repo.settings.remove"))
                        .clicked()
                {
                    remove_account = Some(index);
                }
            });
        }
        if let Some(index) = remove_account {
            self.remote_accounts.remove(index);
            self.save_app_settings();
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            themed_text_edit_selection(ui);
            ui.add_sized(
                [SETTINGS_REMOTE_ACCOUNT_INPUT_WIDTH, 24.0],
                themed_singleline_text_edit(
                    &mut self.remote_account_name_input,
                    i18n::t(language, "settings.remote_account_name"),
                ),
            );
            themed_text_edit_selection(ui);
            ui.add_sized(
                [SETTINGS_REMOTE_ACCOUNT_INPUT_WIDTH, 24.0],
                themed_singleline_text_edit(
                    &mut self.remote_account_host_input,
                    i18n::t(language, "settings.remote_account_host"),
                ),
            );
            if ui.button(i18n::t(language, "repo.settings.add")).clicked() {
                match validate_remote_account_settings(
                    &self.remote_account_name_input,
                    &self.remote_account_host_input,
                ) {
                    Ok(()) => {
                        self.remote_accounts.push(RemoteAccountSettings {
                            name: self.remote_account_name_input.trim().to_owned(),
                            host: self.remote_account_host_input.trim().to_owned(),
                        });
                        self.remote_account_name_input.clear();
                        self.remote_account_host_input.clear();
                        self.remote_account_error = None;
                        self.save_app_settings();
                    }
                    Err(error) => {
                        self.remote_account_error = Some(error);
                    }
                }
            }
        });
        if let Some(error) = &self.remote_account_error {
            ui.label(
                RichText::new(format!(
                    "{}: {error}",
                    i18n::t(language, "repo.settings.account_validation_failed")
                ))
                .small()
                .color(theme::warning()),
            );
        }
    }

    fn repo_remotes_settings_page(&mut self, ui: &mut Ui) {
        let remotes = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.remotes.clone())
            .unwrap_or_default();
        let selected_remote = remotes.first().cloned();
        let language = self.language;

        repo_settings_card(ui, self.tr("repo.settings.remote_paths"), |ui| {
            remote_settings_table(ui, language, &remotes);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(i18n::t(language, "repo.settings.add")).clicked() {
                    self.begin_add_remote_settings();
                }
                if ui
                    .add_enabled(
                        selected_remote.is_some(),
                        egui::Button::new(i18n::t(language, "repo.settings.edit")),
                    )
                    .clicked()
                {
                    if let Some(remote) = selected_remote.as_ref() {
                        self.begin_edit_remote_settings(remote);
                    }
                }
                ui.add_enabled(
                    false,
                    egui::Button::new(i18n::t(language, "repo.settings.remove")),
                );
            });
        });
    }

    fn repo_advanced_settings_page(&mut self, ui: &mut Ui) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let config = snapshot.config.clone();
        let language = self.language;

        repo_settings_card(ui, self.tr("repo.settings.ignore_list"), |ui| {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [(ui.available_width() - 76.0).max(80.0), 24.0],
                    egui::Label::new(
                        RichText::new(config.gitignore_path.display().to_string())
                            .monospace()
                            .color(theme::text()),
                    )
                    .truncate(),
                );
                ui.add_enabled(
                    false,
                    egui::Button::new(i18n::t(language, "repo.settings.edit")),
                );
            });
        });
        ui.add_space(LAYOUT_GAP as f32);
        repo_settings_card(ui, self.tr("repo.settings.user"), |ui| {
            let mut use_global = config.uses_global_user;
            settings_checkbox_row(
                ui,
                &mut use_global,
                i18n::t(language, "repo.settings.use_global_user"),
            );
            repo_settings_readonly_text(
                ui,
                i18n::t(language, "repo.settings.full_name"),
                &config.user_name,
            );
            repo_settings_readonly_text(
                ui,
                i18n::t(language, "repo.settings.email"),
                &config.user_email,
            );
        });
        ui.add_space(LAYOUT_GAP as f32);
        repo_settings_card(ui, self.tr("repo.settings.commit_links"), |ui| {
            repo_settings_commit_links_panel(ui, language);
        });
        ui.add_space(LAYOUT_GAP as f32);
        repo_settings_card(ui, self.tr("repo.settings.options"), |ui| {
            let mut auto_refresh = true;
            let mut refresh_remote = true;
            settings_checkbox_row(
                ui,
                &mut auto_refresh,
                i18n::t(language, "repo.settings.auto_refresh"),
            );
            settings_checkbox_row(
                ui,
                &mut refresh_remote,
                i18n::t(language, "repo.settings.background_remote_refresh"),
            );
        });
    }

    fn repo_remote_action_modal(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.pending_repo_remote_action.take() else {
            return;
        };

        let language = self.language;
        let accounts = normalized_remote_accounts(&self.remote_accounts);
        let mut keep_open = true;
        let mut close_after = false;

        let title = match &dialog {
            RepoRemoteActionDialog::Add { .. } => i18n::t(language, "repo.settings.add_remote"),
            RepoRemoteActionDialog::Edit { .. } => i18n::t(language, "repo.settings.edit_remote"),
        };

        compact_action_dialog(ctx, title, REPO_SETTINGS_REMOTE_DIALOG_WIDTH, |ui| {
            let (name, url, account_index, validation_error) = match &mut dialog {
                RepoRemoteActionDialog::Add {
                    name,
                    url,
                    account_index,
                    validation_error,
                }
                | RepoRemoteActionDialog::Edit {
                    name,
                    url,
                    account_index,
                    validation_error,
                    ..
                } => (name, url, account_index, validation_error),
            };
            repo_settings_editable_text(ui, i18n::t(language, "repo.settings.remote_name"), name);
            repo_settings_editable_text(ui, i18n::t(language, "repo.settings.url_path"), url);
            repo_settings_account_dropdown(
                ui,
                language,
                &accounts,
                account_index,
                "repo_remote_action_account",
            );
            if let Some(error) = validation_error.as_ref() {
                ui.label(
                    RichText::new(format!(
                        "{}: {error}",
                        i18n::t(language, "repo.settings.remote_validation_failed")
                    ))
                    .small()
                    .color(theme::warning()),
                );
            }
            ui.add_space(10.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let submit_requested = dialog_default_submit_requested(ui);
                if ui.button(i18n::t(language, "dialog.ok")).clicked() || submit_requested {
                    match validate_repo_remote_action_dialog(name, url) {
                        Ok(()) => {
                            close_after = true;
                        }
                        Err(error) => {
                            *validation_error = Some(error);
                        }
                    }
                }
                if ui.button(i18n::t(language, "dialog.cancel")).clicked() {
                    close_after = true;
                }
            });
        });

        if close_after {
            keep_open = false;
        }
        if keep_open {
            self.pending_repo_remote_action = Some(dialog);
        }
    }
}

fn remote_settings_table(ui: &mut Ui, language: Language, remotes: &[git::Remote]) {
    let width = ui.available_width();
    let name_width = (width * 0.26).clamp(110.0, 180.0);
    let header_rect = repo_settings_table_row_rect(ui, 24.0, Sense::hover());
    ui.painter().rect_filled(
        header_rect,
        CornerRadius::same(4),
        theme::accent_soft().gamma_multiply(0.65),
    );
    repo_settings_table_text(
        ui,
        header_rect,
        header_rect.left() + 12.0,
        i18n::t(language, "repo.settings.name"),
        false,
        true,
    );
    repo_settings_table_text(
        ui,
        header_rect,
        header_rect.left() + name_width,
        i18n::t(language, "repo.settings.path"),
        false,
        true,
    );

    if remotes.is_empty() {
        let rect = repo_settings_table_row_rect(ui, 34.0, Sense::hover());
        repo_settings_table_text(
            ui,
            rect,
            rect.left() + 12.0,
            i18n::t(language, "remote.none"),
            false,
            false,
        );
        return;
    }

    for (index, remote) in remotes.iter().enumerate() {
        let selected = index == 0;
        let rect = repo_settings_table_row_rect(ui, 32.0, Sense::click());
        let fill = if selected {
            theme::accent_soft()
        } else if row_rect_hovered(ui, rect) {
            theme::hover()
        } else {
            Color32::TRANSPARENT
        };
        if fill != Color32::TRANSPARENT {
            ui.painter().rect_filled(
                rect.shrink2(Vec2::new(0.0, 1.0)),
                CornerRadius::same(4),
                fill,
            );
        }
        repo_settings_table_text(ui, rect, rect.left() + 12.0, &remote.name, false, false);
        repo_settings_table_text(
            ui,
            rect,
            rect.left() + name_width,
            remote_display_url(remote),
            true,
            false,
        );
    }
}

fn repo_settings_table_row_rect(ui: &mut Ui, height: f32, sense: Sense) -> Rect {
    ui.allocate_exact_size(Vec2::new(ui.available_width(), height), sense)
        .0
}

fn repo_settings_table_text(
    ui: &Ui,
    rect: Rect,
    x: f32,
    text: &str,
    monospace: bool,
    strong: bool,
) {
    ui.painter().text(
        Pos2::new(x, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        if monospace {
            FontId::monospace(12.0)
        } else {
            FontId::proportional(if strong { 12.5 } else { 12.0 })
        },
        if strong {
            theme::text()
        } else {
            theme::muted()
        },
    );
}

fn remote_display_url(remote: &git::Remote) -> &str {
    if remote.fetch_url.is_empty() {
        &remote.push_url
    } else {
        &remote.fetch_url
    }
}

fn repo_settings_account_dropdown(
    ui: &mut Ui,
    language: Language,
    accounts: &[RemoteAccountSettings],
    selected_index: &mut usize,
    id_salt: &'static str,
) {
    let accounts = normalized_remote_accounts(accounts);
    if *selected_index >= accounts.len() {
        *selected_index = 0;
    }
    ui.horizontal(|ui| {
        ui.set_min_height(30.0);
        ui.add_sized(
            [112.0, 22.0],
            egui::Label::new(
                RichText::new(i18n::t(language, "repo.settings.remote_account"))
                    .color(theme::muted()),
            ),
        );
        let selected = &accounts[*selected_index];
        egui::ComboBox::from_id_salt(id_salt)
            .width(ui.available_width().max(180.0))
            .selected_text(format!("{}  {}", selected.name, selected.host))
            .show_ui(ui, |ui| {
                for (index, account) in accounts.iter().enumerate() {
                    ui.selectable_value(
                        selected_index,
                        index,
                        format!("{}  {}", account.name, account.host),
                    );
                }
            });
    });
}

fn repo_settings_editable_text(ui: &mut Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.set_min_height(30.0);
        ui.add_sized(
            [112.0, 22.0],
            egui::Label::new(RichText::new(label).color(theme::muted())),
        );
        themed_text_edit_selection(ui);
        ui.add_sized(
            [ui.available_width().max(80.0), 24.0],
            themed_singleline_text_edit(value, ""),
        );
    });
}

fn repo_settings_readonly_text(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.set_min_height(30.0);
        ui.add_sized(
            [112.0, 22.0],
            egui::Label::new(RichText::new(label).color(theme::muted())),
        );
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), theme::panel());
        ui.painter().text(
            Pos2::new(rect.left() + 8.0, rect.center().y),
            Align2::LEFT_CENTER,
            value,
            FontId::proportional(12.0),
            theme::text(),
        );
    });
}

fn repo_settings_commit_links_panel(ui: &mut Ui, language: Language) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 78.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(5), theme::panel());
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.add_enabled(
                    false,
                    egui::Button::new(i18n::t(language, "repo.settings.add")),
                );
                ui.add_enabled(
                    false,
                    egui::Button::new(i18n::t(language, "repo.settings.edit")),
                );
                ui.add_enabled(
                    false,
                    egui::Button::new(i18n::t(language, "repo.settings.remove")),
                );
            });
        });
    });
}

#[derive(Clone, Debug)]
enum WorktreeMenuAction {
    Stage { path: String },
    StageAll,
    Unstage { path: String },
    UnstageAll,
    Discard { path: String, untracked: bool },
    ResolveConflict { path: String },
    AddToGitIgnore { pattern: String },
}

#[derive(Clone, Debug)]
enum StashMenuAction {
    Create,
    Apply { selector: String },
    Pop { selector: String },
    Drop { selector: String, message: String },
}

#[derive(Clone, Debug)]
enum BranchMenuAction {
    Create,
    Checkout {
        name: String,
    },
    CheckoutRemote {
        remote_branch: String,
    },
    MergeIntoCurrent {
        name: String,
    },
    RebaseCurrentOnto {
        name: String,
    },
    FetchTracked {
        remote_branch: String,
    },
    PullTracked {
        name: String,
    },
    PushTracked {
        name: String,
    },
    PushToRemote {
        name: String,
        remote: String,
    },
    TrackRemote {
        name: String,
        remote_branch: Option<String>,
    },
    CompareWithCurrent {
        name: String,
    },
    Rename {
        name: String,
    },
    Delete {
        name: String,
    },
    DeleteRemote {
        remote_branch: String,
    },
    CreatePullRequest {
        name: String,
    },
}

#[derive(Clone, Debug)]
enum TagMenuAction {
    Create,
    Checkout { name: String },
    Push { name: String },
    Delete { name: String },
}

fn tag_remote_selector(ui: &mut Ui, language: Language, remotes: &[String], remote: &mut String) {
    ui.label(
        RichText::new(i18n::t(language, "tag.remote"))
            .small()
            .color(theme::muted()),
    );
    if remotes.is_empty() {
        themed_text_edit_selection(ui);
        ui.add(themed_singleline_text_edit(remote, "origin"));
        return;
    }

    if !remotes.iter().any(|name| name == remote) {
        *remote = remotes[0].clone();
    }
    egui::ComboBox::from_id_salt("tag_remote_selector")
        .selected_text(remote.as_str())
        .show_ui(ui, |ui| {
            for name in remotes {
                ui.selectable_value(remote, name.clone(), name);
            }
        });
}

fn panel_heading(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.label(RichText::new(text).strong().color(theme::text()));
    });
}

fn panel_heading_inline(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).strong().color(theme::text()));
}

fn content_panel_frame(fill: Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(6))
        .shadow(panel_shadow())
        .inner_margin(egui::Margin::symmetric(
            CONTENT_PANEL_INSET_X,
            CONTENT_PANEL_INSET_Y,
        ))
}

fn exact_panel_at_rect(
    ui: &mut Ui,
    rect: Rect,
    fill: Color32,
    x_margin: i8,
    y_margin: i8,
    add_contents: impl FnOnce(&mut Ui),
) {
    let inner_rect = Rect::from_min_max(
        Pos2::new(rect.left() + x_margin as f32, rect.top() + y_margin as f32),
        Pos2::new(
            (rect.right() - x_margin as f32).max(rect.left()),
            (rect.bottom() - y_margin as f32).max(rect.top()),
        ),
    );
    let frame = egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(6))
        .shadow(panel_shadow())
        .inner_margin(egui::Margin::symmetric(x_margin, y_margin));

    ui.painter().add(frame.paint(inner_rect));
    allocate_clipped_ui_at_rect(ui, inner_rect, |ui| {
        safe_set_min_size(ui, inner_rect.size());
        add_contents(ui);
    });
}

fn allocate_clipped_ui_at_rect(ui: &mut Ui, rect: Rect, add_contents: impl FnOnce(&mut Ui)) {
    let clip_rect = rect.intersect(ui.clip_rect());
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.set_clip_rect(clip_rect);
        add_contents(ui);
    });
}

fn panel_shadow() -> egui::epaint::Shadow {
    egui::epaint::Shadow {
        offset: [3, 4],
        blur: 9,
        spread: 0,
        color: theme::accent_shadow(),
    }
}

fn workspace_card_frame(x: i8, y: i8) -> egui::Frame {
    egui::Frame::new()
        .fill(theme::panel_recessed())
        .corner_radius(CornerRadius::same(WORKSPACE_CARD_RADIUS))
        .inner_margin(egui::Margin::symmetric(x, y))
}

fn workspace_card_clip_rect(rect: Rect) -> Rect {
    rect.expand(WORKSPACE_CARD_SHADOW_PAD)
}

fn workspace_card_shadow_dark() -> Color32 {
    theme::inset_shadow()
}

fn workspace_card_shadow_light() -> Color32 {
    theme::inset_highlight()
}

const WORKSPACE_INSET_DARK_ALPHAS: [f32; 7] = [0.16, 0.12, 0.085, 0.058, 0.038, 0.024, 0.014];
const WORKSPACE_INSET_LIGHT_ALPHAS: [f32; 5] = [0.42, 0.27, 0.16, 0.09, 0.045];
const WORKSPACE_INSET_LAYER_STEP: f32 = 0.78;

#[derive(Clone, Copy)]
enum WorkspaceInsetEdge {
    Top,
    Left,
    Bottom,
    Right,
}

#[derive(Clone, Copy)]
enum WorkspaceInsetCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

fn workspace_inset_rect(rect: Rect, layer: usize) -> Rect {
    rect.shrink(0.65 + layer as f32 * WORKSPACE_INSET_LAYER_STEP)
}

fn workspace_inset_corner_radius(rect: Rect, layer: usize) -> f32 {
    let max_radius = (rect.width() * 0.5)
        .max(0.0)
        .min((rect.height() * 0.5).max(0.0));
    let desired_radius = (WORKSPACE_CARD_RADIUS as f32 - layer as f32 * 0.18).max(3.5);

    desired_radius.min(max_radius)
}

fn workspace_inset_web_shadow_stroke(layer: usize) -> f32 {
    (1.45 - layer as f32 * 0.06).max(0.9)
}

fn workspace_inset_edge_segment(
    rect: Rect,
    edge: WorkspaceInsetEdge,
    layer: usize,
) -> (Pos2, Pos2) {
    let r = workspace_inset_rect(rect, layer);
    let radius = workspace_inset_corner_radius(r, layer);

    match edge {
        WorkspaceInsetEdge::Top => (
            Pos2::new(r.left() + radius, r.top()),
            Pos2::new(r.right() - radius, r.top()),
        ),
        WorkspaceInsetEdge::Left => (
            Pos2::new(r.left(), r.top() + radius),
            Pos2::new(r.left(), r.bottom() - radius),
        ),
        WorkspaceInsetEdge::Bottom => (
            Pos2::new(r.left() + radius, r.bottom()),
            Pos2::new(r.right() - radius, r.bottom()),
        ),
        WorkspaceInsetEdge::Right => (
            Pos2::new(r.right(), r.top() + radius),
            Pos2::new(r.right(), r.bottom() - radius),
        ),
    }
}

fn workspace_inset_arc_points(rect: Rect, corner: WorkspaceInsetCorner, layer: usize) -> Vec<Pos2> {
    let r = workspace_inset_rect(rect, layer);
    let radius = workspace_inset_corner_radius(r, layer);
    let (center, start_angle, end_angle) = match corner {
        WorkspaceInsetCorner::TopLeft => (
            Pos2::new(r.left() + radius, r.top() + radius),
            std::f32::consts::PI,
            std::f32::consts::PI * 1.5,
        ),
        WorkspaceInsetCorner::TopRight => (
            Pos2::new(r.right() - radius, r.top() + radius),
            std::f32::consts::PI * 1.5,
            std::f32::consts::PI * 2.0,
        ),
        WorkspaceInsetCorner::BottomRight => (
            Pos2::new(r.right() - radius, r.bottom() - radius),
            0.0,
            std::f32::consts::PI * 0.5,
        ),
        WorkspaceInsetCorner::BottomLeft => (
            Pos2::new(r.left() + radius, r.bottom() - radius),
            std::f32::consts::PI * 0.5,
            std::f32::consts::PI,
        ),
    };

    (0..=6)
        .map(|index| {
            let t = index as f32 / 6.0;
            let angle = start_angle + (end_angle - start_angle) * t;
            Pos2::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            )
        })
        .collect()
}

fn paint_workspace_inset_edge(
    ui: &Ui,
    rect: Rect,
    edge: WorkspaceInsetEdge,
    layer: usize,
    color: Color32,
    alpha: f32,
) {
    let (start, end) = workspace_inset_edge_segment(rect, edge, layer);
    ui.painter().line_segment(
        [start, end],
        Stroke::new(
            workspace_inset_web_shadow_stroke(layer),
            color.gamma_multiply(alpha),
        ),
    );
}

fn paint_workspace_inset_arc(
    ui: &Ui,
    rect: Rect,
    corner: WorkspaceInsetCorner,
    layer: usize,
    color: Color32,
    alpha: f32,
) {
    ui.painter().add(Shape::line(
        workspace_inset_arc_points(rect, corner, layer),
        Stroke::new(
            workspace_inset_web_shadow_stroke(layer),
            color.gamma_multiply(alpha),
        ),
    ));
}

fn paint_workspace_card_inset_shadow(ui: &Ui, rect: Rect) {
    let dark = workspace_card_shadow_dark();
    let light = workspace_card_shadow_light();

    for (layer, alpha) in WORKSPACE_INSET_DARK_ALPHAS.iter().copied().enumerate() {
        paint_workspace_inset_edge(ui, rect, WorkspaceInsetEdge::Top, layer, dark, alpha);
        paint_workspace_inset_edge(ui, rect, WorkspaceInsetEdge::Left, layer, dark, alpha);
        paint_workspace_inset_arc(ui, rect, WorkspaceInsetCorner::TopLeft, layer, dark, alpha);
        paint_workspace_inset_arc(
            ui,
            rect,
            WorkspaceInsetCorner::BottomLeft,
            layer,
            dark,
            alpha,
        );
    }

    for (layer, alpha) in WORKSPACE_INSET_LIGHT_ALPHAS.iter().copied().enumerate() {
        paint_workspace_inset_edge(ui, rect, WorkspaceInsetEdge::Bottom, layer, light, alpha);
        paint_workspace_inset_edge(ui, rect, WorkspaceInsetEdge::Right, layer, light, alpha);
        paint_workspace_inset_arc(
            ui,
            rect,
            WorkspaceInsetCorner::TopRight,
            layer,
            light,
            alpha,
        );
        paint_workspace_inset_arc(
            ui,
            rect,
            WorkspaceInsetCorner::BottomRight,
            layer,
            light,
            alpha,
        );
    }
}

fn workspace_main_layout(
    body_rect: Rect,
    workspace_list_pct: f32,
    workspace_staged_pct: f32,
) -> WorkspaceMainLayout {
    let body_rect = Rect::from_min_max(
        Pos2::new(body_rect.left().round(), body_rect.top().round()),
        Pos2::new(body_rect.right().round(), body_rect.bottom().round()),
    );
    let available_height = body_rect.height().max(0.0);
    let list_commit_gap = WORKSPACE_LIST_COMMIT_GAP.min(available_height);
    let usable_height = (available_height - list_commit_gap).max(0.0);
    let desired_list_height = (available_height * workspace_list_pct)
        .round()
        .clamp(0.0, usable_height);
    let list_height = if usable_height >= 480.0 {
        desired_list_height.clamp(220.0, usable_height - 260.0)
    } else {
        desired_list_height
    };
    let list_rect = Rect::from_min_size(
        body_rect.left_top(),
        Vec2::new(body_rect.width(), list_height),
    );
    let table_gap = (LAYOUT_GAP as f32).min(list_rect.height());
    let table_total = (list_rect.height() - table_gap).max(0.0);
    let desired_staged_height = (table_total * workspace_staged_pct)
        .round()
        .clamp(0.0, table_total);
    let staged_height = if table_total >= 172.0 {
        desired_staged_height.clamp(86.0, table_total - 86.0)
    } else {
        desired_staged_height
    };
    let staged_rect = Rect::from_min_size(
        list_rect.left_top(),
        Vec2::new(list_rect.width(), staged_height),
    );
    let staged_unstaged_splitter_rect = Rect::from_min_size(
        Pos2::new(list_rect.left(), staged_rect.bottom()),
        Vec2::new(list_rect.width(), table_gap),
    );
    let unstaged_rect = Rect::from_min_max(
        Pos2::new(list_rect.left(), staged_unstaged_splitter_rect.bottom()),
        list_rect.right_bottom(),
    );
    let list_commit_splitter_rect = Rect::from_min_size(
        Pos2::new(body_rect.left(), list_rect.bottom()),
        Vec2::new(body_rect.width(), list_commit_gap),
    );
    let commit_rect = Rect::from_min_max(
        Pos2::new(body_rect.left(), list_commit_splitter_rect.bottom()),
        body_rect.right_bottom(),
    );

    WorkspaceMainLayout {
        staged_rect,
        staged_unstaged_splitter_rect,
        unstaged_rect,
        list_commit_splitter_rect,
        commit_rect,
    }
}

fn history_table_splitter(ui: &mut Ui, width: f32) -> Option<f32> {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, LAYOUT_GAP as f32), Sense::click_and_drag());
    let response = response.on_hover_cursor(egui::CursorIcon::ResizeVertical);
    let color = theme::accent_shadow();
    for i in 0..3 {
        let alpha = if response.hovered() || response.dragged() {
            54_u8.saturating_sub(i * 12)
        } else {
            34_u8.saturating_sub(i * 8)
        };
        let y = rect.top() + 1.0 + i as f32;
        ui.painter().line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha),
            ),
        );
    }
    response
        .dragged()
        .then(|| ui.input(|input| input.pointer.delta().y))
        .filter(|delta| delta.abs() > 0.0)
}

fn soft_panel_frame(fill: Color32, x: i8, y: i8) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(6))
        .shadow(panel_shadow())
        .inner_margin(egui::Margin::symmetric(x, y))
}

fn dialog_window_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
        .inner_margin(egui::Margin::same(0))
}

fn dialog_default_submit_requested(ui: &mut Ui) -> bool {
    ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
}

fn compact_action_dialog(
    ctx: &egui::Context,
    title: &str,
    width: f32,
    add_contents: impl FnOnce(&mut Ui),
) {
    let width = safe_ui_length(width).max(280.0);
    egui::Window::new(title)
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .frame(dialog_window_frame())
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme::panel())
                .corner_radius(CornerRadius::same(6))
                .shadow(panel_shadow())
                .inner_margin(egui::Margin::same(0))
                .show(ui, |ui| {
                    ui.set_width(width);
                    compact_dialog_title_bar(ui, title, width);
                    egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(12, 10))
                        .show(ui, |ui| {
                            ui.set_width((width - 24.0).max(0.0));
                            add_contents(ui);
                        });
                });
        });
}

fn compact_dialog_title_bar(ui: &mut Ui, title: &str, width: f32) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(width, ACTION_DIALOG_TITLE_HEIGHT), Sense::hover());
    ui.painter().rect_filled(
        rect,
        CornerRadius {
            nw: 6,
            ne: 6,
            sw: 0,
            se: 0,
        },
        theme::panel_soft(),
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        title,
        FontId::proportional(ACTION_DIALOG_TITLE_SIZE),
        theme::text(),
    );
}

fn settings_dialog_panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::panel())
        .corner_radius(CornerRadius::same(7))
        .shadow(panel_shadow())
        .inner_margin(egui::Margin::same(0))
}

fn settings_dialog_body_frame() -> egui::Frame {
    egui::Frame::new().inner_margin(egui::Margin::symmetric(14, 12))
}

fn frame_inner_size(width: f32, height: f32, x_margin: i8, y_margin: i8) -> Vec2 {
    Vec2::new(
        safe_ui_length(width - f32::from(x_margin) * 2.0),
        safe_ui_length(height - f32::from(y_margin) * 2.0),
    )
}

fn safe_ui_size(size: Vec2) -> Vec2 {
    Vec2::new(safe_ui_length(size.x), safe_ui_length(size.y))
}

fn safe_ui_length(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn safe_set_min_size(ui: &mut Ui, size: Vec2) {
    ui.set_min_size(safe_ui_size(size));
}

fn vertical_resize_delta(ui: &mut Ui, rect: Rect, id: &'static str) -> Option<f32> {
    let handle = resize_handle_rect(rect, true);
    let response = ui
        .interact(handle, ui.id().with(id), Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    response
        .dragged()
        .then(|| ui.input(|input| input.pointer.delta().x))
        .filter(|delta| delta.abs() > 0.0)
}

fn horizontal_resize_delta(ui: &mut Ui, rect: Rect, id: &'static str) -> Option<f32> {
    let handle = resize_handle_rect(rect, false);
    let response = ui
        .interact(handle, ui.id().with(id), Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::ResizeVertical);
    response
        .dragged()
        .then(|| ui.input(|input| input.pointer.delta().y))
        .filter(|delta| delta.abs() > 0.0)
}

fn pointing_hand_cursor(response: egui::Response) -> egui::Response {
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn row_rect_hovered(ui: &Ui, rect: Rect) -> bool {
    ui.input(|input| {
        input
            .pointer
            .hover_pos()
            .is_some_and(|pos| rect.contains(pos))
    })
}

fn full_row_click_response(
    ui: &mut Ui,
    rect: Rect,
    id_salt: impl std::hash::Hash,
) -> egui::Response {
    full_row_click_response_enabled(ui, rect, id_salt, true)
}

fn full_row_click_response_enabled(
    ui: &mut Ui,
    rect: Rect,
    id_salt: impl std::hash::Hash,
    enabled: bool,
) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, ui.id().with(id_salt), sense);
    if enabled {
        pointing_hand_cursor(response)
    } else {
        response
    }
}

fn resize_handle_rect(rect: Rect, vertical: bool) -> Rect {
    if vertical {
        Rect::from_center_size(
            rect.center(),
            Vec2::new(RESIZE_HANDLE_THICKNESS, rect.height()),
        )
    } else {
        Rect::from_center_size(
            rect.center(),
            Vec2::new(rect.width(), RESIZE_HANDLE_THICKNESS),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UiIcon {
    Commit,
    Pull,
    Push,
    Fetch,
    Branch,
    Tag,
    Stash,
    Folder,
    Refresh,
    Settings,
    Plus,
    Edit,
    AddFile,
    DeleteFile,
    RenameFile,
    File,
    Workspace,
    History,
    Search,
    Globe,
    Terminal,
    ResourceManager,
    CherryPick,
    Warning,
    More,
    Loading,
}

#[derive(Clone, Copy)]
enum AppButtonStyle {
    IconOnly,
    Toolbar,
    RepoTab { selected: bool },
}

struct AppButton<'a> {
    icon: UiIcon,
    label: &'a str,
    enabled: bool,
    style: AppButtonStyle,
}

impl<'a> AppButton<'a> {
    fn icon_only(icon: UiIcon, label: &'a str, enabled: bool) -> Self {
        Self {
            icon,
            label,
            enabled,
            style: AppButtonStyle::IconOnly,
        }
    }

    fn toolbar(icon: UiIcon, label: &'a str, enabled: bool) -> Self {
        Self {
            icon,
            label,
            enabled,
            style: AppButtonStyle::Toolbar,
        }
    }

    fn repo_tab(icon: UiIcon, label: &'a str, selected: bool) -> Self {
        Self {
            icon,
            label,
            enabled: true,
            style: AppButtonStyle::RepoTab { selected },
        }
    }

    fn show(self, ui: &mut Ui) -> egui::Response {
        let (icon_size, text_size, min_width, max_width, height, fill, radius) = match self.style {
            AppButtonStyle::IconOnly => {
                (11.0, 0.0, 18.0, 18.0, 18.0, toolbar_button_normal_fill(), 4)
            }
            AppButtonStyle::Toolbar => (
                TOOLBAR_BUTTON_ICON,
                TOOLBAR_BUTTON_TEXT,
                TOOLBAR_BUTTON_MIN_WIDTH,
                TOOLBAR_BUTTON_MAX_WIDTH,
                TOOLBAR_BUTTON_HEIGHT,
                toolbar_button_normal_fill(),
                4,
            ),
            AppButtonStyle::RepoTab { selected } => (
                14.0,
                12.0,
                if self.icon == UiIcon::More {
                    REPO_TAB_OVERFLOW_WIDTH
                } else {
                    110.0
                },
                if self.icon == UiIcon::More {
                    REPO_TAB_OVERFLOW_WIDTH
                } else {
                    220.0
                },
                28.0,
                repo_tab_fill(selected, false),
                3,
            ),
        };

        let tint = if self.enabled {
            if self.icon == UiIcon::Warning {
                Color32::from_rgb(232, 174, 55)
            } else {
                theme::text()
            }
        } else {
            theme::muted()
        };
        let image = egui::Image::new(icon_source(self.icon))
            .fit_to_exact_size(Vec2::splat(icon_size))
            .tint(match self.style {
                AppButtonStyle::RepoTab { selected } => repo_tab_icon_color(selected),
                _ => tint,
            });

        let button = match self.style {
            AppButtonStyle::IconOnly => egui::Button::image(image).min_size(Vec2::splat(height)),
            AppButtonStyle::Toolbar => egui::Button::image_and_text(
                image,
                RichText::new(self.label).size(text_size).color(tint),
            )
            .min_size(Vec2::new(
                inline_button_width(
                    ui,
                    self.label,
                    text_size,
                    min_width,
                    max_width,
                    TOOLBAR_BUTTON_X_PADDING,
                ),
                height,
            )),
            AppButtonStyle::RepoTab { .. } => egui::Button::image_and_text(
                image,
                RichText::new(self.label)
                    .size(text_size)
                    .color(match self.style {
                        AppButtonStyle::RepoTab { selected } => repo_tab_text_color(selected),
                        _ => theme::text(),
                    }),
            )
            .min_size(Vec2::new(
                inline_button_width(ui, self.label, text_size, min_width, max_width, 38.0),
                height,
            )),
        };
        let corner_radius = match self.style {
            AppButtonStyle::RepoTab { .. } => top_corners(radius),
            _ => CornerRadius::same(radius),
        };
        let button = button.stroke(Stroke::NONE).corner_radius(corner_radius);

        let response = match self.style {
            AppButtonStyle::RepoTab { .. } => ui.add_enabled(self.enabled, button.fill(fill)),
            AppButtonStyle::IconOnly | AppButtonStyle::Toolbar => {
                ui.scope(|ui| {
                    ui.spacing_mut().button_padding = Vec2::new(4.0, 0.0);
                    let widgets = &mut ui.visuals_mut().widgets;
                    widgets.inactive.bg_fill = toolbar_button_normal_fill();
                    widgets.inactive.weak_bg_fill = toolbar_button_normal_fill();
                    widgets.hovered.bg_fill = toolbar_button_hover_fill();
                    widgets.hovered.weak_bg_fill = toolbar_button_hover_fill();
                    widgets.active.bg_fill = toolbar_button_hover_fill();
                    widgets.active.weak_bg_fill = toolbar_button_hover_fill();
                    widgets.noninteractive.bg_fill = toolbar_button_normal_fill();
                    widgets.noninteractive.weak_bg_fill = toolbar_button_normal_fill();
                    ui.add_enabled(self.enabled, button)
                })
                .inner
            }
        };
        let response = if self.enabled {
            pointing_hand_cursor(response)
        } else {
            response
        };
        if let AppButtonStyle::RepoTab { selected } = self.style {
            paint_repo_tab_shadow(ui, response.rect, selected);
        }
        response.on_hover_text(self.label)
    }
}

fn toolbar_button_normal_fill() -> Color32 {
    Color32::TRANSPARENT
}

fn toolbar_button_hover_fill() -> Color32 {
    theme::hover()
}

fn repo_tab_fill(selected: bool, hovered: bool) -> Color32 {
    if selected {
        theme::panel()
    } else if hovered {
        theme::accent()
    } else {
        theme::accent_deep()
    }
}

fn repo_tab_text_color(selected: bool) -> Color32 {
    if selected {
        theme::text()
    } else {
        Color32::WHITE
    }
}

fn repo_tab_icon_color(selected: bool) -> Color32 {
    if selected {
        theme::accent()
    } else {
        Color32::WHITE
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RepoTabVisibilityItem {
    Repo(usize),
    Source,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RepoTabVisibleRange {
    start: usize,
    end: usize,
    count: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct RepoTabVisibility {
    visible_items: Vec<RepoTabVisibilityItem>,
    leading_overflow_items: Vec<RepoTabVisibilityItem>,
    trailing_overflow_items: Vec<RepoTabVisibilityItem>,
    visible_repo_indices: Vec<usize>,
    source_visible: bool,
    overflow_repo_indices: Vec<usize>,
    source_overflow: bool,
}

impl RepoTabVisibility {
    fn has_leading_overflow(&self) -> bool {
        !self.leading_overflow_items.is_empty()
    }

    fn has_trailing_overflow(&self) -> bool {
        !self.trailing_overflow_items.is_empty()
    }
}

fn repo_tab_width_from_text_width(text_width: f32) -> f32 {
    (text_width + 70.0).clamp(108.0, 204.0)
}

fn repo_tab_width(ui: &Ui, label: &str) -> f32 {
    let text_width = ui.fonts(|fonts| {
        fonts
            .layout_no_wrap(label.to_owned(), FontId::proportional(12.0), theme::text())
            .rect
            .width()
    });
    repo_tab_width_from_text_width(text_width)
}

fn repo_tab_visibility(
    repo_widths: &[f32],
    source_width: Option<f32>,
    active_repo_tab: Option<usize>,
    source_active: bool,
    available_width: f32,
) -> RepoTabVisibility {
    let mut items = repo_widths
        .iter()
        .copied()
        .enumerate()
        .map(|(index, width)| (RepoTabVisibilityItem::Repo(index), width))
        .collect::<Vec<_>>();
    if let Some(width) = source_width {
        items.push((RepoTabVisibilityItem::Source, width));
    }
    if items.is_empty() {
        return RepoTabVisibility::default();
    }

    let tabs_total = repo_tab_items_width(items.iter().map(|(_, width)| *width));
    let all_fit_width = REPO_TAB_STRIP_LEFT_PADDING + tabs_total + REPO_TAB_PLUS_WIDTH;
    if all_fit_width <= available_width {
        return RepoTabVisibility {
            visible_items: items.iter().map(|(item, _)| *item).collect(),
            leading_overflow_items: Vec::new(),
            trailing_overflow_items: Vec::new(),
            visible_repo_indices: (0..repo_widths.len()).collect(),
            source_visible: source_width.is_some(),
            overflow_repo_indices: Vec::new(),
            source_overflow: false,
        };
    }

    let active_position = if source_active {
        items
            .iter()
            .position(|(item, _)| *item == RepoTabVisibilityItem::Source)
            .unwrap_or(0)
    } else {
        active_repo_tab
            .and_then(|active| {
                items
                    .iter()
                    .position(|(item, _)| *item == RepoTabVisibilityItem::Repo(active))
            })
            .unwrap_or(0)
    };

    let visible_range = repo_tab_visible_range(&items, active_position, available_width);

    let mut visibility = RepoTabVisibility::default();
    let leading_positions = repo_tab_leading_overflow_positions(items.len(), visible_range);
    let visible_positions = repo_tab_visible_positions(items.len(), visible_range);
    let trailing_positions = repo_tab_trailing_overflow_positions(items.len(), visible_range);

    for position in leading_positions {
        repo_tab_visibility_push_overflow(&mut visibility, items[position].0, true);
    }
    for position in visible_positions {
        repo_tab_visibility_push_visible(&mut visibility, items[position].0);
    }
    for position in trailing_positions {
        repo_tab_visibility_push_overflow(&mut visibility, items[position].0, false);
    }
    visibility
}

fn repo_tab_visible_range(
    items: &[(RepoTabVisibilityItem, f32)],
    active_position: usize,
    available_width: f32,
) -> RepoTabVisibleRange {
    let mut start = active_position;
    let mut end = active_position;

    loop {
        if start == 0 && end + 1 >= items.len() {
            break;
        }

        let left_range = if start > 0 {
            Some(RepoTabVisibleRange {
                start: start - 1,
                end,
                count: end - start + 2,
            })
        } else {
            None
        };
        let right_range = if end + 1 < items.len() {
            Some(RepoTabVisibleRange {
                start,
                end: end + 1,
                count: end - start + 2,
            })
        } else {
            None
        };
        let left_fits = repo_tab_range_fits(items, left_range, available_width);
        let right_fits = repo_tab_range_fits(items, right_range, available_width);

        match (left_fits, right_fits) {
            (true, true) => {
                let left_count = active_position - start;
                let right_count = end - active_position;
                if left_count <= right_count {
                    start -= 1;
                } else {
                    end += 1;
                }
            }
            (true, false) => {
                start -= 1;
            }
            (false, true) => {
                end += 1;
            }
            (false, false) => break,
        }
    }

    let count = end - start + 1;
    RepoTabVisibleRange { start, end, count }
}

fn repo_tab_range_fits(
    items: &[(RepoTabVisibilityItem, f32)],
    range: Option<RepoTabVisibleRange>,
    available_width: f32,
) -> bool {
    let Some(range) = range else {
        return false;
    };
    let mut widths = Vec::new();
    let has_leading_overflow = !repo_tab_leading_overflow_positions(items.len(), range).is_empty();
    let has_trailing_overflow =
        !repo_tab_trailing_overflow_positions(items.len(), range).is_empty();

    if has_leading_overflow {
        widths.push(REPO_TAB_OVERFLOW_WIDTH);
    }
    widths.extend(
        repo_tab_visible_positions(items.len(), range)
            .into_iter()
            .map(|position| items[position].1),
    );
    if has_trailing_overflow {
        widths.push(REPO_TAB_OVERFLOW_WIDTH);
    }

    REPO_TAB_STRIP_LEFT_PADDING
        + repo_tab_items_width(widths.into_iter())
        + REPO_TAB_ITEM_GAP
        + REPO_TAB_PLUS_WIDTH
        <= available_width
}

fn repo_tab_visible_positions(len: usize, range: RepoTabVisibleRange) -> Vec<usize> {
    (range.start..=range.end.min(len.saturating_sub(1))).collect()
}

fn repo_tab_leading_overflow_positions(len: usize, range: RepoTabVisibleRange) -> Vec<usize> {
    if range.count >= len {
        Vec::new()
    } else {
        (0..range.start).collect()
    }
}

fn repo_tab_trailing_overflow_positions(len: usize, range: RepoTabVisibleRange) -> Vec<usize> {
    if range.count >= len {
        Vec::new()
    } else {
        ((range.end + 1)..len).collect()
    }
}

fn repo_tab_visibility_push_visible(
    visibility: &mut RepoTabVisibility,
    item: RepoTabVisibilityItem,
) {
    visibility.visible_items.push(item);
    match item {
        RepoTabVisibilityItem::Repo(index) => visibility.visible_repo_indices.push(index),
        RepoTabVisibilityItem::Source => visibility.source_visible = true,
    }
}

fn repo_tab_visibility_push_overflow(
    visibility: &mut RepoTabVisibility,
    item: RepoTabVisibilityItem,
    leading: bool,
) {
    if leading {
        visibility.leading_overflow_items.push(item);
    } else {
        visibility.trailing_overflow_items.push(item);
    }
    match item {
        RepoTabVisibilityItem::Repo(index) => visibility.overflow_repo_indices.push(index),
        RepoTabVisibilityItem::Source => visibility.source_overflow = true,
    }
}

fn repo_tab_items_width(widths: impl Iterator<Item = f32>) -> f32 {
    let mut count = 0_usize;
    let mut total = 0.0;
    for width in widths {
        if count > 0 {
            total += REPO_TAB_ITEM_GAP;
        }
        total += width;
        count += 1;
    }
    total
}

fn top_corners(radius: u8) -> CornerRadius {
    CornerRadius {
        nw: radius,
        ne: radius,
        sw: 0,
        se: 0,
    }
}

fn tool_row_corners() -> CornerRadius {
    CornerRadius {
        nw: 0,
        ne: 0,
        sw: 6,
        se: 6,
    }
}

fn inline_button_width(
    ui: &Ui,
    label: &str,
    text_size: f32,
    min_width: f32,
    max_width: f32,
    x_padding: f32,
) -> f32 {
    let text_width = if text_size > 0.0 {
        ui.fonts(|fonts| {
            fonts
                .layout_no_wrap(
                    label.to_owned(),
                    FontId::proportional(text_size),
                    theme::text(),
                )
                .rect
                .width()
        })
    } else {
        0.0
    };
    inline_button_width_from_text(text_width, min_width, max_width, x_padding)
}

fn inline_button_width_from_text(
    text_width: f32,
    min_width: f32,
    max_width: f32,
    x_padding: f32,
) -> f32 {
    (text_width + x_padding).clamp(min_width, max_width)
}

fn icon_button(ui: &mut Ui, icon: UiIcon, tooltip: &str, enabled: bool) -> egui::Response {
    AppButton::icon_only(icon, tooltip, enabled).show(ui)
}

fn toolbar_button(ui: &mut Ui, icon: &str, label: &str, enabled: bool) -> egui::Response {
    let icon = toolbar_icon(icon, label);
    AppButton::toolbar(icon, label, enabled).show(ui)
}

fn repo_toolbar_loading_indicator(ui: &mut Ui) {
    let rect = ui.max_rect();
    let size = 18.0;
    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(size));
    let angle = ui.input(|input| input.time as f32 * std::f32::consts::TAU);
    ui.ctx().request_repaint();
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(icon_rect), |ui| {
        ui.add(
            egui::Image::new(icon_source(UiIcon::Loading))
                .fit_to_exact_size(Vec2::splat(size))
                .rotate(angle, Vec2::splat(0.5))
                .tint(theme::muted()),
        );
    });
}

fn themed_text_edit_selection(ui: &mut Ui) {
    ui.visuals_mut().selection.bg_fill = theme::accent_soft();
    ui.visuals_mut().selection.stroke = Stroke::new(1.0, theme::accent_deep());
}

fn themed_singleline_text_edit<'a>(text: &'a mut String, hint: &str) -> TextEdit<'a> {
    TextEdit::singleline(text)
        .hint_text(RichText::new(hint.to_owned()).color(theme::muted()))
        .text_color(theme::text())
}

fn commit_message_text_edit<'a>(message: &'a mut String, id: egui::Id, hint: &str) -> TextEdit<'a> {
    TextEdit::multiline(message)
        .id(id)
        .hint_text(RichText::new(hint.to_owned()).color(theme::muted()))
        .text_color(theme::text())
        .frame(false)
}

fn commit_message_editor_ui(
    ui: &mut Ui,
    message: &mut String,
    id: egui::Id,
    hint: &str,
    size: Vec2,
) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let inner_rect = rect.shrink2(Vec2::new(4.0, 2.0));

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(inner_rect), |ui| {
        ui.set_clip_rect(inner_rect);
        themed_text_edit_selection(ui);
        ui.add_sized(
            inner_rect.size(),
            commit_message_text_edit(message, id, hint),
        )
    })
    .inner
}

fn commit_submit_button(ui: &mut Ui, rect: Rect, label: &str, enabled: bool) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, ui.id().with("commit_submit_button"), sense);
    let response = if enabled {
        pointing_hand_cursor(response)
    } else {
        response
    };
    let fill = if enabled {
        theme::accent_deep()
    } else {
        theme::accent_deep().gamma_multiply(0.55)
    };
    let text_color = if enabled {
        Color32::WHITE
    } else {
        Color32::WHITE.gamma_multiply(0.78)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
        text_color,
    );

    response
}

fn commit_checkbox(ui: &mut Ui, value: &mut bool, label: &str) -> egui::Response {
    let font = FontId::proportional(12.0);
    let text_width = ui.fonts(|fonts| {
        fonts
            .layout_no_wrap(label.to_owned(), font.clone(), theme::text())
            .rect
            .width()
    });
    let size = Vec2::new(text_width + 24.0, 22.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let mut response = pointing_hand_cursor(response);
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }

    let square = Rect::from_min_size(
        Pos2::new(rect.left() + 1.0, rect.center().y - 6.0),
        Vec2::splat(12.0),
    );
    let shadow = square.translate(Vec2::new(1.5, 2.0));
    ui.painter().rect_filled(
        shadow.expand(1.0),
        CornerRadius::same(2),
        theme::accent_shadow().gamma_multiply(0.45),
    );
    ui.painter().rect_filled(
        square,
        CornerRadius::same(2),
        if *value {
            theme::accent_deep()
        } else {
            theme::panel()
        },
    );
    ui.painter().line_segment(
        [square.left_top(), Pos2::new(square.right(), square.top())],
        Stroke::new(1.0, Color32::WHITE.gamma_multiply(0.7)),
    );
    ui.painter().line_segment(
        [square.left_top(), Pos2::new(square.left(), square.bottom())],
        Stroke::new(1.0, Color32::WHITE.gamma_multiply(0.7)),
    );
    if *value {
        let check_color = Color32::WHITE;
        ui.painter().line_segment(
            [
                Pos2::new(square.left() + 2.5, square.center().y),
                Pos2::new(square.left() + 5.0, square.bottom() - 3.0),
            ],
            Stroke::new(1.6, check_color),
        );
        ui.painter().line_segment(
            [
                Pos2::new(square.left() + 5.0, square.bottom() - 3.0),
                Pos2::new(square.right() - 2.0, square.top() + 3.0),
            ],
            Stroke::new(1.6, check_color),
        );
    }
    ui.painter().text(
        Pos2::new(square.right() + 7.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        theme::text(),
    );

    response
}

fn toolbar_icon(raw: &str, _label: &str) -> UiIcon {
    match raw {
        "commit" => UiIcon::Commit,
        "pull" => UiIcon::Pull,
        "push" => UiIcon::Push,
        "fetch" => UiIcon::Fetch,
        "branch" => UiIcon::Branch,
        "tag" => UiIcon::Tag,
        "stash" => UiIcon::Stash,
        "git-flow" => UiIcon::Branch,
        "remote" => UiIcon::Globe,
        "terminal" => UiIcon::Terminal,
        "resource-manager" => UiIcon::ResourceManager,
        "clone" => UiIcon::Fetch,
        "add" => UiIcon::Folder,
        "create" => UiIcon::Plus,
        "open" => UiIcon::Folder,
        "refresh" => UiIcon::Refresh,
        "settings" => UiIcon::Settings,
        "cherry-pick" => UiIcon::CherryPick,
        "+" => UiIcon::Plus,
        _ => UiIcon::Commit,
    }
}

fn icon_source(icon: UiIcon) -> egui::ImageSource<'static> {
    match icon {
        UiIcon::Commit => egui::include_image!("../assets/icons/commit.svg"),
        UiIcon::Pull => egui::include_image!("../assets/icons/pull.svg"),
        UiIcon::Push => egui::include_image!("../assets/icons/push.svg"),
        UiIcon::Fetch => egui::include_image!("../assets/icons/fetch.svg"),
        UiIcon::Branch => egui::include_image!("../assets/icons/branch.svg"),
        UiIcon::Tag => egui::include_image!("../assets/icons/tag.svg"),
        UiIcon::Stash => egui::include_image!("../assets/icons/stash.svg"),
        UiIcon::Folder => egui::include_image!("../assets/icons/folder.svg"),
        UiIcon::Refresh => egui::include_image!("../assets/icons/refresh.svg"),
        UiIcon::Settings => egui::include_image!("../assets/icons/settings.svg"),
        UiIcon::Plus => egui::include_image!("../assets/icons/plus.svg"),
        UiIcon::Edit => egui::include_image!("../assets/icons/edit.svg"),
        UiIcon::AddFile => egui::include_image!("../assets/icons/plus.svg"),
        UiIcon::DeleteFile => egui::include_image!("../assets/icons/delete-file.svg"),
        UiIcon::RenameFile => egui::include_image!("../assets/icons/rename-file.svg"),
        UiIcon::File => egui::include_image!("../assets/icons/file.svg"),
        UiIcon::Workspace => egui::include_image!("../assets/icons/workspace.svg"),
        UiIcon::History => egui::include_image!("../assets/icons/history.svg"),
        UiIcon::Search => egui::include_image!("../assets/icons/search.svg"),
        UiIcon::Globe => egui::include_image!("../assets/icons/globe.svg"),
        UiIcon::Terminal => egui::include_image!("../assets/icons/terminal.svg"),
        UiIcon::ResourceManager => egui::include_image!("../assets/icons/resource-manager.svg"),
        UiIcon::CherryPick => egui::include_image!("../assets/icons/cherry-pick.svg"),
        UiIcon::Warning => egui::include_image!("../assets/icons/warning.svg"),
        UiIcon::More => egui::include_image!("../assets/icons/more.svg"),
        UiIcon::Loading => egui::include_image!("../assets/icons/loading.svg"),
    }
}

fn draw_ui_icon(ui: &mut Ui, rect: Rect, icon: UiIcon, color: Color32) {
    let clip = rect.intersect(ui.clip_rect());
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.set_clip_rect(clip);
        ui.add(
            egui::Image::new(icon_source(icon))
                .fit_to_exact_size(rect.size())
                .tint(color),
        );
    });
}

fn paint_ui_icon(ui: &Ui, rect: Rect, icon: UiIcon, color: Color32) {
    egui::Image::new(icon_source(icon))
        .fit_to_exact_size(rect.size())
        .tint(color)
        .paint_at(ui, rect);
}

fn app_title_logo(ui: &mut Ui) {
    ui.add(
        egui::Image::new(egui::include_image!("../assets/icons/logo-ga.svg"))
            .fit_to_exact_size(Vec2::new(16.0, 16.0)),
    );
}

fn window_control_button(ui: &mut Ui, label: &str, close: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(36.0, 24.0), Sense::click());
    let fill = if close {
        Color32::from_rgb(192, 55, 43)
    } else if response.hovered() {
        theme::hover()
    } else {
        theme::panel_soft()
    };
    ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(14.0),
        if close { Color32::WHITE } else { theme::text() },
    );
    pointing_hand_cursor(response)
}

fn menu_button(ui: &mut Ui, label: &'static str, add_contents: impl FnOnce(&mut Ui)) {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = Vec2::new(6.0, 2.0);
        ui.visuals_mut().widgets.inactive.bg_fill = Color32::TRANSPARENT;
        ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        ui.visuals_mut().widgets.hovered.bg_fill = theme::hover().gamma_multiply(0.35);
        ui.visuals_mut().widgets.hovered.weak_bg_fill = ui.visuals().widgets.hovered.bg_fill;
        ui.visuals_mut().widgets.open.bg_fill = theme::hover().gamma_multiply(0.42);
        ui.visuals_mut().widgets.open.weak_bg_fill = ui.visuals().widgets.open.bg_fill;
        ui.menu_button(
            RichText::new(label).color(theme::text()).size(13.0),
            add_contents,
        );
    });
}

fn menu_label(language: Language, key: &str) -> &'static str {
    match (language, key) {
        (Language::Chinese, "file") => "\u{6587}\u{4ef6}(F)",
        (Language::Chinese, "edit") => "\u{7f16}\u{8f91}(E)",
        (Language::Chinese, "view") => "\u{67e5}\u{770b}(V)",
        (Language::Chinese, "repo") => "\u{4ed3}\u{5e93}(R)",
        (Language::Chinese, "actions") => "\u{64cd}\u{4f5c}(A)",
        (Language::Chinese, "tools") => "\u{5de5}\u{5177}(T)",
        (Language::Chinese, "help") => "\u{5e2e}\u{52a9}(H)",
        (Language::Chinese, "options") => "\u{9009}\u{9879}(O)",
        (Language::Chinese, "light_theme") => "\u{65e5}\u{95f4}\u{4e3b}\u{9898}",
        (Language::Chinese, "dark_theme") => "\u{591c}\u{95f4}\u{4e3b}\u{9898}",
        (Language::Chinese, "undo") => "\u{64a4}\u{9500}",
        (Language::Chinese, "redo") => "\u{91cd}\u{505a}",
        (Language::Chinese, "ssh_agent") => "\u{542f}\u{52a8}SSH\u{52a9}\u{624b}...",
        (Language::Chinese, "process_viewer") => "\u{8fdb}\u{7a0b}\u{67e5}\u{770b}\u{5668}",
        (Language::Chinese, "about") => "\u{5173}\u{4e8e} Git Agent",
        (_, "file") => "File(F)",
        (_, "edit") => "Edit(E)",
        (_, "view") => "View(V)",
        (_, "repo") => "Repository(R)",
        (_, "actions") => "Actions(A)",
        (_, "tools") => "Tools(T)",
        (_, "help") => "Help(H)",
        (_, "options") => "Options(O)",
        (_, "light_theme") => "Light Theme",
        (_, "dark_theme") => "Dark Theme",
        (_, "undo") => "Undo",
        (_, "redo") => "Redo",
        (_, "ssh_agent") => "Start SSH Agent...",
        (_, "process_viewer") => "Process Viewer",
        (_, "about") => "About Git Agent",
        _ => "",
    }
}

fn settings_theme_accent_title(language: Language) -> &'static str {
    match language {
        Language::Chinese => "\u{4e3b}\u{9898}\u{8272}",
        Language::English => "Theme Color",
    }
}

fn theme_accent_label(language: Language, accent: theme::ThemeAccent) -> &'static str {
    match (language, accent) {
        (Language::Chinese, theme::ThemeAccent::Green) => "\u{7eff}\u{8272}",
        (Language::Chinese, theme::ThemeAccent::Blue) => "\u{84dd}\u{8272}",
        (Language::Chinese, theme::ThemeAccent::Purple) => "\u{7d2b}\u{8272}",
        (Language::Chinese, theme::ThemeAccent::Rose) => "\u{73ab}\u{7ea2}",
        (Language::Chinese, theme::ThemeAccent::Orange) => "\u{6a59}\u{8272}",
        (_, theme::ThemeAccent::Green) => "Green",
        (_, theme::ThemeAccent::Blue) => "Blue",
        (_, theme::ThemeAccent::Purple) => "Purple",
        (_, theme::ThemeAccent::Rose) => "Rose",
        (_, theme::ThemeAccent::Orange) => "Orange",
    }
}

fn resource_header(ui: &mut Ui, title: &str, meta: &str, action: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(title)
                .size(22.0)
                .strong()
                .color(theme::text()),
        );
        ui.label(RichText::new(meta).small().color(theme::muted()));
        ui.with_layout(Layout::right_to_left(Align::Center), action);
    });
    ui.add_space(8.0);
}

fn resource_label(language: Language, key: &str) -> &'static str {
    match (language, key) {
        (Language::Chinese, "name") => "\u{540d}\u{79f0}",
        (Language::Chinese, "type") => "\u{7c7b}\u{578b}",
        (Language::Chinese, "status") => "\u{72b6}\u{6001}",
        (Language::Chinese, "target") => "\u{76ee}\u{6807}",
        (Language::Chinese, "message") => "\u{4fe1}\u{606f}",
        (Language::Chinese, "when") => "\u{65f6}\u{95f4}",
        (Language::Chinese, "stash") => "\u{8d2e}\u{85cf}",
        (Language::Chinese, "action") => "\u{64cd}\u{4f5c}",
        (_, "name") => "Name",
        (_, "type") => "Type",
        (_, "status") => "Status",
        (_, "target") => "Target",
        (_, "message") => "Message",
        (_, "when") => "When",
        (_, "stash") => "Stash",
        (_, "action") => "Action",
        _ => "",
    }
}

fn branch_table_header(ui: &mut Ui, language: Language) {
    let width = ui.available_width();
    let status_w = 110.0;
    let type_w = 120.0;
    let name_w = (width - status_w - type_w).max(220.0);
    ui.horizontal(|ui| {
        table_header_cell(ui, resource_label(language, "name"), name_w);
        table_header_cell(ui, resource_label(language, "type"), type_w);
        table_header_cell(ui, resource_label(language, "status"), status_w);
    });
    ui.add_space(6.0);
}

fn tag_table_header(ui: &mut Ui, language: Language) {
    let columns = tag_table_columns(ui.available_width());
    ui.horizontal(|ui| {
        table_header_cell(ui, resource_label(language, "name"), columns.name);
        table_header_cell(ui, resource_label(language, "target"), columns.target);
        table_header_cell(ui, resource_label(language, "message"), columns.subject);
        table_header_cell(ui, resource_label(language, "action"), columns.action);
    });
    ui.add_space(6.0);
}

#[derive(Clone, Copy, Debug)]
struct TagTableColumns {
    name: f32,
    target: f32,
    subject: f32,
    action: f32,
}

fn tag_table_columns(width: f32) -> TagTableColumns {
    let action = 112.0;
    let target = 150.0;
    let name = width.mul_add(0.26, 0.0).clamp(180.0, 240.0);
    let subject = (width - name - target - action).clamp(260.0, 520.0);
    TagTableColumns {
        name,
        target,
        subject,
        action,
    }
}

fn tag_table_cell(ui: &mut Ui, width: f32, text: RichText) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, RESOURCE_ROW_HEIGHT),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.add(egui::Label::new(text).truncate());
        },
    );
}

fn stash_table_header(ui: &mut Ui, language: Language) {
    let width = ui.available_width();
    let stash_w = 150.0;
    let when_w = 132.0;
    let message_w = (width - stash_w - when_w).max(220.0);
    ui.horizontal(|ui| {
        table_header_cell(ui, resource_label(language, "stash"), stash_w);
        table_header_cell(ui, resource_label(language, "message"), message_w);
        table_header_cell(ui, resource_label(language, "when"), when_w);
    });
    ui.add_space(6.0);
}

fn table_header_cell(ui: &mut Ui, label: &str, width: f32) {
    ui.add_sized(
        [width, RESOURCE_TABLE_HEADER_HEIGHT],
        egui::Label::new(RichText::new(label).color(theme::muted())),
    );
}

fn source_tree_panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::bg())
        .shadow(panel_shadow())
        .inner_margin(egui::Margin::symmetric(8, 8))
}

fn diff_panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::panel_recessed())
        .corner_radius(CornerRadius::same(WORKSPACE_CARD_RADIUS))
        .inner_margin(egui::Margin::same(0))
}

fn worktree_diff_panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::panel())
        .corner_radius(CornerRadius::same(WORKSPACE_CARD_RADIUS))
        .inner_margin(egui::Margin::same(0))
}

fn diff_display_mode_salt(mode: DiffDisplayMode) -> &'static str {
    match mode {
        DiffDisplayMode::Blocks => "blocks",
        DiffDisplayMode::Full => "full",
    }
}

fn diff_display_mode_switch(
    ui: &mut Ui,
    rect: Rect,
    current: &mut DiffDisplayMode,
    blocks_label: &str,
    full_label: &str,
) -> egui::Response {
    let response = ui.interact(
        rect,
        ui.id().with("diff_display_mode_switch"),
        Sense::click(),
    );
    ui.painter()
        .rect_filled(rect, CornerRadius::same(3), theme::panel());

    let half = rect.width() / 2.0;
    let blocks_rect = Rect::from_min_size(rect.left_top(), Vec2::new(half, rect.height()));
    let full_rect = Rect::from_min_size(
        Pos2::new(blocks_rect.right(), rect.top()),
        Vec2::new(half, rect.height()),
    );
    let selected_rect = if *current == DiffDisplayMode::Blocks {
        blocks_rect
    } else {
        full_rect
    };
    ui.painter().rect_filled(
        selected_rect.shrink(1.0),
        CornerRadius::same(3),
        theme::accent_deep(),
    );

    for (mode, part_rect, label) in [
        (DiffDisplayMode::Blocks, blocks_rect, blocks_label),
        (DiffDisplayMode::Full, full_rect, full_label),
    ] {
        let selected = *current == mode;
        ui.painter().text(
            part_rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(11.0),
            if selected {
                Color32::WHITE
            } else {
                theme::text()
            },
        );
    }
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            *current = if pos.x < rect.center().x {
                DiffDisplayMode::Blocks
            } else {
                DiffDisplayMode::Full
            };
        }
    }
    pointing_hand_cursor(response)
}

fn header_action_button_at(ui: &mut Ui, rect: Rect, label: &str) -> egui::Response {
    let response = pointing_hand_cursor(ui.interact(
        rect,
        ui.id().with(("header_action", label)),
        Sense::click(),
    ));
    let fill = if response.hovered() {
        theme::hover()
    } else {
        theme::panel()
    };
    ui.painter().rect_filled(rect, CornerRadius::same(3), fill);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0),
        theme::text(),
    );
    response
}

fn draw_elided_path_label(ui: &mut Ui, rect: Rect, path: &str) {
    let font = FontId::proportional(13.0);
    let text = elide_start_to_width(ui, path, rect.width().max(0.0), font.clone());
    let response = ui.interact(rect, ui.id().with(("diff_path", path)), Sense::hover());
    ui.painter().text(
        Pos2::new(rect.right(), rect.center().y),
        Align2::RIGHT_CENTER,
        text,
        font,
        theme::text(),
    );
    response.on_hover_text(path);
}

fn elide_start_to_width(ui: &Ui, text: &str, max_width: f32, font: FontId) -> String {
    let text_width = |value: &str| {
        ui.fonts(|fonts| {
            fonts
                .layout_no_wrap(value.to_owned(), font.clone(), theme::text())
                .rect
                .width()
        })
    };
    if text_width(text) <= max_width {
        return text.to_owned();
    }

    let chars = text.chars().collect::<Vec<_>>();
    let mut low = 0usize;
    let mut high = chars.len();
    let mut best = String::from("~");
    while low <= high {
        let keep = (low + high) / 2;
        let candidate = format!(
            "~{}",
            chars[chars.len().saturating_sub(keep)..]
                .iter()
                .collect::<String>()
        );
        if text_width(&candidate) <= max_width {
            best = candidate;
            low = keep.saturating_add(1);
        } else if keep == 0 {
            break;
        } else {
            high = keep - 1;
        }
    }
    best
}

fn history_sort_order_label(language: Language, order: HistorySortOrder) -> &'static str {
    match (language, order) {
        (Language::Chinese, HistorySortOrder::Date) => "\u{6309}\u{65e5}\u{671f}\u{6392}\u{5e8f}",
        (Language::Chinese, HistorySortOrder::Topology) => "\u{5c42}\u{7ea7}\u{6392}\u{5e8f}",
        (Language::English, HistorySortOrder::Date) => "Sort by date",
        (Language::English, HistorySortOrder::Topology) => "Topo order",
    }
}

fn history_branch_scope_label(language: Language, scope: HistoryBranchScope) -> &'static str {
    match (language, scope) {
        (Language::Chinese, HistoryBranchScope::Current) => "\u{5f53}\u{524d}\u{5206}\u{652f}",
        (Language::Chinese, HistoryBranchScope::All) => "\u{6240}\u{6709}\u{5206}\u{652f}",
        (Language::English, HistoryBranchScope::Current) => "Current branch",
        (Language::English, HistoryBranchScope::All) => "All branches",
    }
}

fn history_branch_scope_dropdown(
    ui: &mut Ui,
    rect: Rect,
    language: Language,
    current: HistoryBranchScope,
) -> Option<HistoryBranchScope> {
    let (response, popup_id) = history_toolbar_dropdown_button(
        ui,
        rect,
        "history_branch_scope",
        history_branch_scope_label(language, current),
    );
    egui::popup::popup_below_widget(
        ui,
        popup_id,
        &response,
        egui::popup::PopupCloseBehavior::CloseOnClick,
        |ui| {
            ui.set_min_width(rect.width());
            let mut selected = None;
            if history_toolbar_popup_option(
                ui,
                current == HistoryBranchScope::All,
                history_branch_scope_label(language, HistoryBranchScope::All),
            )
            .clicked()
            {
                selected = Some(HistoryBranchScope::All);
            }
            if history_toolbar_popup_option(
                ui,
                current == HistoryBranchScope::Current,
                history_branch_scope_label(language, HistoryBranchScope::Current),
            )
            .clicked()
            {
                selected = Some(HistoryBranchScope::Current);
            }
            selected
        },
    )
    .flatten()
}

fn history_sort_order_dropdown(
    ui: &mut Ui,
    rect: Rect,
    language: Language,
    current: HistorySortOrder,
) -> Option<HistorySortOrder> {
    let (response, popup_id) = history_toolbar_dropdown_button(
        ui,
        rect,
        "history_sort_order",
        history_sort_order_label(language, current),
    );
    egui::popup::popup_below_widget(
        ui,
        popup_id,
        &response,
        egui::popup::PopupCloseBehavior::CloseOnClick,
        |ui| {
            ui.set_min_width(rect.width());
            let mut selected = None;
            if history_toolbar_popup_option(
                ui,
                current == HistorySortOrder::Date,
                history_sort_order_label(language, HistorySortOrder::Date),
            )
            .clicked()
            {
                selected = Some(HistorySortOrder::Date);
            }
            if history_toolbar_popup_option(
                ui,
                current == HistorySortOrder::Topology,
                history_sort_order_label(language, HistorySortOrder::Topology),
            )
            .clicked()
            {
                selected = Some(HistorySortOrder::Topology);
            }
            selected
        },
    )
    .flatten()
}

fn history_toolbar_dropdown_button(
    ui: &mut Ui,
    rect: Rect,
    id_salt: &'static str,
    label: &'static str,
) -> (egui::Response, egui::Id) {
    let button_id = ui.make_persistent_id((id_salt, "button"));
    let popup_id = ui.make_persistent_id((id_salt, "popup"));
    let response = pointing_hand_cursor(ui.interact(rect, button_id, Sense::click()));
    if response.clicked() {
        ui.memory_mut(|memory| memory.toggle_popup(popup_id));
    }

    let fill = if response.hovered() {
        theme::hover()
    } else {
        theme::panel_soft()
    };
    ui.painter().rect_filled(rect, CornerRadius::same(2), fill);
    ui.painter().text(
        Pos2::new(rect.left() + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        theme::text(),
    );
    let arrow = Pos2::new(rect.right() - 13.0, rect.center().y + 1.0);
    ui.painter().line_segment(
        [Pos2::new(arrow.x - 4.0, arrow.y - 3.0), arrow],
        Stroke::new(1.4, theme::text()),
    );
    ui.painter().line_segment(
        [arrow, Pos2::new(arrow.x + 4.0, arrow.y - 3.0)],
        Stroke::new(1.4, theme::text()),
    );
    (response, popup_id)
}

fn history_toolbar_popup_option(
    ui: &mut Ui,
    selected: bool,
    label: &'static str,
) -> egui::Response {
    let text = RichText::new(label).color(if selected {
        Color32::WHITE
    } else {
        theme::text()
    });
    pointing_hand_cursor(ui.selectable_label(selected, text))
}

fn history_toolbar_checkbox_at(
    ui: &mut Ui,
    rect: Rect,
    value: &mut bool,
    label: &str,
) -> egui::Response {
    let mut response = pointing_hand_cursor(ui.interact(
        rect,
        ui.make_persistent_id("history_remote_refs_checkbox"),
        Sense::click(),
    ));
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }

    let painter = ui.painter();
    let check_rect = Rect::from_center_size(
        Pos2::new(rect.left() + 7.0, rect.center().y),
        Vec2::splat(12.0),
    );
    let fill = if *value {
        theme::accent_soft()
    } else {
        theme::panel_recessed()
    };
    painter.rect_filled(check_rect, CornerRadius::same(1), fill);
    for (a, b) in [
        (check_rect.left_top(), check_rect.right_top()),
        (check_rect.right_top(), check_rect.right_bottom()),
        (check_rect.right_bottom(), check_rect.left_bottom()),
        (check_rect.left_bottom(), check_rect.left_top()),
    ] {
        painter.line_segment([a, b], Stroke::new(1.0, theme::muted()));
    }
    if *value {
        let y = check_rect.center().y;
        painter.line_segment(
            [
                Pos2::new(check_rect.left() + 2.5, y),
                Pos2::new(check_rect.left() + 5.0, y + 3.0),
            ],
            Stroke::new(1.5, theme::accent_deep()),
        );
        painter.line_segment(
            [
                Pos2::new(check_rect.left() + 5.0, y + 3.0),
                Pos2::new(check_rect.right() - 2.0, y - 4.0),
            ],
            Stroke::new(1.5, theme::accent_deep()),
        );
    }
    painter.text(
        Pos2::new(check_rect.right() + 6.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        theme::text(),
    );
    response
}

fn history_graph_width(width: f32, lanes: usize, prefs: &LayoutPrefs) -> f32 {
    let _ = lanes;
    let min_width = 24.0;
    let preferred = width * prefs.history_graph_pct;
    let max_width = (width * 0.50).max(min_width);
    preferred.clamp(min_width, max_width)
}

fn history_commit_matches_search(
    commit: &Commit,
    search_dimension: SearchDimension,
    query: &str,
    _details_cache: &HashMap<String, CommitDetails>,
    file_search_query: &str,
    file_search_hashes: &HashSet<String>,
) -> bool {
    match search_dimension {
        SearchDimension::Message => {
            commit.subject.to_lowercase().contains(query)
                || commit.hash.starts_with(query)
                || commit.short_hash.starts_with(query)
        }
        SearchDimension::Files => {
            file_search_query == query && file_search_hashes.contains(&commit.hash)
        }
        SearchDimension::Author => commit.author.to_lowercase().contains(query),
    }
}

fn history_virtual_row_range(
    viewport: Rect,
    row_height: f32,
    row_count: usize,
) -> std::ops::Range<usize> {
    if row_count == 0 || row_height <= 0.0 {
        return 0..0;
    }
    let overscan = 4usize;
    let start = (viewport.top() / row_height).floor().max(0.0) as usize;
    let start = start.saturating_sub(overscan);
    let visible = (viewport.height() / row_height).ceil().max(1.0) as usize + overscan * 2;
    let end = start.saturating_add(visible).min(row_count);
    start..end
}

fn history_table_chrome_height() -> f32 {
    28.0 + 4.0 + HISTORY_TABLE_HEADER_HEIGHT
}

fn history_top_min_height() -> f32 {
    history_table_height_for_rows(3.0)
}

fn history_table_height_for_rows(rows: f32) -> f32 {
    history_table_chrome_height() + rows.max(1.0) * HISTORY_TABLE_ROW_HEIGHT
}

#[derive(Clone, Copy, Debug)]
struct HistoryColumnWidths {
    desc: f32,
    date: f32,
    author: f32,
    hash: f32,
}

fn history_column_min_widths() -> [f32; 4] {
    [48.0, 88.0, 64.0, 18.0]
}

fn history_column_widths(width: f32, graph_width: f32, prefs: &LayoutPrefs) -> HistoryColumnWidths {
    let remaining = (width - graph_width - 8.0).max(1.0);
    let mins = history_column_min_widths();
    let min_total = mins.iter().sum::<f32>();
    if remaining <= min_total {
        let scale = remaining / min_total;
        return HistoryColumnWidths {
            desc: mins[0] * scale,
            date: mins[1] * scale,
            author: mins[2] * scale,
            hash: mins[3] * scale,
        };
    }

    let weights = [
        sanitize_history_pct(prefs.history_desc_pct, 0.52),
        sanitize_history_pct(prefs.history_date_pct, 0.18),
        sanitize_history_pct(prefs.history_author_pct, 0.18),
        sanitize_history_pct(prefs.history_hash_pct, 0.08),
    ];
    let weight_total = weights.iter().sum::<f32>().max(0.001);
    let mut widths = weights.map(|weight| remaining * weight / weight_total);

    for index in 0..widths.len() {
        if widths[index] >= mins[index] {
            continue;
        }
        let deficit = mins[index] - widths[index];
        widths[index] = mins[index];
        let spare_total = widths
            .iter()
            .zip(mins.iter())
            .enumerate()
            .filter(|(spare_index, _)| *spare_index != index)
            .map(|(_, (width, min_width))| (width - min_width).max(0.0))
            .sum::<f32>();
        if spare_total <= 0.0 {
            continue;
        }
        for spare_index in 0..widths.len() {
            if spare_index == index {
                continue;
            }
            let spare = (widths[spare_index] - mins[spare_index]).max(0.0);
            widths[spare_index] -= deficit * spare / spare_total;
        }
    }

    HistoryColumnWidths {
        desc: widths[0],
        date: widths[1],
        author: widths[2],
        hash: widths[3],
    }
}

fn history_table_header(
    ui: &mut Ui,
    language: Language,
    graph_width: f32,
    prefs: &mut LayoutPrefs,
    select_for_cherry_pick: bool,
) -> bool {
    let width = ui.available_width();
    let selection_width = if select_for_cherry_pick { 30.0 } else { 0.0 };
    let cols = history_column_widths(width - selection_width, graph_width, prefs);
    let labels = if language == Language::Chinese {
        (
            "\u{56fe}\u{8c31}",
            "\u{63cf}\u{8ff0}",
            "\u{65e5}\u{671f}",
            "\u{4f5c}\u{8005}",
            "\u{63d0}\u{4ea4}",
        )
    } else {
        ("Graph", "Description", "Date", "Author", "Commit")
    };
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(width, HISTORY_TABLE_HEADER_HEIGHT),
        Sense::hover(),
    );
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, theme::panel_soft());
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(120, 130, 148, 80)),
    );

    let mut x = rect.left() + 8.0;
    let y = rect.center().y;
    if select_for_cherry_pick {
        x += selection_width;
    }
    for (label, col_w) in [
        (labels.0, graph_width),
        (labels.1, cols.desc),
        (labels.2, cols.date),
        (labels.3, cols.author),
        (labels.4, cols.hash),
    ] {
        ui.painter().text(
            Pos2::new(x, y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(12.0),
            theme::muted(),
        );
        x += col_w;
    }

    let content_left = rect.left() + selection_width + graph_width;
    let boundaries = [
        (content_left, "history_graph_desc_resize", 3),
        (content_left + cols.desc, "history_desc_date_resize", 0),
        (
            content_left + cols.desc + cols.date,
            "history_date_author_resize",
            1,
        ),
        (
            content_left + cols.desc + cols.date + cols.author,
            "history_author_hash_resize",
            2,
        ),
    ];
    let mut changed = false;
    for (x, id, boundary) in boundaries {
        let handle = Rect::from_min_max(
            Pos2::new(x - 3.0, rect.top()),
            Pos2::new(x + 3.0, rect.bottom()),
        );
        let response = ui
            .interact(handle, ui.id().with(id), Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
        if response.hovered() || response.dragged() {
            ui.painter().line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(1.0, theme::accent()),
            );
        }
        if response.dragged() {
            let delta = ui.input(|input| input.pointer.delta().x);
            if delta.abs() > 0.0 {
                if boundary == 3 {
                    adjust_history_graph_width(
                        prefs,
                        width - selection_width,
                        graph_width,
                        cols.desc,
                        delta,
                    );
                } else {
                    adjust_history_column_widths(
                        prefs,
                        &cols,
                        width - selection_width - graph_width - 8.0,
                        boundary,
                        delta,
                    );
                }
                changed = true;
            }
        }
    }
    changed
}

fn adjust_history_graph_width(
    prefs: &mut LayoutPrefs,
    table_width: f32,
    graph_width: f32,
    desc_width: f32,
    delta: f32,
) {
    let min_graph = 24.0;
    let min_desc = 90.0;
    let max_graph = (table_width * 0.50).max(min_graph);
    let delta = delta.clamp(min_graph - graph_width, desc_width - min_desc);
    let graph_width = (graph_width + delta).clamp(min_graph, max_graph);
    prefs.history_graph_pct = graph_width / table_width.max(1.0);
    prefs.clamp();
}

fn adjust_history_column_widths(
    prefs: &mut LayoutPrefs,
    cols: &HistoryColumnWidths,
    remaining: f32,
    boundary: usize,
    delta: f32,
) {
    let [min_desc, min_date, min_author, min_hash] = history_column_min_widths();
    let remaining = remaining.max(1.0);
    let mut desc = cols.desc;
    let mut date = cols.date;
    let mut author = cols.author;
    let mut hash = cols.hash;
    match boundary {
        0 => {
            let delta = delta.clamp(min_desc - desc, date - min_date);
            desc += delta;
            date -= delta;
        }
        1 => {
            let delta = delta.clamp(min_date - date, author - min_author);
            date += delta;
            author -= delta;
        }
        _ => {
            let delta = delta.clamp(min_author - author, hash - min_hash);
            author += delta;
            hash -= delta;
        }
    }
    prefs.history_desc_pct = desc / remaining;
    prefs.history_date_pct = date / remaining;
    prefs.history_author_pct = author / remaining;
    prefs.history_hash_pct = hash / remaining;
    prefs.clamp();
}

fn history_commit_table_row(
    ui: &mut Ui,
    commit: &Commit,
    row: Option<&graph::GraphRow>,
    graph_width: f32,
    lane_count: usize,
    prefs: &LayoutPrefs,
    language: Language,
    selected: bool,
    show_remote_refs: bool,
    select_for_cherry_pick: bool,
    cherry_pick_selected: bool,
) -> (egui::Response, bool, bool) {
    let response = pointing_hand_cursor(ui.allocate_response(
        Vec2::new(ui.available_width(), HISTORY_TABLE_ROW_HEIGHT),
        Sense::click(),
    ));
    let rect = response.rect;
    let selection_width = if select_for_cherry_pick { 30.0 } else { 0.0 };
    let painter = ui.painter();
    let row_bg = if selected {
        Color32::from_rgb(42, 137, 232)
    } else if cherry_pick_selected {
        Color32::from_rgb(32, 56, 38)
    } else if response.hovered() {
        theme::accent_soft()
    } else {
        Color32::TRANSPARENT
    };
    if row_bg != Color32::TRANSPARENT {
        painter.rect_filled(rect, CornerRadius::ZERO, row_bg);
    }
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(120, 130, 148, 16)),
    );

    let mut cherry_pick_clicked = false;
    if select_for_cherry_pick {
        let checkbox_rect = Rect::from_center_size(
            Pos2::new(rect.left() + selection_width / 2.0, rect.center().y),
            Vec2::splat(16.0),
        );
        cherry_pick_clicked =
            history_cherry_pick_checkbox_at(ui, checkbox_rect, cherry_pick_selected).clicked();
    }

    let content_rect = Rect::from_min_max(
        Pos2::new(rect.left() + selection_width, rect.top()),
        rect.right_bottom(),
    );
    draw_history_graph_cell(ui, content_rect, row, graph_width, lane_count);

    let width = rect.width() - selection_width;
    let cols = history_column_widths(width, graph_width, prefs);
    let mut x = rect.left() + selection_width + graph_width;
    let text_color = if selected {
        Color32::WHITE
    } else {
        theme::text()
    };
    let muted_color = if selected {
        Color32::from_rgb(232, 245, 255)
    } else {
        theme::muted()
    };
    let y = rect.center().y;
    let desc_rect = Rect::from_min_size(
        Pos2::new(x + 4.0, rect.top()),
        Vec2::new(cols.desc - 8.0, rect.height()),
    );
    draw_history_description(ui, desc_rect, commit, selected, show_remote_refs);
    x += cols.desc;
    draw_clipped_cell(
        ui,
        x + 4.0,
        y,
        cols.date - 8.0,
        &commit.date,
        muted_color,
        false,
    );
    x += cols.date;
    draw_clipped_cell(
        ui,
        x + 4.0,
        y,
        cols.author - 8.0,
        &commit.author,
        text_color,
        false,
    );
    x += cols.author;
    let hash_rect = Rect::from_min_size(
        Pos2::new(x, rect.top()),
        Vec2::new(cols.hash.max(12.0), rect.height()),
    );
    draw_clipped_cell(
        ui,
        x + 4.0,
        y,
        cols.hash - 8.0,
        &commit.short_hash,
        text_color,
        true,
    );
    let copy_response = ui
        .interact(
            hash_rect,
            ui.id().with(("history_hash_copy", commit.hash.as_str())),
            Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(i18n::t(language, "menu.copy_hash"));
    let hash_copied = copy_response.clicked();
    if hash_copied {
        ui.ctx().copy_text(commit.hash.clone());
    }
    (response, hash_copied, cherry_pick_clicked)
}

fn draw_history_graph_cell(
    ui: &mut Ui,
    rect: Rect,
    row: Option<&graph::GraphRow>,
    graph_width: f32,
    lane_count: usize,
) {
    let Some(row) = row else {
        return;
    };
    let graph_rect = Rect::from_min_max(
        rect.left_top(),
        Pos2::new(rect.left() + graph_width, rect.bottom()),
    );
    let painter = ui
        .painter()
        .with_clip_rect(graph_rect.intersect(ui.clip_rect()));
    let left_pad = if graph_width < 48.0 { 6.0 } else { 14.0 };
    let lane_spacing = ((graph_width - left_pad - 4.0) / lane_count.max(1) as f32).clamp(7.0, 22.0);
    let lane_x = |lane: usize| graph_rect.left() + left_pad + lane as f32 * lane_spacing;
    let center_y = graph_rect.center().y;

    for lane in &row.before_lanes {
        if *lane >= lane_count {
            continue;
        }
        let x = lane_x(*lane);
        let color = theme::LANES[*lane % theme::LANES.len()];
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, center_y)],
            Stroke::new(1.35, color),
        );
    }

    for lane in &row.after_lanes {
        if *lane >= lane_count {
            continue;
        }
        if !row.before_lanes.contains(lane) && *lane != row.lane {
            continue;
        }
        let x = lane_x(*lane);
        let color = theme::LANES[*lane % theme::LANES.len()];
        painter.line_segment(
            [Pos2::new(x, center_y), Pos2::new(x, rect.bottom())],
            Stroke::new(1.35, color),
        );
    }

    for edge in &row.edges {
        if edge.kind == EdgeKind::Continue && edge.from_lane == edge.to_lane {
            continue;
        }
        let color = theme::LANES[edge.to_lane % theme::LANES.len()];
        let from = Pos2::new(lane_x(edge.from_lane), center_y);
        let to = Pos2::new(lane_x(edge.to_lane), rect.bottom());
        let stroke = Stroke::new(
            if edge.kind == EdgeKind::Continue {
                1.8
            } else {
                1.5
            },
            color,
        );
        let mid_y = center_y + (rect.bottom() - center_y) * 0.72;
        painter.add(Shape::CubicBezier(CubicBezierShape::from_points_stroke(
            [from, Pos2::new(from.x, mid_y), Pos2::new(to.x, mid_y), to],
            false,
            Color32::TRANSPARENT,
            stroke,
        )));
    }

    let node = Pos2::new(lane_x(row.lane), center_y);
    let color = theme::LANES[row.lane % theme::LANES.len()];
    painter.circle_filled(node, 2.8, color);
}

fn draw_history_description(
    ui: &mut Ui,
    rect: Rect,
    commit: &Commit,
    selected: bool,
    show_remote_refs: bool,
) {
    let painter = ui.painter().with_clip_rect(rect);
    let mut x = rect.left();
    let refs = commit_refs_for_display(commit, show_remote_refs);
    let badge_font = FontId::monospace(11.0);
    let badge_text_color = Color32::from_rgb(26, 65, 112);
    for name in refs.iter().take(3) {
        let label = history_ref_badge_label_for_width(
            ui,
            name,
            badge_font.clone(),
            badge_text_color,
            HISTORY_REF_BADGE_MAX_WIDTH - HISTORY_REF_BADGE_X_PADDING * 2.0,
        );
        let galley = ui
            .painter()
            .layout_no_wrap(label, badge_font.clone(), badge_text_color);
        let width = (galley.size().x + HISTORY_REF_BADGE_X_PADDING * 2.0)
            .clamp(HISTORY_REF_BADGE_MIN_WIDTH, HISTORY_REF_BADGE_MAX_WIDTH);
        if x + width + HISTORY_REF_BADGE_GAP > rect.right() {
            break;
        }
        let badge_rect =
            Rect::from_min_size(Pos2::new(x, rect.center().y - 8.5), Vec2::new(width, 17.0));
        let fill = if selected {
            Color32::from_rgb(232, 245, 255)
        } else if name.starts_with("tag:") {
            Color32::from_rgb(246, 226, 160)
        } else if name.starts_with("origin/") {
            Color32::from_rgb(214, 232, 255)
        } else {
            Color32::from_rgb(218, 236, 255)
        };
        painter.rect_filled(badge_rect, CornerRadius::same(3), fill);
        let text_rect = badge_rect.shrink2(Vec2::new(HISTORY_REF_BADGE_X_PADDING, 0.0));
        let text_pos = Pos2::new(
            text_rect.left(),
            text_rect.center().y - galley.size().y / 2.0,
        );
        let text_clip = text_rect.intersect(rect).intersect(ui.clip_rect());
        painter
            .with_clip_rect(text_clip)
            .galley(text_pos, galley, badge_text_color);
        x += width + HISTORY_REF_BADGE_GAP;
    }

    let subject_x = x.max(rect.left()) + 2.0;
    painter.text(
        Pos2::new(subject_x, rect.center().y),
        Align2::LEFT_CENTER,
        &commit.subject,
        FontId::proportional(12.5),
        if selected {
            Color32::WHITE
        } else {
            theme::text()
        },
    );
}

fn history_ref_badge_label_for_width(
    ui: &Ui,
    value: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
) -> String {
    let original_chars = value.chars().count();
    let mut max_chars = original_chars.min(22);
    loop {
        let label = truncate_middle(value, max_chars);
        let width = ui
            .painter()
            .layout_no_wrap(label.clone(), font.clone(), color)
            .size()
            .x;
        if width <= max_width || max_chars <= 3 {
            return label;
        }
        max_chars -= 1;
    }
}

fn commit_refs_for_display(commit: &Commit, show_remote_refs: bool) -> Vec<String> {
    commit
        .refs
        .iter()
        .filter_map(|raw| {
            let name = raw
                .trim()
                .strip_prefix("HEAD -> ")
                .unwrap_or(raw.trim())
                .strip_prefix("tag: ")
                .unwrap_or_else(|| raw.trim().strip_prefix("HEAD -> ").unwrap_or(raw.trim()))
                .to_owned();
            if !show_remote_refs && (name.starts_with("origin/") || name.contains("/origin/")) {
                return None;
            }
            (!name.is_empty()).then_some(name)
        })
        .collect()
}

fn truncate_middle(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_owned();
    }
    let keep = max_chars.saturating_sub(1);
    let head = keep / 2;
    let tail = keep - head;
    let start = value.chars().take(head).collect::<String>();
    let end = value
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{start}~{end}")
}

fn draw_clipped_cell(
    ui: &mut Ui,
    x: f32,
    y: f32,
    width: f32,
    text: &str,
    color: Color32,
    monospace: bool,
) {
    let rect = Rect::from_min_size(Pos2::new(x, y - 11.0), Vec2::new(width.max(20.0), 22.0));
    ui.painter().with_clip_rect(rect).text(
        rect.left_center(),
        Align2::LEFT_CENTER,
        text,
        if monospace {
            FontId::monospace(12.0)
        } else {
            FontId::proportional(12.0)
        },
        color,
    );
}

fn source_tree_meta_line(ui: &mut Ui, label: &str, value: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    let label_w = 58.0;
    ui.painter().text(
        Pos2::new(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.0),
        theme::muted(),
    );
    let value_rect = Rect::from_min_max(
        Pos2::new(rect.left() + label_w, rect.top()),
        rect.right_bottom(),
    );
    ui.painter()
        .with_clip_rect(value_rect.intersect(ui.clip_rect()))
        .text(
            value_rect.left_center(),
            Align2::LEFT_CENTER,
            value,
            FontId::monospace(11.0),
            theme::text(),
        );
}

fn history_file_table_header(ui: &mut Ui, language: Language) {
    let labels = if language == Language::Chinese {
        ("?", "\u{6587}\u{4ef6}\u{540d}", "\u{8def}\u{5f84}")
    } else {
        ("?", "File Name", "Path")
    };
    let width = ui.available_width();
    let (status_w, name_w, path_w) = history_file_column_widths(width);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, theme::panel_soft());
    let mut x = rect.left() + 4.0;
    for (label, col_w) in [(labels.0, status_w), (labels.1, name_w), (labels.2, path_w)] {
        ui.painter().text(
            Pos2::new(x, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(12.0),
            theme::muted(),
        );
        x += col_w;
    }
}

fn history_file_table_row(ui: &mut Ui, status: &str, path: &str, selected: bool) -> egui::Response {
    let response = pointing_hand_cursor(
        ui.allocate_response(Vec2::new(ui.available_width(), 24.0), Sense::click()),
    );
    let rect = response.rect;
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::ZERO,
            if selected {
                Color32::from_rgb(42, 137, 232)
            } else {
                theme::accent_soft()
            },
        );
    }

    let width = rect.width();
    let (status_w, name_w, path_w) = history_file_column_widths(width);
    let (file_name, dir) = split_file_display_path(path);
    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.left() + 14.0, rect.center().y),
        Vec2::splat(16.0),
    );
    draw_file_status_icon(ui, icon_rect, status, selected);
    let text_color = if selected {
        Color32::WHITE
    } else {
        theme::text()
    };
    draw_clipped_cell(
        ui,
        rect.left() + status_w + 4.0,
        rect.center().y,
        name_w - 8.0,
        &file_name,
        text_color,
        false,
    );
    draw_clipped_cell(
        ui,
        rect.left() + status_w + name_w + 4.0,
        rect.center().y,
        path_w - 8.0,
        &dir,
        if selected {
            Color32::WHITE
        } else {
            theme::muted()
        },
        false,
    );
    response
}

fn history_file_column_widths(width: f32) -> (f32, f32, f32) {
    let status_w = 28.0;
    let remaining = (width - status_w).max(80.0);
    let name_w = if remaining <= 112.0 {
        remaining * 0.55
    } else {
        (remaining * 0.42).clamp(72.0, remaining - 40.0)
    };
    let path_w = (remaining - name_w).max(40.0);
    (status_w, name_w, path_w)
}

fn split_file_display_path(path: &str) -> (String, String) {
    let normalized = path.replace('\\', "/");
    if let Some((dir, file)) = normalized.rsplit_once('/') {
        (file.to_owned(), dir.to_owned())
    } else {
        (normalized, String::new())
    }
}

fn resource_row_response(ui: &mut Ui) -> (Rect, egui::Response) {
    let response = pointing_hand_cursor(ui.allocate_response(
        Vec2::new(ui.available_width(), RESOURCE_ROW_HEIGHT),
        Sense::click(),
    ));
    let rect = response.rect;
    if response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(1.0, 1.0)),
            CornerRadius::same(3),
            theme::accent_soft(),
        );
    }
    (rect, response)
}

fn search_dimension_label(language: Language, dimension: SearchDimension) -> &'static str {
    match (language, dimension) {
        (Language::Chinese, SearchDimension::Message) => "\u{63d0}\u{4ea4}\u{4fe1}\u{606f}",
        (Language::Chinese, SearchDimension::Files) => "\u{6587}\u{4ef6}\u{53d8}\u{5316}",
        (Language::Chinese, SearchDimension::Author) => "\u{4f5c}\u{8005}",
        (_, SearchDimension::Message) => "Message",
        (_, SearchDimension::Files) => "Files",
        (_, SearchDimension::Author) => "Author",
    }
}

fn search_dimension_dropdown(
    ui: &mut Ui,
    rect: Rect,
    language: Language,
    current: SearchDimension,
    enabled: bool,
) -> Option<SearchDimension> {
    let (response, popup_id) = ui
        .add_enabled_ui(enabled, |ui| {
            history_toolbar_dropdown_button(
                ui,
                rect,
                "search_dimension",
                search_dimension_label(language, current),
            )
        })
        .inner;
    if !enabled {
        return None;
    }
    egui::popup::popup_below_widget(
        ui,
        popup_id,
        &response,
        egui::popup::PopupCloseBehavior::CloseOnClick,
        |ui| {
            ui.set_min_width(rect.width());
            let mut selected = None;
            for dimension in [
                SearchDimension::Message,
                SearchDimension::Files,
                SearchDimension::Author,
            ] {
                if history_toolbar_popup_option(
                    ui,
                    current == dimension,
                    search_dimension_label(language, dimension),
                )
                .clicked()
                {
                    selected = Some(dimension);
                }
            }
            selected
        },
    )
    .flatten()
}

fn search_submit_button(ui: &mut Ui, rect: Rect, busy: bool, enabled: bool) -> egui::Response {
    let active = enabled && !busy;
    let response = ui.allocate_rect(
        rect,
        if active {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let fill = if busy {
        theme::panel_soft()
    } else if response.hovered() && active {
        theme::accent_soft()
    } else {
        theme::panel()
    };
    ui.painter()
        .rect_filled(rect.shrink(1.0), CornerRadius::same(3), fill);

    if busy {
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
        });
    } else {
        draw_ui_icon(
            ui,
            Rect::from_center_size(rect.center(), Vec2::splat(14.0)),
            UiIcon::Search,
            if active {
                theme::accent_deep()
            } else {
                theme::muted()
            },
        );
    }
    response.on_hover_text("Search")
}

fn history_toolbar_action_button_at(
    ui: &mut Ui,
    rect: Rect,
    label: &str,
    enabled: bool,
) -> egui::Response {
    let response = ui.allocate_rect(
        rect,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let fill = if enabled && response.hovered() {
        theme::accent_soft()
    } else if enabled {
        theme::panel()
    } else {
        theme::panel_soft()
    };
    ui.painter()
        .rect_filled(rect.shrink(1.0), CornerRadius::same(3), fill);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0),
        if enabled {
            theme::text()
        } else {
            theme::muted()
        },
    );
    response.on_hover_text(label)
}

fn history_toolbar_icon_button_at(
    ui: &mut Ui,
    rect: Rect,
    icon: UiIcon,
    label: &str,
) -> egui::Response {
    let response = ui
        .allocate_rect(rect, Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(label);
    let fill = if response.hovered() {
        theme::accent_soft()
    } else {
        theme::panel()
    };
    ui.painter()
        .rect_filled(rect.shrink(1.0), CornerRadius::same(3), fill);
    draw_ui_icon(
        ui,
        Rect::from_center_size(rect.center(), Vec2::splat(14.0)),
        icon,
        theme::accent_deep(),
    );
    response
}

fn history_toolbar_label_at(ui: &mut Ui, rect: Rect, label: &str) {
    ui.painter().text(
        rect.left_center(),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.0),
        theme::muted(),
    );
}

fn history_cherry_pick_checkbox_at(ui: &mut Ui, rect: Rect, checked: bool) -> egui::Response {
    let response = ui
        .interact(
            rect,
            ui.id().with((
                "history_cherry_pick_checkbox",
                rect.left().to_bits(),
                rect.top().to_bits(),
            )),
            Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let stroke = Stroke::new(
        1.2,
        if checked {
            theme::accent()
        } else {
            theme::muted()
        },
    );
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(3),
        stroke,
        egui::StrokeKind::Inside,
    );
    if checked {
        ui.painter().line_segment(
            [
                Pos2::new(rect.left() + 3.5, rect.center().y),
                Pos2::new(rect.center().x - 1.0, rect.bottom() - 4.0),
            ],
            Stroke::new(1.7, theme::accent()),
        );
        ui.painter().line_segment(
            [
                Pos2::new(rect.center().x - 1.0, rect.bottom() - 4.0),
                Pos2::new(rect.right() - 3.0, rect.top() + 4.0),
            ],
            Stroke::new(1.7, theme::accent()),
        );
    }
    response
}

#[derive(Clone, Copy, Debug)]
struct SearchColumnWidths {
    desc: f32,
    date: f32,
    author: f32,
    hash: f32,
}

fn search_column_widths(width: f32) -> SearchColumnWidths {
    let hash = 92.0;
    let date = 132.0;
    let author = 180.0;
    let fixed = hash + date + author;
    let desc = (width - fixed).max(180.0);
    SearchColumnWidths {
        desc,
        date,
        author,
        hash,
    }
}

fn search_table_header(ui: &mut Ui, language: Language) {
    let labels = if language == Language::Chinese {
        (
            "\u{63cf}\u{8ff0}",
            "\u{65e5}\u{671f}",
            "\u{4f5c}\u{8005}",
            "\u{63d0}\u{4ea4}",
        )
    } else {
        ("Description", "Date", "Author", "Commit")
    };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::hover());
    let cols = search_column_widths(rect.width());
    let mut x = rect.left();
    let y = rect.center().y;
    draw_clipped_cell(
        ui,
        x + 6.0,
        y,
        cols.desc - 12.0,
        labels.0,
        theme::muted(),
        false,
    );
    x += cols.desc;
    draw_clipped_cell(
        ui,
        x + 6.0,
        y,
        cols.date - 12.0,
        labels.1,
        theme::muted(),
        false,
    );
    x += cols.date;
    draw_clipped_cell(
        ui,
        x + 6.0,
        y,
        cols.author - 12.0,
        labels.2,
        theme::muted(),
        false,
    );
    x += cols.author;
    draw_clipped_cell(
        ui,
        x + 6.0,
        y,
        cols.hash - 12.0,
        labels.3,
        theme::muted(),
        false,
    );
}

fn search_commit_row(ui: &mut Ui, commit: &Commit, selected: bool) -> (egui::Response, bool) {
    let response = ui.allocate_response(
        Vec2::new(ui.available_width(), HISTORY_TABLE_ROW_HEIGHT),
        Sense::click(),
    );
    let rect = response.rect;
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::ZERO,
            if selected {
                theme::accent_deep()
            } else {
                theme::accent_soft()
            },
        );
    }

    let cols = search_column_widths(rect.width());
    let text_color = if selected {
        Color32::WHITE
    } else {
        theme::text()
    };
    let muted_color = if selected {
        Color32::from_rgb(222, 247, 244)
    } else {
        theme::muted()
    };
    let mut x = rect.left();
    let y = rect.center().y;
    let hash_rect = Rect::from_min_size(
        Pos2::new(x, rect.top()),
        Vec2::new(cols.hash, rect.height()),
    );
    let copy_response = ui
        .interact(
            hash_rect,
            ui.id().with(("search_hash", &commit.hash)),
            Sense::click(),
        )
        .on_hover_text(commit.hash.as_str());
    let hash_copied = copy_response.clicked();
    if hash_copied {
        ui.ctx().copy_text(commit.hash.clone());
    }
    draw_clipped_cell(
        ui,
        x + 6.0,
        y,
        cols.desc - 12.0,
        &commit.subject,
        text_color,
        false,
    );
    x += cols.desc;
    draw_clipped_cell(
        ui,
        x + 6.0,
        y,
        cols.date - 12.0,
        &commit.relative_time,
        text_color,
        false,
    );
    x += cols.date;
    draw_clipped_cell(
        ui,
        x + 6.0,
        y,
        cols.author - 12.0,
        &commit.author,
        text_color,
        false,
    );
    x += cols.author;
    draw_clipped_cell(
        ui,
        x + 6.0,
        y,
        cols.hash - 12.0,
        &commit.short_hash,
        muted_color,
        true,
    );
    (response, hash_copied)
}

fn empty_list_panel(ui: &mut Ui, text: &str) {
    soft_panel_frame(theme::panel(), 18, 18).show(ui, |ui| {
        ui.set_min_height(160.0);
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new(text).color(theme::muted()));
        });
    });
}

fn sidebar_nav_card_width(available_width: f32, count: usize) -> f32 {
    let count = count.max(1) as f32;
    let gaps = (count - 1.0) * 6.0;
    ((available_width - gaps).max(0.0) / count)
        .min(70.0)
        .max(36.0)
}

fn sidebar_nav_card(
    ui: &mut Ui,
    width: f32,
    selected: bool,
    icon: UiIcon,
    label: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 58.0), Sense::click());
    let fill = if selected {
        theme::accent_deep()
    } else if response.hovered() {
        theme::panel()
    } else {
        theme::panel_soft()
    };
    ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.top() + 18.0),
        Vec2::splat(18.0),
    );
    egui::Image::new(icon_source(icon))
        .fit_to_exact_size(icon_rect.size())
        .tint(if selected {
            Color32::WHITE
        } else {
            theme::accent()
        })
        .paint_at(ui, icon_rect);

    let text_rect = Rect::from_min_max(
        Pos2::new(rect.left() + 4.0, rect.top() + 34.0),
        Pos2::new(rect.right() - 4.0, rect.bottom() - 4.0),
    );
    ui.painter()
        .with_clip_rect(text_rect.intersect(ui.clip_rect()))
        .text(
            text_rect.center_top(),
            Align2::CENTER_TOP,
            label,
            FontId::proportional(11.0),
            if selected {
                Color32::WHITE
            } else {
                theme::text()
            },
        );

    pointing_hand_cursor(response).on_hover_text(label)
}

#[derive(Clone, Copy)]
enum RepoTabShadowSide {
    Top,
    Left,
    Right,
}

fn paint_repo_tab_shadow(ui: &mut Ui, rect: Rect, selected: bool) {
    let shadow = egui::epaint::Shadow {
        offset: [2, 3],
        blur: 6,
        spread: 0,
        color: theme::accent_shadow(),
    };
    let base_alpha = if selected {
        shadow.color.a().max(42)
    } else {
        shadow.color.a().saturating_sub(16).max(24)
    };

    for layer in 0..3 {
        let highlight_color = repo_tab_highlight_color(base_alpha, layer);
        paint_repo_tab_shadow_side(
            ui,
            rect,
            RepoTabShadowSide::Top,
            layer,
            0.34,
            highlight_color,
        );
        paint_repo_tab_shadow_side(
            ui,
            rect,
            RepoTabShadowSide::Left,
            layer,
            0.28,
            highlight_color,
        );
        paint_repo_tab_shadow_side(
            ui,
            rect,
            RepoTabShadowSide::Right,
            layer,
            0.58,
            repo_tab_shadow_color(shadow.color, base_alpha, layer),
        );
    }
}

fn repo_tab_highlight_color(base_alpha: u8, layer: usize) -> Color32 {
    let alpha = ((base_alpha as f32 * 0.42) as u8)
        .saturating_sub((layer as u8).saturating_mul(10))
        .max(10);
    Color32::WHITE.gamma_multiply(alpha as f32 / 255.0)
}

fn repo_tab_shadow_color(shadow_color: Color32, base_alpha: u8, layer: usize) -> Color32 {
    let alpha = ((base_alpha as f32 * 0.72) as u8)
        .saturating_sub((layer as u8).saturating_mul(8))
        .max(8);
    Color32::from_rgba_unmultiplied(shadow_color.r(), shadow_color.g(), shadow_color.b(), alpha)
}

fn paint_repo_tab_shadow_side(
    ui: &mut Ui,
    rect: Rect,
    side: RepoTabShadowSide,
    layer: usize,
    side_weight: f32,
    color: Color32,
) {
    if color.a() == 0 {
        return;
    }
    let color = color.gamma_multiply(side_weight);
    ui.painter().rect_filled(
        repo_tab_shadow_rect(rect, side, layer),
        CornerRadius::same(3),
        color,
    );
}

fn repo_tab_shadow_rect(rect: Rect, side: RepoTabShadowSide, layer: usize) -> Rect {
    let highlight_spread = 0.5 + layer as f32 * 0.55;
    let shadow_spread = 1.0 + layer as f32 * 0.65;
    match side {
        RepoTabShadowSide::Top => Rect::from_min_max(
            Pos2::new(
                rect.left() - highlight_spread,
                rect.top() - highlight_spread,
            ),
            Pos2::new(rect.right() + highlight_spread, rect.top() + 0.8),
        ),
        RepoTabShadowSide::Left => Rect::from_min_max(
            Pos2::new(
                rect.left() - highlight_spread,
                rect.top() - highlight_spread,
            ),
            Pos2::new(rect.left() + 0.8, rect.bottom() + 0.6),
        ),
        RepoTabShadowSide::Right => Rect::from_min_max(
            Pos2::new(rect.right(), rect.top() + 2.0),
            Pos2::new(rect.right() + shadow_spread, rect.bottom() - 0.6),
        ),
    }
}

struct RepoTabInteraction {
    tab_clicked: bool,
    close_clicked: bool,
    response: egui::Response,
}

fn repo_tab_with_close(
    ui: &mut Ui,
    icon: UiIcon,
    selected: bool,
    label: &str,
    close_label: &str,
) -> RepoTabInteraction {
    let width = repo_tab_width(ui, label);
    let (_, response) =
        ui.allocate_exact_size(Vec2::new(width, REPO_TAB_HEIGHT), Sense::click_and_drag());
    let rect = response.rect;
    let close_rect = Rect::from_center_size(
        Pos2::new(rect.right() - 12.0, rect.center().y),
        Vec2::splat(18.0),
    );
    let close_hovered = repo_tab_close_hovered(ui, close_rect);
    let tab_hovered = response.hovered() || close_hovered;
    let show_close = selected || tab_hovered;
    let fill = repo_tab_fill(selected, tab_hovered);
    let text_color = repo_tab_text_color(selected);
    let icon_color = repo_tab_icon_color(selected);
    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.left() + 17.0, rect.center().y),
        Vec2::splat(14.0),
    );
    let text_rect = Rect::from_min_max(
        Pos2::new(rect.left() + 34.0, rect.top()),
        Pos2::new(close_rect.left() - 2.0, rect.bottom()),
    );

    paint_repo_tab_shadow(ui, rect, selected);
    ui.painter().rect_filled(rect, top_corners(3), fill);
    egui::Image::new(icon_source(icon))
        .fit_to_exact_size(icon_rect.size())
        .tint(icon_color)
        .paint_at(ui, icon_rect);
    ui.painter()
        .with_clip_rect(text_rect.intersect(ui.clip_rect()))
        .text(
            text_rect.left_center(),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(12.0),
            text_color,
        );

    let mut close_clicked = false;
    if show_close {
        let close_response = ui
            .interact(
                close_rect,
                ui.id().with(("repo_tab_close", label)),
                Sense::click(),
            )
            .on_hover_text(close_label);
        close_clicked = close_response.clicked();
        let close_hovering = close_response.hovered();
        let close_color = if close_hovering {
            theme::text()
        } else if selected {
            theme::muted()
        } else {
            Color32::WHITE
        };
        if close_hovering {
            ui.painter()
                .rect_filled(close_rect, CornerRadius::same(3), theme::panel_soft());
        }
        ui.painter().text(
            close_rect.center(),
            Align2::CENTER_CENTER,
            "\u{00d7}",
            FontId::proportional(16.0),
            close_color,
        );
    }

    RepoTabInteraction {
        tab_clicked: response.clicked() && !close_clicked,
        close_clicked,
        response,
    }
}

fn repo_tab_overflow_menu(
    ui: &mut Ui,
    id_salt: &'static str,
    label: &str,
    items: &[RepoTabVisibilityItem],
    repo_tab_names: &[String],
    active_repo_tab: Option<usize>,
    source_active: bool,
    new_tab_label: &str,
    switch_to: &mut Option<usize>,
    activate_source_tab: &mut bool,
) {
    let (response, popup_id) = repo_tab_overflow_button(ui, id_salt, label);
    egui::popup::popup_below_widget(
        ui,
        popup_id,
        &response,
        egui::popup::PopupCloseBehavior::CloseOnClick,
        |ui| {
            ui.set_min_width(220.0);
            for item in items {
                match *item {
                    RepoTabVisibilityItem::Repo(index) => {
                        if repo_tab_overflow_option(
                            ui,
                            active_repo_tab == Some(index),
                            &repo_tab_names[index],
                        )
                        .clicked()
                        {
                            *switch_to = Some(index);
                            ui.close_menu();
                        }
                    }
                    RepoTabVisibilityItem::Source => {
                        if repo_tab_overflow_option(ui, source_active, new_tab_label).clicked() {
                            *activate_source_tab = true;
                            ui.close_menu();
                        }
                    }
                }
            }
        },
    );
}

fn repo_tab_overflow_button(
    ui: &mut Ui,
    id_salt: &'static str,
    label: &str,
) -> (egui::Response, egui::Id) {
    let response = AppButton::repo_tab(UiIcon::More, label, false).show(ui);
    let popup_id = ui.make_persistent_id(("repo_tab_overflow_popup", id_salt));
    if response.clicked() {
        ui.memory_mut(|memory| memory.toggle_popup(popup_id));
    }
    (response, popup_id)
}

fn repo_tab_overflow_option(ui: &mut Ui, selected: bool, label: &str) -> egui::Response {
    let text = RichText::new(label).color(if selected {
        Color32::WHITE
    } else {
        theme::text()
    });
    pointing_hand_cursor(ui.selectable_label(selected, text))
}

fn repo_tab_close_hovered(ui: &Ui, close_rect: Rect) -> bool {
    ui.ctx()
        .pointer_hover_pos()
        .map_or(false, |pos| close_rect.contains(pos))
}

fn source_tab_button(ui: &mut Ui, selected: bool, icon: UiIcon, label: &str) -> egui::Response {
    AppButton::repo_tab(icon, label, selected).show(ui)
}

fn known_repository_row(ui: &mut Ui, repository: &KnownRepository) -> egui::Response {
    let width = ui.available_width().min(820.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 46.0), Sense::click());
    let fill = if response.hovered() {
        theme::accent_soft()
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(
        rect.shrink2(Vec2::new(2.0, 2.0)),
        CornerRadius::same(5),
        fill,
    );

    draw_ui_icon(
        ui,
        Rect::from_center_size(
            Pos2::new(rect.left() + 22.0, rect.center().y),
            Vec2::splat(18.0),
        ),
        UiIcon::Folder,
        theme::accent(),
    );
    ui.painter().text(
        Pos2::new(rect.left() + 46.0, rect.top() + 15.0),
        Align2::LEFT_CENTER,
        &repository.name,
        FontId::proportional(14.0),
        theme::text(),
    );
    ui.painter().text(
        Pos2::new(rect.left() + 46.0, rect.top() + 32.0),
        Align2::LEFT_CENTER,
        repository.root.display().to_string(),
        FontId::proportional(11.0),
        theme::muted(),
    );

    response
}

fn form_row(ui: &mut Ui, label: &str, add_contents: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.add_sized(
            [136.0, 28.0],
            egui::Label::new(RichText::new(label).color(theme::muted())),
        );
        add_contents(ui);
    });
}

fn add_known_repository(repositories: &mut Vec<KnownRepository>, path: PathBuf) {
    if !is_git_repository_dir(&path) {
        return;
    }
    let root = path.canonicalize().unwrap_or(path);
    if repositories
        .iter()
        .any(|repository| paths_equal(&repository.root, &root))
    {
        return;
    }
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Repository")
        .to_owned();
    repositories.push(KnownRepository { root, name });
}

fn scan_repository_children(base: &Path, repositories: &mut Vec<KnownRepository>) {
    let Ok(children) = fs::read_dir(base) else {
        return;
    };
    for child in children.flatten().take(240) {
        let path = child.path();
        if path.is_dir() {
            add_known_repository(repositories, path);
        }
    }
}

fn is_git_repository_dir(path: &Path) -> bool {
    path.join(".git").exists()
}

fn repo_name_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches('/');
    let last = trimmed
        .rsplit(['/', ':'])
        .next()
        .unwrap_or_default()
        .trim_end_matches(".git");
    (!last.is_empty()).then(|| last.to_owned())
}

fn settings_dialog_title_row(
    ui: &mut Ui,
    title: &str,
    width: f32,
    add_trailing: impl FnOnce(&mut Ui),
) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(width, SETTINGS_DIALOG_TITLE_HEIGHT),
        Sense::hover(),
    );
    ui.painter().rect_filled(
        rect,
        CornerRadius {
            nw: 7,
            ne: 7,
            sw: 0,
            se: 0,
        },
        theme::panel_soft(),
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        title,
        FontId::proportional(SETTINGS_DIALOG_TITLE_SIZE),
        theme::text(),
    );
    let trailing_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 48.0, rect.top() + 4.0),
        Vec2::new(40.0, 24.0),
    );
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(trailing_rect), |ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), add_trailing);
    });
}

fn settings_tab_label(language: Language, tab: SettingsTab) -> &'static str {
    match (language, tab) {
        (Language::Chinese, SettingsTab::General) => "\u{901a}\u{7528}",
        (Language::Chinese, SettingsTab::RepoRemotes) => "\u{8fdc}\u{7aef}\u{4ed3}\u{5e93}",
        (Language::Chinese, SettingsTab::RepoAdvanced) => "\u{9ad8}\u{7ea7}",
        (_, SettingsTab::General) => "General",
        (_, SettingsTab::RepoRemotes) => "Remote Repositories",
        (_, SettingsTab::RepoAdvanced) => "Advanced",
    }
}

fn repo_settings_card(ui: &mut Ui, title: &str, content: impl FnOnce(&mut Ui)) {
    soft_panel_frame(theme::panel_soft(), 12, 10)
        .stroke(Stroke::NONE)
        .shadow(panel_shadow())
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            settings_section_title(ui, title);
            content(ui);
        });
}

fn repo_settings_content_width(dialog_width: f32) -> f32 {
    safe_ui_length(dialog_width - 28.0)
}

fn repo_settings_dialog_height(tab: SettingsTab) -> f32 {
    match tab {
        SettingsTab::RepoRemotes => 330.0,
        SettingsTab::RepoAdvanced => REPO_SETTINGS_DIALOG_HEIGHT,
        SettingsTab::General => 330.0,
    }
}

fn repo_settings_content_max_height(tab: SettingsTab) -> f32 {
    match tab {
        SettingsTab::RepoRemotes => 190.0,
        SettingsTab::RepoAdvanced => 320.0,
        SettingsTab::General => 190.0,
    }
}

fn repo_settings_tab_strip(ui: &mut Ui, current: &mut SettingsTab, language: Language) {
    soft_panel_frame(theme::panel_soft(), 4, 4)
        .stroke(Stroke::NONE)
        .shadow(panel_shadow())
        .show(ui, |ui| {
            safe_set_min_size(
                ui,
                frame_inner_size(
                    REPO_SETTINGS_TAB_WIDTH * 2.0 + LAYOUT_GAP as f32,
                    REPO_SETTINGS_TABS_HEIGHT,
                    4,
                    4,
                ),
            );
            ui.allocate_ui_with_layout(
                Vec2::new(
                    REPO_SETTINGS_TAB_WIDTH * 2.0 + LAYOUT_GAP as f32,
                    REPO_SETTINGS_TAB_HEIGHT,
                ),
                Layout::left_to_right(Align::Center),
                |ui| {
                    repo_settings_tab_button(
                        ui,
                        current,
                        SettingsTab::RepoRemotes,
                        UiIcon::Globe,
                        settings_tab_label(language, SettingsTab::RepoRemotes),
                    );
                    ui.add_space(LAYOUT_GAP as f32);
                    repo_settings_tab_button(
                        ui,
                        current,
                        SettingsTab::RepoAdvanced,
                        UiIcon::Settings,
                        settings_tab_label(language, SettingsTab::RepoAdvanced),
                    );
                },
            );
        });
}

fn repo_settings_tab_button(
    ui: &mut Ui,
    current: &mut SettingsTab,
    tab: SettingsTab,
    icon: UiIcon,
    label: &str,
) -> egui::Response {
    let selected = *current == tab;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(REPO_SETTINGS_TAB_WIDTH, REPO_SETTINGS_TAB_HEIGHT),
        Sense::click(),
    );
    let fill = if selected {
        theme::panel()
    } else if response.hovered() {
        theme::hover()
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect.shrink(1.0), CornerRadius::same(5), fill);
    }

    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.left() + 15.0, rect.center().y),
        Vec2::splat(13.0),
    );
    paint_ui_icon(
        ui,
        icon_rect,
        icon,
        if selected {
            theme::accent()
        } else {
            theme::muted()
        },
    );
    let text_clip = Rect::from_min_max(
        Pos2::new(icon_rect.right() + 5.0, rect.top()),
        Pos2::new(rect.right() - 6.0, rect.bottom()),
    )
    .intersect(ui.clip_rect());
    ui.painter().with_clip_rect(text_clip).text(
        Pos2::new(icon_rect.right() + 5.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.0),
        if selected {
            theme::text()
        } else {
            theme::muted()
        },
    );
    if response.clicked() {
        *current = tab;
    }
    pointing_hand_cursor(response)
}

fn settings_section_title(ui: &mut Ui, title: &str) {
    ui.label(RichText::new(title).strong().color(theme::text()));
    ui.add_space(8.0);
}

fn settings_field(ui: &mut Ui, label: &str, content: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(34.0);
        ui.add_sized(
            [120.0, 24.0],
            egui::Label::new(RichText::new(label).color(theme::muted())),
        );
        content(ui);
    });
}

fn settings_choice_button(ui: &mut Ui, selected: bool, label: &str, width: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::click());
    let fill = if selected {
        theme::accent_deep()
    } else if response.hovered() {
        theme::accent_soft()
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(1.0, 2.0)),
            CornerRadius::same(3),
            fill,
        );
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(13.0),
        if selected {
            Color32::WHITE
        } else {
            theme::text()
        },
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn settings_accent_button(
    ui: &mut Ui,
    selected: bool,
    label: &str,
    swatch: Color32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(88.0, 30.0), Sense::click());
    let fill = if selected {
        theme::accent_deep()
    } else if response.hovered() {
        theme::accent_soft()
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(1.0, 2.0)),
            CornerRadius::same(3),
            fill,
        );
    }
    let swatch_rect = Rect::from_min_size(
        Pos2::new(rect.left() + 8.0, rect.center().y - 5.0),
        Vec2::splat(10.0),
    );
    ui.painter()
        .rect_filled(swatch_rect, CornerRadius::same(5), swatch);
    ui.painter().text(
        Pos2::new(swatch_rect.right() + 6.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        if selected {
            Color32::WHITE
        } else {
            theme::text()
        },
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn settings_checkbox_row(ui: &mut Ui, value: &mut bool, label: &str) {
    ui.horizontal(|ui| {
        ui.set_min_height(28.0);
        ui.add_space(6.0);
        ui.checkbox(value, label);
    });
}

fn tree_arrow_points(open: bool, rect: Rect) -> [Pos2; 3] {
    let center = rect.center();
    let size = 5.0;
    if open {
        [
            Pos2::new(center.x - size, center.y - 2.0),
            Pos2::new(center.x + size, center.y - 2.0),
            Pos2::new(center.x, center.y + size),
        ]
    } else {
        [
            Pos2::new(center.x - 2.0, center.y - size),
            Pos2::new(center.x - 2.0, center.y + size),
            Pos2::new(center.x + size, center.y),
        ]
    }
}

fn draw_tree_arrow(ui: &mut Ui, rect: Rect, open: bool) {
    let points = tree_arrow_points(open, rect);
    ui.painter().add(Shape::convex_polygon(
        points.to_vec(),
        theme::muted(),
        Stroke::NONE,
    ));
}

fn tree_header(ui: &mut Ui, open: &mut bool, icon: UiIcon, label: &str) -> bool {
    tree_header_inner(ui, open, icon, label, None).0
}

fn tree_header_with_action(
    ui: &mut Ui,
    open: &mut bool,
    icon: UiIcon,
    label: &str,
    action_icon: UiIcon,
    action_label: &str,
) -> (bool, bool) {
    tree_header_with_action_enabled(ui, open, icon, label, action_icon, action_label, true)
}

fn tree_header_with_action_enabled(
    ui: &mut Ui,
    open: &mut bool,
    icon: UiIcon,
    label: &str,
    action_icon: UiIcon,
    action_label: &str,
    action_enabled: bool,
) -> (bool, bool) {
    tree_header_inner(
        ui,
        open,
        icon,
        label,
        Some((action_icon, action_label, action_enabled)),
    )
}

fn tree_header_inner(
    ui: &mut Ui,
    open: &mut bool,
    icon: UiIcon,
    label: &str,
    action: Option<(UiIcon, &str, bool)>,
) -> (bool, bool) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
    let response = pointing_hand_cursor(response);
    let mut action_clicked = false;
    allocate_clipped_ui_at_rect(ui, rect, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            let (arrow_rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 18.0), Sense::hover());
            draw_tree_arrow(ui, arrow_rect, *open);
            let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
            draw_ui_icon(ui, icon_rect, icon, theme::accent());
            let label_width = if action.is_some() {
                (ui.available_width() - 38.0).max(32.0)
            } else {
                ui.available_width().max(32.0)
            };
            ui.add_sized(
                [label_width, 24.0],
                egui::Label::new(RichText::new(label).strong().color(theme::text())).truncate(),
            );
            if let Some((action_icon, action_label, action_enabled)) = action {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let response = icon_button(ui, action_icon, action_label, action_enabled);
                    action_clicked = response.clicked();
                });
            }
        });
    });
    if response.clicked() && !action_clicked {
        *open = !*open;
    }
    (*open, action_clicked)
}

fn tree_empty(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(30.0);
        ui.label(RichText::new(text).color(theme::muted()));
    });
}

fn worktree_conflict_files(snapshot: &RepositorySnapshot) -> Vec<WorktreeFile> {
    let mut files = BTreeMap::new();
    for file in snapshot.staged.iter().chain(snapshot.unstaged.iter()) {
        if file.is_conflicted() {
            files
                .entry(file.path.clone())
                .or_insert_with(|| file.clone());
        }
    }
    files.into_values().collect()
}

fn selected_or_first_conflict<'a>(
    conflicts: &'a [WorktreeFile],
    selected: Option<&SelectedWorktreeFile>,
) -> Option<&'a WorktreeFile> {
    selected
        .and_then(|selected| conflicts.iter().find(|file| file.path == selected.path))
        .or_else(|| conflicts.first())
}

fn worktree_header_action_button(
    ui: &mut Ui,
    icon: Option<UiIcon>,
    label: &str,
    enabled: bool,
) -> egui::Response {
    let height = 30.0;
    let text_width = ui.fonts(|fonts| {
        fonts
            .layout_no_wrap(label.to_owned(), FontId::proportional(13.0), theme::text())
            .rect
            .width()
    });
    let width = (text_width + if icon.is_some() { 38.0 } else { 24.0 }).clamp(88.0, 132.0);
    let text = RichText::new(label).size(13.0).color(if enabled {
        if icon == Some(UiIcon::Warning) {
            Color32::from_rgb(232, 174, 55)
        } else {
            theme::text()
        }
    } else {
        theme::muted()
    });
    let button = if let Some(icon) = icon {
        egui::Button::image_and_text(
            egui::Image::new(icon_source(icon))
                .fit_to_exact_size(Vec2::splat(14.0))
                .tint(if icon == UiIcon::Warning {
                    Color32::from_rgb(232, 174, 55)
                } else {
                    theme::text()
                }),
            text,
        )
    } else {
        egui::Button::new(text)
    }
    .min_size(Vec2::new(width, height))
    .fill(theme::panel_soft())
    .stroke(Stroke::NONE)
    .corner_radius(CornerRadius::same(4));

    let response = ui.add_enabled(enabled, button);
    if enabled {
        pointing_hand_cursor(response)
    } else {
        response
    }
}

fn conflict_resolution_dialog_background() -> Color32 {
    theme::bg()
}

fn conflict_resolution_modal_rect(ctx: &egui::Context) -> Rect {
    let screen = ctx.screen_rect();
    let center = screen.center();
    let min = Pos2::new(
        center.x - CONFLICT_MODAL_SIZE.x / 2.0,
        center.y - CONFLICT_MODAL_SIZE.y / 2.0,
    );
    Rect::from_min_size(min, CONFLICT_MODAL_SIZE)
}

fn conflict_resolution_list_panel(
    ui: &mut Ui,
    panel_size: Vec2,
    conflict_files: &[WorktreeFile],
    language: Language,
    selected_path: &mut Option<String>,
) {
    egui::Frame::new()
        .fill(theme::panel())
        .corner_radius(CornerRadius::same(6))
        .shadow(panel_shadow())
        .inner_margin(egui::Margin::symmetric(10, 10))
        .show(ui, |ui| {
            safe_set_min_size(ui, frame_inner_size(panel_size.x, panel_size.y, 10, 10));
            conflict_resolution_header(ui, language);
            ui.add_space(8.0);
            if conflict_files.is_empty() {
                ui.add_space(16.0);
                ui.label(
                    RichText::new(i18n::t(language, "worktree.conflicts.empty"))
                        .color(theme::muted()),
                );
            } else {
                ScrollArea::vertical()
                    .max_height((panel_size.y - 48.0).max(80.0))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for file in conflict_files {
                            let selected = selected_path.as_deref() == Some(file.path.as_str());
                            if conflict_resolution_row(ui, file, selected).clicked() {
                                *selected_path = Some(file.path.clone());
                            }
                        }
                    });
            }
        });
}

fn conflict_resolution_actions_panel(
    ui: &mut Ui,
    panel_size: Vec2,
    language: Language,
    has_selection: bool,
) -> Option<ConflictResolutionDialogAction> {
    let mut action = None;
    egui::Frame::new()
        .fill(theme::panel())
        .corner_radius(CornerRadius::same(6))
        .shadow(panel_shadow())
        .inner_margin(egui::Margin::symmetric(10, 10))
        .show(ui, |ui| {
            safe_set_min_size(ui, frame_inner_size(panel_size.x, panel_size.y, 10, 10));
            ui.add_space(2.0);
            if conflict_resolution_action_button(
                ui,
                i18n::t(language, "worktree.accept_yours"),
                has_selection,
            )
            .clicked()
            {
                action = Some(ConflictResolutionDialogAction::Accept(
                    git::ConflictSide::Ours,
                ));
            }
            ui.add_space(8.0);
            if conflict_resolution_action_button(
                ui,
                i18n::t(language, "worktree.accept_theirs"),
                has_selection,
            )
            .clicked()
            {
                action = Some(ConflictResolutionDialogAction::Accept(
                    git::ConflictSide::Theirs,
                ));
            }
            ui.add_space(8.0);
            if conflict_resolution_action_button(
                ui,
                i18n::t(language, "worktree.merge"),
                has_selection,
            )
            .clicked()
            {
                action = Some(ConflictResolutionDialogAction::Merge);
            }
        });
    action
}

fn conflict_resolution_action_button(ui: &mut Ui, text: &str, enabled: bool) -> egui::Response {
    let button = egui::Button::new(text)
        .min_size(CONFLICT_ACTION_BUTTON_SIZE)
        .fill(theme::panel_soft())
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(4));
    let response = ui.add_enabled(enabled, button);
    if enabled {
        pointing_hand_cursor(response)
    } else {
        response
    }
}

fn conflict_resolution_header(ui: &mut Ui, language: Language) {
    let (name, yours, theirs) = if language == Language::Chinese {
        ("名称", "本地", "远端")
    } else {
        ("Name", "Yours", "Theirs")
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(name).strong().color(theme::muted()));
        ui.add_space(300.0);
        ui.label(RichText::new(yours).strong().color(theme::muted()));
        ui.add_space(36.0);
        ui.label(RichText::new(theirs).strong().color(theme::muted()));
    });
}

fn conflict_resolution_row(ui: &mut Ui, file: &WorktreeFile, selected: bool) -> egui::Response {
    let response = pointing_hand_cursor(
        ui.allocate_response(Vec2::new(ui.available_width(), 26.0), Sense::click()),
    );
    let rect = response.rect;
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(3),
            if selected {
                theme::accent_deep()
            } else {
                theme::accent_soft()
            },
        );
    }
    let text_color = if selected {
        Color32::WHITE
    } else {
        theme::text()
    };
    draw_ui_icon(
        ui,
        Rect::from_center_size(
            Pos2::new(rect.left() + 16.0, rect.center().y),
            Vec2::splat(14.0),
        ),
        UiIcon::Warning,
        Color32::from_rgb(232, 174, 55),
    );
    draw_clipped_cell(
        ui,
        rect.left() + 34.0,
        rect.center().y,
        (rect.width() - 220.0).max(120.0),
        &file.display_path,
        text_color,
        false,
    );
    ui.painter().text(
        Pos2::new(rect.right() - 150.0, rect.center().y),
        Align2::LEFT_CENTER,
        "Modified",
        FontId::proportional(12.0),
        if selected {
            Color32::WHITE
        } else {
            theme::muted()
        },
    );
    ui.painter().text(
        Pos2::new(rect.right() - 74.0, rect.center().y),
        Align2::LEFT_CENTER,
        "Modified",
        FontId::proportional(12.0),
        if selected {
            Color32::WHITE
        } else {
            theme::muted()
        },
    );
    response
}

fn merge_theme_arg(theme_mode: theme::ThemeMode) -> &'static str {
    match theme_mode {
        theme::ThemeMode::Dark => "dark",
        theme::ThemeMode::Light => "light",
    }
}

fn merge_language_arg(language: Language) -> &'static str {
    match language {
        Language::English => "en",
        Language::Chinese => "zh",
    }
}

fn worktree_table(
    ui: &mut Ui,
    title: &str,
    files: &[WorktreeFile],
    staged: bool,
    height: f32,
    language: Language,
    selection: &WorktreeSelectionState,
    display_mode: WorktreeDisplayMode,
    collapsed_dirs: &mut HashSet<String>,
    action: &mut Option<WorktreeMenuAction>,
    selected: &mut Option<WorktreeRowClick>,
) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let panel_rect = rect;
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(panel_rect), |ui| {
        ui.set_clip_rect(workspace_card_clip_rect(rect));
        workspace_card_frame(10, 8).show(ui, |ui| {
            safe_set_min_size(
                ui,
                frame_inner_size(panel_rect.width(), panel_rect.height(), 10, 8),
            );
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).strong().color(theme::text()));
                ui.label(RichText::new(format!("({})", files.len())).color(theme::muted()));
            });
            ui.add_space(8.0);
            if files.is_empty() {
                ui.add_space(20.0);
                ui.label(RichText::new("-").color(theme::muted()));
            } else {
                ScrollArea::vertical()
                    .id_salt(("worktree_table_scroll", staged, title))
                    .max_height((height - 44.0).max(60.0))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_width((width - 28.0).max(80.0));
                        match display_mode {
                            WorktreeDisplayMode::Flat => {
                                for file in files {
                                    let row_selected = selection.contains(staged, &file.path);
                                    let response = worktree_file_row(
                                        ui,
                                        file,
                                        staged,
                                        row_selected,
                                        language,
                                        action,
                                        0,
                                        &file.display_path,
                                    );
                                    if response.clicked() {
                                        select_worktree_row(ui, file, staged, selected);
                                    }
                                }
                            }
                            WorktreeDisplayMode::Tree => {
                                for row in worktree_tree_rows(files, collapsed_dirs) {
                                    match row {
                                        WorktreeTreeRow::Directory { path, depth } => {
                                            let response = worktree_directory_row(
                                                ui,
                                                &path,
                                                depth,
                                                collapsed_dirs.contains(&path),
                                                language,
                                                action,
                                            );
                                            if response.clicked() {
                                                if !collapsed_dirs.insert(path.clone()) {
                                                    collapsed_dirs.remove(&path);
                                                }
                                            }
                                        }
                                        WorktreeTreeRow::File { file, depth } => {
                                            let row_selected =
                                                selection.contains(staged, &file.path);
                                            let label = worktree_path_basename(&file.display_path);
                                            let response = worktree_file_row(
                                                ui,
                                                file,
                                                staged,
                                                row_selected,
                                                language,
                                                action,
                                                depth,
                                                label,
                                            );
                                            if response.clicked() {
                                                select_worktree_row(ui, file, staged, selected);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    });
            }
        });
        paint_workspace_card_inset_shadow(ui, panel_rect);
    });
}

fn clean_worktree_state(ui: &mut Ui, text: &str, detail: &str) {
    let (rect, _) = ui.allocate_exact_size(safe_ui_size(ui.available_size()), Sense::hover());
    let center = rect.center();
    ui.painter().text(
        Pos2::new(center.x, center.y - 16.0),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(22.0),
        theme::text(),
    );
    ui.painter().text(
        Pos2::new(center.x, center.y + 18.0),
        Align2::CENTER_CENTER,
        detail,
        FontId::proportional(13.0),
        theme::muted(),
    );
}

fn branch_table_row(
    ui: &mut Ui,
    current: bool,
    remote: bool,
    name: &str,
    upstream: Option<&str>,
    remote_branch_names: &[String],
    remotes: &[String],
    language: Language,
    enabled: bool,
    action: &mut Option<BranchMenuAction>,
) -> egui::Response {
    let (rect, response) = resource_row_response(ui);
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        let width = ui.available_width();
        let status_w = 110.0;
        let type_w = 120.0;
        let name_w = (width - status_w - type_w).max(220.0);
        let scope = if remote {
            i18n::t(language, "common.remote")
        } else {
            i18n::t(language, "common.local")
        };
        let status = if current {
            i18n::t(language, "branch.current")
        } else {
            ""
        };
        ui.horizontal(|ui| {
            ui.add_sized(
                [name_w, RESOURCE_ROW_HEIGHT],
                egui::Label::new(RichText::new(name).color(if current {
                    theme::text()
                } else {
                    theme::muted()
                }))
                .truncate(),
            );
            ui.add_sized(
                [type_w, RESOURCE_ROW_HEIGHT],
                egui::Label::new(RichText::new(scope).small().color(if remote {
                    theme::info()
                } else {
                    theme::accent()
                })),
            );
            ui.add_sized(
                [status_w, RESOURCE_ROW_HEIGHT],
                egui::Label::new(RichText::new(status).small().color(theme::accent())),
            );
        });
    });
    branch_context_menu(
        response,
        current,
        remote,
        name,
        upstream,
        remote_branch_names,
        remotes,
        language,
        enabled,
        action,
    )
}

fn branch_context_menu(
    response: egui::Response,
    current: bool,
    remote: bool,
    name: &str,
    upstream: Option<&str>,
    remote_branch_names: &[String],
    remotes: &[String],
    language: Language,
    enabled: bool,
    action: &mut Option<BranchMenuAction>,
) -> egui::Response {
    response.context_menu(|ui| {
        ui.set_min_width(if remote { 220.0 } else { 270.0 });
        if remote {
            ui.label(RichText::new(name).color(theme::text()));
            ui.separator();
            if ui
                .add_enabled(
                    enabled,
                    egui::Button::new(i18n::t(language, "branch.checkout_remote")),
                )
                .clicked()
            {
                *action = Some(BranchMenuAction::CheckoutRemote {
                    remote_branch: name.to_owned(),
                });
                ui.close_menu();
            }
            if ui
                .add_enabled(
                    enabled,
                    egui::Button::new(i18n::t(language, "branch.delete_remote")),
                )
                .clicked()
            {
                *action = Some(BranchMenuAction::DeleteRemote {
                    remote_branch: name.to_owned(),
                });
                ui.close_menu();
            }
        } else {
            if ui
                .add_enabled(
                    enabled && !current,
                    egui::Button::new(branch_checkout_menu_label(language, name)),
                )
                .clicked()
            {
                *action = Some(BranchMenuAction::Checkout {
                    name: name.to_owned(),
                });
                ui.close_menu();
            }
            if ui
                .add_enabled(
                    enabled && !current,
                    egui::Button::new(branch_merge_menu_label(language, name)),
                )
                .clicked()
            {
                *action = Some(BranchMenuAction::MergeIntoCurrent {
                    name: name.to_owned(),
                });
                ui.close_menu();
            }
            if ui
                .add_enabled(
                    enabled && !current,
                    egui::Button::new(branch_rebase_menu_label(language, name)),
                )
                .clicked()
            {
                *action = Some(BranchMenuAction::RebaseCurrentOnto {
                    name: name.to_owned(),
                });
                ui.close_menu();
            }
            ui.separator();
            if ui
                .add_enabled(
                    enabled && upstream.is_some(),
                    egui::Button::new(branch_fetch_menu_label(language, name)),
                )
                .clicked()
            {
                if let Some(remote_branch) = upstream {
                    *action = Some(BranchMenuAction::FetchTracked {
                        remote_branch: remote_branch.to_owned(),
                    });
                    ui.close_menu();
                }
            }
            ui.separator();
            if ui
                .add_enabled(
                    enabled && current && upstream.is_some(),
                    egui::Button::new(i18n::t(language, "branch.pull_tracked")),
                )
                .clicked()
            {
                *action = Some(BranchMenuAction::PullTracked {
                    name: name.to_owned(),
                });
                ui.close_menu();
            }
            if ui
                .add_enabled(
                    enabled && current && upstream.is_some(),
                    egui::Button::new(i18n::t(language, "branch.push_tracked")),
                )
                .clicked()
            {
                *action = Some(BranchMenuAction::PushTracked {
                    name: name.to_owned(),
                });
                ui.close_menu();
            }
            ui.add_enabled_ui(enabled && !remotes.is_empty(), |ui| {
                ui.menu_button(i18n::t(language, "branch.push_to"), |ui| {
                    if remotes.is_empty() {
                        ui.add_enabled(
                            false,
                            egui::Button::new(i18n::t(language, "branch.no_remotes")),
                        );
                    } else {
                        for remote in remotes {
                            if ui.button(remote).clicked() {
                                *action = Some(BranchMenuAction::PushToRemote {
                                    name: name.to_owned(),
                                    remote: remote.clone(),
                                });
                                ui.close_menu();
                            }
                        }
                    }
                });
            });
            ui.add_enabled_ui(enabled, |ui| {
                ui.menu_button(i18n::t(language, "branch.track_remote"), |ui| {
                    for remote_branch in remote_branch_names {
                        let selected = upstream == Some(remote_branch.as_str());
                        if ui
                            .add_enabled(
                                !selected,
                                egui::Button::new(branch_tracking_menu_label(
                                    selected,
                                    remote_branch,
                                )),
                            )
                            .clicked()
                        {
                            *action = Some(BranchMenuAction::TrackRemote {
                                name: name.to_owned(),
                                remote_branch: Some(remote_branch.clone()),
                            });
                            ui.close_menu();
                        }
                    }
                    if !remote_branch_names.is_empty() {
                        ui.separator();
                    }
                    let selected = upstream.is_none();
                    if ui
                        .add_enabled(
                            !selected,
                            egui::Button::new(branch_tracking_menu_label(
                                selected,
                                i18n::t(language, "branch.no_remote_tracking"),
                            )),
                        )
                        .clicked()
                    {
                        *action = Some(BranchMenuAction::TrackRemote {
                            name: name.to_owned(),
                            remote_branch: None,
                        });
                        ui.close_menu();
                    }
                });
            });
            ui.separator();
            if ui
                .add_enabled(
                    enabled && !current,
                    egui::Button::new(i18n::t(language, "branch.compare_with_current")),
                )
                .clicked()
            {
                *action = Some(BranchMenuAction::CompareWithCurrent {
                    name: name.to_owned(),
                });
                ui.close_menu();
            }
            ui.separator();
            if ui
                .add_enabled(
                    enabled && !current,
                    egui::Button::new(branch_rename_menu_label(language, name)),
                )
                .clicked()
            {
                *action = Some(BranchMenuAction::Rename {
                    name: name.to_owned(),
                });
                ui.close_menu();
            }
            if ui
                .add_enabled(
                    enabled && !current,
                    egui::Button::new(branch_delete_menu_label(language, name)),
                )
                .clicked()
            {
                *action = Some(BranchMenuAction::Delete {
                    name: name.to_owned(),
                });
                ui.close_menu();
            }
            ui.separator();
            if ui
                .add_enabled(
                    enabled && !remotes.is_empty(),
                    egui::Button::new(i18n::t(language, "branch.create_pull_request")),
                )
                .clicked()
            {
                *action = Some(BranchMenuAction::CreatePullRequest {
                    name: name.to_owned(),
                });
                ui.close_menu();
            }
        }
    });
    response
}

fn branch_checkout_menu_label(language: Language, name: &str) -> String {
    match language {
        Language::Chinese => format!("\u{68c0}\u{51fa} {name}..."),
        Language::English => format!("Checkout {name}..."),
    }
}

fn branch_merge_menu_label(language: Language, name: &str) -> String {
    match language {
        Language::Chinese => {
            format!("\u{5408}\u{5e76} {name} \u{81f3}\u{5f53}\u{524d}\u{5206}\u{652f}")
        }
        Language::English => format!("Merge {name} into current branch"),
    }
}

fn branch_rebase_menu_label(language: Language, name: &str) -> String {
    match language {
        Language::Chinese => {
            format!("\u{5c06}\u{5f53}\u{524d}\u{53d8}\u{66f4}\u{53d8}\u{57fa}\u{5230} {name}")
        }
        Language::English => format!("Rebase current changes onto {name}"),
    }
}

fn branch_fetch_menu_label(language: Language, name: &str) -> String {
    match language {
        Language::Chinese => format!("\u{83b7}\u{53d6} {name}"),
        Language::English => format!("Fetch {name}"),
    }
}

fn branch_rename_menu_label(language: Language, name: &str) -> String {
    match language {
        Language::Chinese => format!("\u{91cd}\u{547d}\u{540d} {name}..."),
        Language::English => format!("Rename {name}..."),
    }
}

fn branch_delete_menu_label(language: Language, name: &str) -> String {
    match language {
        Language::Chinese => format!("\u{5220}\u{9664} {name}"),
        Language::English => format!("Delete {name}"),
    }
}

fn branch_tracking_menu_label(selected: bool, name: &str) -> String {
    if selected {
        format!("\u{2713} {name}")
    } else {
        format!("  {name}")
    }
}

fn tag_table_row(
    ui: &mut Ui,
    tag: &Tag,
    language: Language,
    action: &mut Option<TagMenuAction>,
) -> egui::Response {
    let (rect, response) = resource_row_response(ui);
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        let columns = tag_table_columns(ui.available_width());
        ui.horizontal(|ui| {
            tag_table_cell(
                ui,
                columns.name,
                RichText::new(&tag.name).color(theme::accent()),
            );
            tag_table_cell(
                ui,
                columns.target,
                RichText::new(&tag.target)
                    .monospace()
                    .small()
                    .color(theme::muted()),
            );
            tag_table_cell(
                ui,
                columns.subject,
                RichText::new(&tag.subject).small().color(theme::text()),
            );
            ui.allocate_ui(Vec2::new(columns.action, RESOURCE_ROW_HEIGHT), |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_sized(
                            [54.0, 22.0],
                            egui::Button::new(
                                RichText::new(i18n::t(language, "tag.delete")).small(),
                            ),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        *action = Some(TagMenuAction::Delete {
                            name: tag.name.clone(),
                        });
                    }
                    if ui
                        .add_sized(
                            [54.0, 22.0],
                            egui::Button::new(RichText::new(i18n::t(language, "tag.push")).small()),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        *action = Some(TagMenuAction::Push {
                            name: tag.name.clone(),
                        });
                    }
                });
            });
        });
    });
    tag_context_menu(response, tag, language, action)
}

fn tag_context_menu(
    response: egui::Response,
    tag: &Tag,
    language: Language,
    action: &mut Option<TagMenuAction>,
) -> egui::Response {
    response.context_menu(|ui| {
        ui.set_min_width(180.0);
        ui.label(RichText::new(&tag.name).color(theme::text()));
        ui.separator();
        if ui.button(i18n::t(language, "tag.checkout")).clicked() {
            *action = Some(TagMenuAction::Checkout {
                name: tag.name.clone(),
            });
            ui.close_menu();
        }
        if ui.button(i18n::t(language, "tag.push")).clicked() {
            *action = Some(TagMenuAction::Push {
                name: tag.name.clone(),
            });
            ui.close_menu();
        }
        if ui.button(i18n::t(language, "tag.delete")).clicked() {
            *action = Some(TagMenuAction::Delete {
                name: tag.name.clone(),
            });
            ui.close_menu();
        }
    });
    response
}

fn stash_table_row(
    ui: &mut Ui,
    stash: &StashEntry,
    language: Language,
    action: &mut Option<StashMenuAction>,
) -> egui::Response {
    let (rect, response) = resource_row_response(ui);
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        let width = ui.available_width();
        let stash_w = 150.0;
        let when_w = 132.0;
        let message_w = (width - stash_w - when_w).max(220.0);
        ui.horizontal(|ui| {
            ui.add_sized(
                [stash_w, RESOURCE_ROW_HEIGHT],
                egui::Label::new(
                    RichText::new(&stash.selector)
                        .monospace()
                        .color(theme::accent()),
                )
                .truncate(),
            );
            ui.add_sized(
                [message_w, RESOURCE_ROW_HEIGHT],
                egui::Label::new(RichText::new(&stash.message).color(theme::text())).truncate(),
            );
            ui.add_sized(
                [when_w, RESOURCE_ROW_HEIGHT],
                egui::Label::new(
                    RichText::new(&stash.relative_time)
                        .small()
                        .color(theme::muted()),
                )
                .truncate(),
            );
        });
    });
    stash_context_menu(response, stash, language, action)
}

fn stash_context_menu(
    response: egui::Response,
    stash: &StashEntry,
    language: Language,
    action: &mut Option<StashMenuAction>,
) -> egui::Response {
    response.context_menu(|ui| {
        ui.set_min_width(180.0);
        ui.label(
            RichText::new(&stash.selector)
                .monospace()
                .color(theme::text()),
        );
        ui.separator();
        if ui.button(i18n::t(language, "stash.apply")).clicked() {
            *action = Some(StashMenuAction::Apply {
                selector: stash.selector.clone(),
            });
            ui.close_menu();
        }
        if ui.button(i18n::t(language, "stash.pop")).clicked() {
            *action = Some(StashMenuAction::Pop {
                selector: stash.selector.clone(),
            });
            ui.close_menu();
        }
        if ui.button(i18n::t(language, "stash.drop")).clicked() {
            *action = Some(StashMenuAction::Drop {
                selector: stash.selector.clone(),
                message: stash.message.clone(),
            });
            ui.close_menu();
        }
    });
    response
}

fn branch_row(
    ui: &mut Ui,
    current: bool,
    remote: bool,
    name: &str,
    display_name: &str,
    depth: usize,
    upstream: Option<&str>,
    sync_counts: Option<UpstreamSyncCounts>,
    remote_branch_names: &[String],
    remotes: &[String],
    language: Language,
    enabled: bool,
    action: &mut Option<BranchMenuAction>,
) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::hover());
    let row_rect = rect.shrink2(Vec2::new(2.0, 1.0));
    if current {
        ui.painter()
            .rect_filled(row_rect, CornerRadius::same(4), theme::accent_soft());
        ui.painter().rect_filled(
            branch_current_indicator_rect(row_rect),
            CornerRadius::same(2),
            theme::accent_deep(),
        );
    } else if enabled && row_rect_hovered(ui, rect) {
        ui.painter()
            .rect_filled(row_rect, CornerRadius::same(4), theme::hover());
    }

    let color = if current {
        theme::accent()
    } else if remote {
        theme::info()
    } else {
        theme::muted()
    };
    let badge_left =
        paint_branch_row_badges(ui, row_rect, current, remote, sync_counts, language, color)
            .unwrap_or(row_rect.right());
    let name_left = rect.left() + if current { 22.0 } else { 16.0 } + depth as f32 * 14.0;
    let name_right = (badge_left - 6.0).max(name_left);
    let name_rect = Rect::from_min_max(
        Pos2::new(name_left, rect.top()),
        Pos2::new(name_right, rect.bottom()),
    );
    let mut name_text = RichText::new(display_name).color(if current {
        theme::text()
    } else {
        theme::muted()
    });
    if current {
        name_text = name_text.strong();
    }
    paint_branch_name(ui, name_rect, name_text);

    let response = full_row_click_response_enabled(ui, rect, ("branch_row", remote, name), enabled);
    if enabled && response.double_clicked() {
        if remote {
            *action = Some(BranchMenuAction::CheckoutRemote {
                remote_branch: name.to_owned(),
            });
        } else if !current {
            *action = Some(BranchMenuAction::Checkout {
                name: name.to_owned(),
            });
        }
    }

    branch_context_menu(
        response,
        current,
        remote,
        name,
        upstream,
        remote_branch_names,
        remotes,
        language,
        enabled,
        action,
    )
}

fn branch_current_indicator_rect(row_rect: Rect) -> Rect {
    Rect::from_min_max(
        Pos2::new(row_rect.left() + 7.0, row_rect.top() + 5.0),
        Pos2::new(row_rect.left() + 10.0, row_rect.bottom() - 5.0),
    )
}

fn paint_branch_name(ui: &mut Ui, name_rect: Rect, name_text: RichText) {
    let mut layout_job = egui::text::LayoutJob::default();
    name_text.append_to(
        &mut layout_job,
        ui.style(),
        egui::FontSelection::Default,
        Align::Min,
    );
    layout_job.wrap.max_width = name_rect.width();
    layout_job.wrap.max_rows = 1;
    layout_job.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(layout_job);
    let text_pos = Align2::LEFT_CENTER
        .anchor_size(name_rect.left_center(), galley.size())
        .min;
    ui.painter()
        .with_clip_rect(name_rect)
        .galley(text_pos, galley, theme::muted());
}

fn paint_branch_row_badges(
    ui: &mut Ui,
    row_rect: Rect,
    current: bool,
    remote: bool,
    sync_counts: Option<UpstreamSyncCounts>,
    language: Language,
    remote_color: Color32,
) -> Option<f32> {
    let mut right = row_rect.right() - BRANCH_CURRENT_BADGE_RIGHT_GAP;
    let mut painted = false;
    if let Some(counts) = sync_counts {
        if let Some(label) = upstream_pull_badge(Some(counts)) {
            right = paint_branch_badge_at(ui, row_rect, right, &label, theme::accent_deep());
            painted = true;
        }
        if let Some(label) = upstream_push_badge(Some(counts)) {
            right = paint_branch_badge_at(ui, row_rect, right, &label, theme::text());
            painted = true;
        }
    }
    if current {
        right = paint_branch_current_badge_at(
            ui,
            row_rect,
            right,
            i18n::t(language, "branch.current_badge"),
        );
        painted = true;
    }
    if remote {
        right = paint_branch_text_badge_at(
            ui,
            row_rect,
            right,
            i18n::t(language, "common.remote"),
            remote_color,
        );
        painted = true;
    }
    painted.then_some(right + 4.0)
}

fn paint_branch_current_badge_at(ui: &mut Ui, row_rect: Rect, right: f32, label: &str) -> f32 {
    paint_branch_badge_at(ui, row_rect, right, label, theme::accent_deep())
}

fn paint_branch_badge_at(
    ui: &mut Ui,
    row_rect: Rect,
    right: f32,
    label: &str,
    fill: Color32,
) -> f32 {
    let text_width = ui.fonts(|fonts| {
        fonts
            .layout_no_wrap(label.to_owned(), FontId::proportional(10.5), Color32::WHITE)
            .rect
            .width()
    });
    let width = (text_width + 12.0).max(26.0);
    let rect = Rect::from_center_size(
        Pos2::new(
            right - width / 2.0,
            row_rect.center().y + BRANCH_CURRENT_BADGE_Y_OFFSET,
        ),
        Vec2::new(width, 17.0),
    );
    ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(10.5),
        Color32::WHITE,
    );
    rect.left() - 4.0
}

fn paint_branch_text_badge_at(
    ui: &mut Ui,
    row_rect: Rect,
    right: f32,
    label: &str,
    color: Color32,
) -> f32 {
    let text_width = ui.fonts(|fonts| {
        fonts
            .layout_no_wrap(label.to_owned(), FontId::proportional(11.0), color)
            .rect
            .width()
    });
    let center = Pos2::new(
        right - text_width / 2.0,
        row_rect.center().y + BRANCH_CURRENT_BADGE_Y_OFFSET,
    );
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0),
        color,
    );
    right - text_width - 4.0
}

#[derive(Clone, Debug, Default)]
struct BranchTreeNode {
    name: String,
    path: String,
    full_name: Option<String>,
    children: BTreeMap<String, BranchTreeNode>,
}

impl BranchTreeNode {
    fn new(name: String, path: String) -> Self {
        Self {
            name,
            path,
            full_name: None,
            children: BTreeMap::new(),
        }
    }
}

fn local_branch_tree(branches: &[&git::Branch]) -> Vec<BranchTreeNode> {
    let mut roots = BTreeMap::<String, BranchTreeNode>::new();
    for branch in branches.iter().filter(|branch| !branch.remote) {
        let segments = branch
            .name
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        if segments.is_empty() {
            continue;
        }
        insert_branch_node(&mut roots, &segments, &branch.name, String::new());
    }
    roots.into_values().collect()
}

fn remote_branch_tree(branches: &[&git::Branch]) -> Vec<BranchTreeNode> {
    let mut roots = BTreeMap::<String, BranchTreeNode>::new();
    for branch in branches.iter().filter(|branch| branch.remote) {
        let segments = branch
            .name
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        if segments.len() < 2 {
            continue;
        }
        insert_branch_node(&mut roots, &segments, &branch.name, String::new());
    }
    roots.into_values().collect()
}

fn insert_branch_node(
    nodes: &mut BTreeMap<String, BranchTreeNode>,
    segments: &[&str],
    full_name: &str,
    parent_path: String,
) {
    let Some((segment, rest)) = segments.split_first() else {
        return;
    };
    let path = if parent_path.is_empty() {
        (*segment).to_owned()
    } else {
        format!("{parent_path}/{segment}")
    };
    let node = nodes
        .entry((*segment).to_owned())
        .or_insert_with(|| BranchTreeNode::new((*segment).to_owned(), path.clone()));
    if rest.is_empty() {
        node.full_name = Some(full_name.to_owned());
    } else {
        insert_branch_node(&mut node.children, rest, full_name, path);
    }
}

fn local_branch_tree_rows(
    ui: &mut Ui,
    node: &BranchTreeNode,
    depth: usize,
    branches_by_name: &HashMap<&str, &git::Branch>,
    pending_checkout: Option<&str>,
    remote_branch_names: &[String],
    remotes: &[String],
    language: Language,
    collapsed_groups: &mut HashSet<String>,
    enabled: bool,
    action: &mut Option<BranchMenuAction>,
) {
    if node.children.is_empty() {
        if let Some(full_name) = &node.full_name
            && let Some(branch) = branches_by_name.get(full_name.as_str())
        {
            branch_row(
                ui,
                branch_current_for_display(branch, pending_checkout),
                false,
                full_name,
                &node.name,
                depth,
                branch
                    .upstream
                    .as_ref()
                    .map(|upstream| upstream.name.as_str()),
                upstream_sync_counts_for_branch(branch),
                remote_branch_names,
                remotes,
                language,
                enabled,
                action,
            );
        }
        return;
    }

    let collapsed = collapsed_groups.contains(&node.path);
    if remote_branch_group_row(ui, node, depth, collapsed).clicked() {
        if collapsed {
            collapsed_groups.remove(&node.path);
        } else {
            collapsed_groups.insert(node.path.clone());
        }
    }
    if !collapsed {
        if let Some(full_name) = &node.full_name
            && let Some(branch) = branches_by_name.get(full_name.as_str())
        {
            branch_row(
                ui,
                branch_current_for_display(branch, pending_checkout),
                false,
                full_name,
                &node.name,
                depth + 1,
                branch
                    .upstream
                    .as_ref()
                    .map(|upstream| upstream.name.as_str()),
                upstream_sync_counts_for_branch(branch),
                remote_branch_names,
                remotes,
                language,
                enabled,
                action,
            );
        }
        for child in node.children.values() {
            local_branch_tree_rows(
                ui,
                child,
                depth + 1,
                branches_by_name,
                pending_checkout,
                remote_branch_names,
                remotes,
                language,
                collapsed_groups,
                enabled,
                action,
            );
        }
    }
}

fn remote_branch_tree_rows(
    ui: &mut Ui,
    node: &BranchTreeNode,
    depth: usize,
    language: Language,
    collapsed_groups: &mut HashSet<String>,
    enabled: bool,
    action: &mut Option<BranchMenuAction>,
) {
    if node.children.is_empty() {
        if let Some(full_name) = &node.full_name {
            remote_branch_row(ui, full_name, &node.name, depth, language, enabled, action);
        }
        return;
    }

    let collapsed = collapsed_groups.contains(&node.path);
    if remote_branch_group_row(ui, node, depth, collapsed).clicked() {
        if collapsed {
            collapsed_groups.remove(&node.path);
        } else {
            collapsed_groups.insert(node.path.clone());
        }
    }
    if !collapsed {
        for child in node.children.values() {
            remote_branch_tree_rows(
                ui,
                child,
                depth + 1,
                language,
                collapsed_groups,
                enabled,
                action,
            );
        }
    }
}

fn remote_branch_group_row(
    ui: &mut Ui,
    node: &BranchTreeNode,
    depth: usize,
    collapsed: bool,
) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::hover());
    if row_rect_hovered(ui, rect) {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(2.0, 1.0)),
            CornerRadius::same(4),
            theme::accent_soft(),
        );
    }

    allocate_clipped_ui_at_rect(ui, rect, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(12.0 + depth as f32 * 14.0);
            let (arrow_rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 18.0), Sense::hover());
            draw_tree_arrow(ui, arrow_rect, !collapsed);
            ui.label(RichText::new(&node.name).strong().color(theme::text()));
        });
    });

    let response = full_row_click_response(ui, rect, ("remote_branch_group_row", &node.path));
    response
}

fn remote_branch_row(
    ui: &mut Ui,
    full_name: &str,
    display_name: &str,
    depth: usize,
    language: Language,
    enabled: bool,
    action: &mut Option<BranchMenuAction>,
) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::hover());
    if enabled && row_rect_hovered(ui, rect) {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(2.0, 1.0)),
            CornerRadius::same(4),
            theme::accent_soft(),
        );
    }

    allocate_clipped_ui_at_rect(ui, rect, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(34.0 + depth as f32 * 14.0);
            ui.label(RichText::new(display_name).color(theme::muted()));
        });
    });

    let response =
        full_row_click_response_enabled(ui, rect, ("remote_branch_row", full_name), enabled);
    if enabled && response.double_clicked() {
        *action = Some(BranchMenuAction::CheckoutRemote {
            remote_branch: full_name.to_owned(),
        });
    }

    response.context_menu(|ui| {
        ui.set_min_width(220.0);
        ui.label(RichText::new(full_name).color(theme::text()));
        ui.separator();
        if ui
            .add_enabled(
                enabled,
                egui::Button::new(i18n::t(language, "branch.checkout_remote")),
            )
            .clicked()
        {
            *action = Some(BranchMenuAction::CheckoutRemote {
                remote_branch: full_name.to_owned(),
            });
            ui.close_menu();
        }
        if ui
            .add_enabled(
                enabled,
                egui::Button::new(i18n::t(language, "branch.delete_remote")),
            )
            .clicked()
        {
            *action = Some(BranchMenuAction::DeleteRemote {
                remote_branch: full_name.to_owned(),
            });
            ui.close_menu();
        }
    });

    response
}

fn remote_empty_label(language: Language, snapshot: &RepositorySnapshot) -> &'static str {
    if snapshot.remotes.is_empty() {
        i18n::t(language, "remote.none")
    } else {
        i18n::t(language, "remote.no_branches")
    }
}

fn stash_row(
    ui: &mut Ui,
    stash: &StashEntry,
    language: Language,
    action: &mut Option<StashMenuAction>,
) -> egui::Response {
    let response = pointing_hand_cursor(
        ui.allocate_response(Vec2::new(ui.available_width(), 42.0), Sense::click()),
    );
    let rect = response.rect;
    if response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(2.0, 2.0)),
            CornerRadius::same(4),
            theme::accent_soft(),
        );
    }

    allocate_clipped_ui_at_rect(ui, rect, |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new(&stash.selector)
                        .monospace()
                        .color(theme::accent()),
                );
                ui.label(
                    RichText::new(&stash.relative_time)
                        .small()
                        .color(theme::muted()),
                );
            });
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(RichText::new(&stash.message).small().color(theme::text()));
            });
        });
    });

    response.context_menu(|ui| {
        ui.set_min_width(180.0);
        ui.label(
            RichText::new(&stash.selector)
                .monospace()
                .color(theme::text()),
        );
        ui.separator();
        if ui.button(i18n::t(language, "stash.apply")).clicked() {
            *action = Some(StashMenuAction::Apply {
                selector: stash.selector.clone(),
            });
            ui.close_menu();
        }
        if ui.button(i18n::t(language, "stash.pop")).clicked() {
            *action = Some(StashMenuAction::Pop {
                selector: stash.selector.clone(),
            });
            ui.close_menu();
        }
        if ui.button(i18n::t(language, "stash.drop")).clicked() {
            *action = Some(StashMenuAction::Drop {
                selector: stash.selector.clone(),
                message: stash.message.clone(),
            });
            ui.close_menu();
        }
    });

    response
}

fn tag_row(
    ui: &mut Ui,
    tag: &Tag,
    language: Language,
    action: &mut Option<TagMenuAction>,
) -> egui::Response {
    let response = pointing_hand_cursor(
        ui.allocate_response(Vec2::new(ui.available_width(), 38.0), Sense::click()),
    );
    let rect = response.rect;
    if response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(2.0, 2.0)),
            CornerRadius::same(4),
            theme::accent_soft(),
        );
    }

    allocate_clipped_ui_at_rect(ui, rect, |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(RichText::new(&tag.name).color(theme::accent()));
                ui.label(
                    RichText::new(&tag.target)
                        .monospace()
                        .small()
                        .color(theme::muted()),
                );
            });
            if !tag.subject.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(RichText::new(&tag.subject).small().color(theme::muted()));
                });
            }
        });
    });

    response.context_menu(|ui| {
        ui.set_min_width(180.0);
        ui.label(RichText::new(&tag.name).color(theme::text()));
        ui.separator();
        if ui.button(i18n::t(language, "tag.checkout")).clicked() {
            *action = Some(TagMenuAction::Checkout {
                name: tag.name.clone(),
            });
            ui.close_menu();
        }
        if ui.button(i18n::t(language, "tag.delete")).clicked() {
            *action = Some(TagMenuAction::Delete {
                name: tag.name.clone(),
            });
            ui.close_menu();
        }
    });

    response
}

enum WorktreeTreeRow<'a> {
    Directory {
        path: String,
        depth: usize,
    },
    File {
        file: &'a WorktreeFile,
        depth: usize,
    },
}

fn worktree_tree_rows<'a>(
    files: &'a [WorktreeFile],
    collapsed_dirs: &HashSet<String>,
) -> Vec<WorktreeTreeRow<'a>> {
    let mut dirs = BTreeSet::new();
    for file in files {
        let path = normalize_worktree_path(&file.display_path);
        let parts = path.split('/').collect::<Vec<_>>();
        let mut prefix = String::new();
        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            dirs.insert(prefix.clone());
        }
    }

    let mut rows = Vec::new();
    for dir in dirs {
        if worktree_parent_collapsed(&dir, collapsed_dirs) {
            continue;
        }
        rows.push(WorktreeTreeRow::Directory {
            depth: dir.matches('/').count(),
            path: dir,
        });
    }
    for file in files {
        let path = normalize_worktree_path(&file.display_path);
        if worktree_parent_collapsed(&path, collapsed_dirs) {
            continue;
        }
        rows.push(WorktreeTreeRow::File {
            depth: path.matches('/').count(),
            file,
        });
    }

    rows.sort_by(|left, right| {
        let left_key = match left {
            WorktreeTreeRow::Directory { path, .. } => (path.as_str(), 0),
            WorktreeTreeRow::File { file, .. } => (file.display_path.as_str(), 1),
        };
        let right_key = match right {
            WorktreeTreeRow::Directory { path, .. } => (path.as_str(), 0),
            WorktreeTreeRow::File { file, .. } => (file.display_path.as_str(), 1),
        };
        left_key.cmp(&right_key)
    });
    rows
}

fn worktree_parent_collapsed(path: &str, collapsed_dirs: &HashSet<String>) -> bool {
    let path = normalize_worktree_path(path);
    let parts = path.split('/').collect::<Vec<_>>();
    let mut prefix = String::new();
    for part in parts.iter().take(parts.len().saturating_sub(1)) {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(part);
        if collapsed_dirs.contains(&prefix) {
            return true;
        }
    }
    false
}

fn normalize_worktree_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn worktree_path_basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn select_worktree_row(
    ui: &Ui,
    file: &WorktreeFile,
    staged: bool,
    selected: &mut Option<WorktreeRowClick>,
) {
    let modifiers = ui.input(|input| WorktreeSelectionModifiers {
        ctrl: input.modifiers.ctrl,
        shift: input.modifiers.shift,
    });
    *selected = Some(WorktreeRowClick {
        file: SelectedWorktreeFile {
            path: file.path.clone(),
            display_path: file.display_path.clone(),
            staged,
            untracked: file.index_status == '?',
        },
        modifiers,
    });
}

fn worktree_directory_row(
    ui: &mut Ui,
    path: &str,
    depth: usize,
    collapsed: bool,
    language: Language,
    action: &mut Option<WorktreeMenuAction>,
) -> egui::Response {
    let response = pointing_hand_cursor(ui.allocate_response(
        Vec2::new(ui.available_width(), FILE_ROW_HEIGHT),
        Sense::click(),
    ));
    let rect = response.rect;
    if response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(2.0, 1.0)),
            CornerRadius::same(4),
            theme::accent_soft(),
        );
    }
    let indent = FILE_ROW_LEFT_INSET + depth as f32 * 16.0;
    draw_clipped_cell(
        ui,
        rect.left() + indent,
        rect.center().y,
        14.0,
        if collapsed { ">" } else { "v" },
        theme::muted(),
        true,
    );
    draw_clipped_cell(
        ui,
        rect.left() + indent + 20.0,
        rect.center().y,
        (rect.width() - indent - 26.0).max(20.0),
        worktree_path_basename(path),
        theme::text(),
        true,
    );

    response.context_menu(|ui| {
        ui.set_min_width(180.0);
        ui.label(RichText::new(path).monospace().color(theme::text()));
        ui.separator();
        if ui
            .button(i18n::t(language, "worktree.add_gitignore"))
            .clicked()
        {
            *action = Some(WorktreeMenuAction::AddToGitIgnore {
                pattern: format!("{}/", path.trim_end_matches('/')),
            });
            ui.close_menu();
        }
    });

    response
}

fn worktree_file_row(
    ui: &mut Ui,
    file: &WorktreeFile,
    staged: bool,
    selected: bool,
    language: Language,
    action: &mut Option<WorktreeMenuAction>,
    depth: usize,
    path_label: &str,
) -> egui::Response {
    let status = if file.is_conflicted() {
        "U".to_owned()
    } else if staged {
        file.index_status.to_string()
    } else if file.index_status == '?' {
        "A".to_owned()
    } else {
        file.worktree_status.to_string()
    };
    let response = pointing_hand_cursor(ui.allocate_response(
        Vec2::new(ui.available_width(), FILE_ROW_HEIGHT),
        Sense::click(),
    ));
    let rect = response.rect;
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(2.0, 1.0)),
            CornerRadius::same(4),
            if selected {
                theme::accent_deep()
            } else {
                theme::accent_soft()
            },
        );
    }

    draw_worktree_file_row_content(
        ui,
        rect,
        FILE_ROW_LEFT_INSET,
        &status,
        path_label,
        selected,
        depth,
    );

    response.context_menu(|ui| {
        ui.set_min_width(180.0);
        ui.label(
            RichText::new(&file.display_path)
                .monospace()
                .color(theme::text()),
        );
        ui.separator();
        if file.is_conflicted()
            && ui
                .button(i18n::t(language, "worktree.resolve_conflict"))
                .clicked()
        {
            *action = Some(WorktreeMenuAction::ResolveConflict {
                path: file.path.clone(),
            });
            ui.close_menu();
        }
        if staged {
            if ui
                .button(i18n::t(language, "worktree.unstage_file"))
                .clicked()
            {
                *action = Some(WorktreeMenuAction::Unstage {
                    path: file.path.clone(),
                });
                ui.close_menu();
            }
        } else {
            if ui
                .button(i18n::t(language, "worktree.stage_file"))
                .clicked()
            {
                *action = Some(WorktreeMenuAction::Stage {
                    path: file.path.clone(),
                });
                ui.close_menu();
            }
        }
        if ui.button(i18n::t(language, "worktree.discard")).clicked() {
            *action = Some(WorktreeMenuAction::Discard {
                path: file.path.clone(),
                untracked: file.index_status == '?',
            });
            ui.close_menu();
        }
        if ui
            .button(i18n::t(language, "worktree.add_gitignore"))
            .clicked()
        {
            *action = Some(WorktreeMenuAction::AddToGitIgnore {
                pattern: normalize_worktree_path(&file.display_path),
            });
            ui.close_menu();
        }
    });

    response
}

fn draw_worktree_file_row_content(
    ui: &mut Ui,
    rect: Rect,
    left_inset: f32,
    status: &str,
    path: &str,
    selected: bool,
    depth: usize,
) {
    let left_inset = left_inset + depth as f32 * 16.0;
    let icon_rect = Rect::from_min_size(
        Pos2::new(
            rect.left() + left_inset,
            rect.center().y - FILE_ROW_ICON_SLOT / 2.0,
        ),
        Vec2::splat(FILE_ROW_ICON_SLOT),
    );
    draw_file_status_icon(ui, icon_rect, status, selected);

    let text_rect = Rect::from_min_max(
        Pos2::new(icon_rect.right() + 6.0, rect.top()),
        Pos2::new(rect.right() - 6.0, rect.bottom()),
    );
    let text_color = if selected {
        Color32::WHITE
    } else {
        theme::text()
    };

    draw_clipped_cell(
        ui,
        text_rect.left(),
        rect.center().y,
        text_rect.width().max(20.0),
        path,
        text_color,
        true,
    );
}

fn commit_context_menu(
    ui: &mut Ui,
    commit: &Commit,
    language: Language,
) -> Option<CommitMenuAction> {
    let mut action = None;
    ui.set_min_width(220.0);
    ui.label(RichText::new(&commit.subject).strong().color(theme::text()));
    ui.label(
        RichText::new(format!("{}  {}", commit.short_hash, commit.author))
            .small()
            .color(theme::muted()),
    );
    ui.add_space(6.0);

    if ui.button(i18n::t(language, "menu.copy_hash")).clicked() {
        ui.ctx().copy_text(commit.hash.clone());
        ui.close_menu();
    }
    if ui
        .button(i18n::t(language, "menu.copy_short_hash"))
        .clicked()
    {
        ui.ctx().copy_text(commit.short_hash.clone());
        ui.close_menu();
    }

    ui.add_space(6.0);
    if ui
        .button(i18n::t(language, "menu.checkout_commit"))
        .clicked()
    {
        action = Some(CommitMenuAction::Checkout {
            hash: commit.hash.clone(),
            short_hash: commit.short_hash.clone(),
        });
        ui.close_menu();
    }
    if ui.button(i18n::t(language, "menu.create_branch")).clicked() {
        action = Some(CommitMenuAction::CreateBranch {
            hash: commit.hash.clone(),
            short_hash: commit.short_hash.clone(),
        });
        ui.close_menu();
    }
    if ui.button(i18n::t(language, "menu.create_tag")).clicked() {
        action = Some(CommitMenuAction::CreateTag {
            hash: commit.hash.clone(),
            short_hash: commit.short_hash.clone(),
        });
        ui.close_menu();
    }
    ui.add_space(6.0);
    if ui.button(i18n::t(language, "menu.cherry_pick")).clicked() {
        action = Some(CommitMenuAction::CherryPick {
            hash: commit.hash.clone(),
            short_hash: commit.short_hash.clone(),
        });
        ui.close_menu();
    }
    if ui.button(i18n::t(language, "menu.revert")).clicked() {
        action = Some(CommitMenuAction::Revert {
            hash: commit.hash.clone(),
            short_hash: commit.short_hash.clone(),
        });
        ui.close_menu();
    }
    if ui.button(i18n::t(language, "menu.reset")).clicked() {
        action = Some(CommitMenuAction::Reset {
            hash: commit.hash.clone(),
            short_hash: commit.short_hash.clone(),
        });
        ui.close_menu();
    }
    ui.add_space(6.0);
    ui.menu_button(i18n::t(language, "menu.compare"), |ui| {
        if ui
            .button(i18n::t(language, "menu.compare_worktree"))
            .clicked()
        {
            action = Some(CommitMenuAction::CompareWithWorktree {
                hash: commit.hash.clone(),
                short_hash: commit.short_hash.clone(),
            });
            ui.close_menu();
        }
        if ui.button(i18n::t(language, "menu.external_diff")).clicked() {
            action = Some(CommitMenuAction::ExternalDiff {
                hash: commit.hash.clone(),
                short_hash: commit.short_hash.clone(),
            });
            ui.close_menu();
        }
        if ui.button(i18n::t(language, "menu.open_remote")).clicked() {
            action = Some(CommitMenuAction::OpenRemote {
                hash: commit.hash.clone(),
            });
            ui.close_menu();
        }
    });
    action
}

fn detail_line(ui: &mut Ui, label: &str, value: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(label).small().color(theme::muted()));
    ui.add(
        egui::Label::new(RichText::new(value).monospace().color(theme::text()))
            .wrap()
            .selectable(true),
    );
}

fn file_change_row(ui: &mut Ui, status: &str, path: &str, selected: bool) -> egui::Response {
    let response = ui.allocate_response(
        Vec2::new(ui.available_width(), FILE_ROW_HEIGHT),
        Sense::click(),
    );
    let rect = response.rect;
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(2.0, 1.0)),
            CornerRadius::same(4),
            if selected {
                theme::accent_deep()
            } else {
                theme::accent_soft()
            },
        );
    }

    draw_file_row_content(ui, rect, 4.0, status, path, selected);
    response
}

fn draw_file_row_content(
    ui: &mut Ui,
    rect: Rect,
    left_inset: f32,
    status: &str,
    path: &str,
    selected: bool,
) {
    let icon_rect = Rect::from_min_size(
        Pos2::new(
            rect.left() + left_inset,
            rect.center().y - FILE_ROW_ICON_SLOT / 2.0,
        ),
        Vec2::splat(FILE_ROW_ICON_SLOT),
    );
    draw_file_status_icon(ui, icon_rect, status, selected);

    let text_rect = Rect::from_min_max(
        Pos2::new(icon_rect.right() + 6.0, rect.top()),
        Pos2::new(rect.right() - 6.0, rect.bottom()),
    );
    let text_clip = text_rect.intersect(ui.clip_rect());
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(text_rect), |ui| {
        ui.set_clip_rect(text_clip);
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.add_sized(
                [ui.available_width(), FILE_ROW_HEIGHT],
                egui::Label::new(RichText::new(path).monospace().color(if selected {
                    Color32::WHITE
                } else {
                    theme::text()
                }))
                .truncate(),
            );
        });
    });
}

fn draw_file_status_icon(ui: &mut Ui, rect: Rect, status: &str, on_dark: bool) {
    let kind = status.chars().next().unwrap_or('M');
    let color = file_status_color(kind, on_dark);
    let icon = file_status_icon(kind);
    draw_ui_icon(
        ui,
        Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
        icon,
        color,
    );
}

fn file_status_color(kind: char, on_dark: bool) -> Color32 {
    if on_dark {
        return Color32::WHITE;
    }
    match kind {
        'A' | '?' => Color32::from_rgb(42, 166, 109),
        'D' => Color32::from_rgb(220, 76, 70),
        'U' => Color32::from_rgb(232, 174, 55),
        'M' | 'R' => theme::info(),
        _ => theme::muted(),
    }
}

fn file_status_icon(kind: char) -> UiIcon {
    match kind {
        'A' | '?' => UiIcon::AddFile,
        'D' => UiIcon::DeleteFile,
        'R' => UiIcon::RenameFile,
        'U' => UiIcon::Warning,
        'M' => UiIcon::Edit,
        _ => UiIcon::File,
    }
}

fn render_unified_diff(
    ui: &mut Ui,
    text: &str,
    mode: DiffDisplayMode,
    language: Language,
    selected_rows: &mut Vec<DiffLineKey>,
) {
    let previous_item_spacing = ui.spacing().item_spacing;
    ui.spacing_mut().item_spacing.y = 0.0;

    for item in collect_unified_diff_items(text, mode) {
        match item {
            DiffRenderItem::FileHeader(text) => {
                ui.label(RichText::new(text).monospace().color(theme::info()));
            }
            DiffRenderItem::HunkHeader(text) => {
                diff_hunk_row(ui, &text);
            }
            DiffRenderItem::Omitted => diff_omitted_row(ui),
            DiffRenderItem::Line(line) => {
                diff_row(ui, &line, language, selected_rows);
            }
        }
    }

    ui.spacing_mut().item_spacing = previous_item_spacing;
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DiffRenderItem {
    FileHeader(String),
    HunkHeader(String),
    Omitted,
    Line(DiffLine),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DiffLine {
    left_no: String,
    right_no: String,
    body: String,
    kind: DiffKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DiffLineKey {
    left_no: String,
    right_no: String,
    body: String,
    kind: DiffKind,
}

impl DiffLine {
    fn key(&self) -> DiffLineKey {
        DiffLineKey {
            left_no: self.left_no.clone(),
            right_no: self.right_no.clone(),
            body: self.body.clone(),
            kind: self.kind,
        }
    }
}

fn collect_unified_diff_items(text: &str, mode: DiffDisplayMode) -> Vec<DiffRenderItem> {
    let mut items = Vec::new();
    let mut hunk_header = None;
    let mut hunk_lines = Vec::new();
    let mut old_line: Option<usize> = None;
    let mut new_line: Option<usize> = None;

    for line in text.lines().take(1_200) {
        if line.starts_with("diff --git")
            || line.starts_with("index ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
        {
            push_diff_hunk_items(&mut items, hunk_header.take(), &hunk_lines, mode);
            hunk_lines.clear();
            if line.starts_with("diff --git") {
                if mode == DiffDisplayMode::Full {
                    items.push(DiffRenderItem::FileHeader(clean_diff_header(line)));
                }
            }
            continue;
        }

        if line.starts_with("@@") {
            push_diff_hunk_items(&mut items, hunk_header.take(), &hunk_lines, mode);
            hunk_lines.clear();
            let (old_start, new_start) = parse_hunk_header(line);
            old_line = old_start;
            new_line = new_start;
            hunk_header = Some(line.to_owned());
            continue;
        }

        let kind = if line.starts_with('+') {
            DiffKind::Added
        } else if line.starts_with('-') {
            DiffKind::Removed
        } else {
            DiffKind::Context
        };

        let left_no = match kind {
            DiffKind::Added => String::new(),
            _ => old_line.map(|line| line.to_string()).unwrap_or_default(),
        };
        let right_no = match kind {
            DiffKind::Removed => String::new(),
            _ => new_line.map(|line| line.to_string()).unwrap_or_default(),
        };
        let body = line
            .strip_prefix('+')
            .or_else(|| line.strip_prefix('-'))
            .unwrap_or(line);

        hunk_lines.push(DiffLine {
            left_no,
            right_no,
            body: body.to_owned(),
            kind,
        });

        match kind {
            DiffKind::Added => new_line = new_line.map(|line| line + 1),
            DiffKind::Removed => old_line = old_line.map(|line| line + 1),
            DiffKind::Context => {
                old_line = old_line.map(|line| line + 1);
                new_line = new_line.map(|line| line + 1);
            }
        }
    }

    push_diff_hunk_items(&mut items, hunk_header.take(), &hunk_lines, mode);
    items
}

fn push_diff_hunk_items(
    items: &mut Vec<DiffRenderItem>,
    header: Option<String>,
    lines: &[DiffLine],
    mode: DiffDisplayMode,
) {
    if lines.is_empty() {
        return;
    }

    match mode {
        DiffDisplayMode::Full => {
            if let Some(header) = header {
                items.push(DiffRenderItem::HunkHeader(header));
            }
            items.extend(lines.iter().cloned().map(DiffRenderItem::Line));
        }
        DiffDisplayMode::Blocks => {
            let ranges = condensed_diff_ranges(lines);
            for (range_index, (start, end)) in ranges.iter().copied().enumerate() {
                if range_index > 0 || start > 0 {
                    items.push(DiffRenderItem::Omitted);
                }
                items.extend(lines[start..=end].iter().cloned().map(DiffRenderItem::Line));
            }
            if ranges
                .last()
                .is_some_and(|(_, end)| *end < lines.len().saturating_sub(1))
            {
                items.push(DiffRenderItem::Omitted);
            }
        }
    }
}

fn condensed_diff_ranges(lines: &[DiffLine]) -> Vec<(usize, usize)> {
    const CONTEXT: usize = 3;
    let mut ranges: Vec<(usize, usize)> = Vec::new();

    for index in lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (line.kind != DiffKind::Context).then_some(index))
    {
        let start = index.saturating_sub(CONTEXT);
        let end = (index + CONTEXT).min(lines.len().saturating_sub(1));
        if let Some((_, last_end)) = ranges.last_mut() {
            if start <= *last_end + 1 {
                *last_end = (*last_end).max(end);
                continue;
            }
        }
        ranges.push((start, end));
    }

    ranges
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffKind {
    Added,
    Removed,
    Context,
}

fn diff_row(
    ui: &mut Ui,
    line: &DiffLine,
    language: Language,
    selected_rows: &mut Vec<DiffLineKey>,
) {
    let key = line.key();
    let width = ui.available_width().max(560.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::click());

    if response.clicked_by(egui::PointerButton::Primary) {
        let ctrl = ui.input(|input| input.modifiers.ctrl);
        if ctrl {
            if let Some(index) = selected_rows.iter().position(|selected| selected == &key) {
                selected_rows.remove(index);
            } else {
                selected_rows.push(key.clone());
            }
        } else {
            selected_rows.clear();
            selected_rows.push(key.clone());
        }
    }
    if response.clicked_by(egui::PointerButton::Secondary)
        && !selected_rows.iter().any(|selected| selected == &key)
    {
        selected_rows.clear();
        selected_rows.push(key.clone());
    }

    let selected = selected_rows.iter().any(|selected| selected == &key);
    let fill = if selected {
        Color32::from_rgb(82, 168, 236)
    } else {
        match line.kind {
            DiffKind::Added => Color32::from_rgb(214, 250, 221),
            DiffKind::Removed => Color32::from_rgb(255, 226, 226),
            DiffKind::Context => Color32::TRANSPARENT,
        }
    };
    let text_color = if selected {
        Color32::WHITE
    } else {
        match line.kind {
            DiffKind::Added => Color32::from_rgb(16, 92, 42),
            DiffKind::Removed => Color32::from_rgb(142, 37, 37),
            DiffKind::Context => Color32::from_rgb(71, 82, 96),
        }
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
    }

    response.context_menu(|ui| {
        if ui.button(i18n::t(language, "menu.copy")).clicked() {
            ui.ctx().copy_text(selected_diff_rows_text(selected_rows));
            ui.close_menu();
        }
    });

    let gutter_bg = Rect::from_min_max(
        rect.left_top(),
        Pos2::new(rect.left() + 96.0, rect.bottom()),
    );
    ui.painter().rect_filled(
        gutter_bg,
        CornerRadius::ZERO,
        if selected {
            Color32::from_rgb(74, 151, 214)
        } else {
            Color32::from_rgb(248, 250, 252)
        },
    );
    ui.painter().line_segment(
        [
            Pos2::new(gutter_bg.right(), rect.top()),
            Pos2::new(gutter_bg.right(), rect.bottom()),
        ],
        Stroke::new(
            1.0,
            if selected {
                Color32::from_rgb(67, 137, 195)
            } else {
                Color32::from_rgb(224, 229, 236)
            },
        ),
    );
    let gutter = if selected {
        Color32::from_rgb(235, 247, 255)
    } else {
        Color32::from_rgb(124, 135, 148)
    };
    ui.painter().text(
        Pos2::new(rect.left() + 36.0, rect.center().y),
        Align2::RIGHT_CENTER,
        &line.left_no,
        FontId::monospace(12.0),
        gutter,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 74.0, rect.center().y),
        Align2::RIGHT_CENTER,
        &line.right_no,
        FontId::monospace(12.0),
        gutter,
    );
    let sign = match line.kind {
        DiffKind::Added => "+",
        DiffKind::Removed => "-",
        DiffKind::Context => " ",
    };
    ui.painter().text(
        Pos2::new(rect.left() + 104.0, rect.center().y),
        Align2::LEFT_CENTER,
        sign,
        FontId::monospace(12.0),
        text_color,
    );
    let text_rect = Rect::from_min_max(
        Pos2::new(rect.left() + 122.0, rect.top()),
        Pos2::new(rect.right() - 6.0, rect.bottom()),
    );
    if !selected {
        draw_diff_indent_guides(ui, text_rect, &line.body);
    }
    ui.painter().with_clip_rect(text_rect).text(
        text_rect.left_center(),
        Align2::LEFT_CENTER,
        &line.body,
        FontId::monospace(12.0),
        text_color,
    );
}

fn selected_diff_rows_text(selected_rows: &[DiffLineKey]) -> String {
    selected_rows
        .iter()
        .map(|line| line.body.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn draw_diff_indent_guides(ui: &mut Ui, text_rect: Rect, body: &str) {
    let char_width = ui.fonts(|fonts| {
        fonts
            .layout_no_wrap(
                " ".to_owned(),
                FontId::monospace(12.0),
                Color32::TRANSPARENT,
            )
            .rect
            .width()
            .max(1.0)
    });
    let dot_color = Color32::from_rgb(190, 198, 210);
    let mut column = 0usize;
    for ch in body.chars() {
        match ch {
            ' ' => {
                let x = text_rect.left() + column as f32 * char_width + char_width * 0.5;
                if x < text_rect.right() {
                    ui.painter()
                        .circle_filled(Pos2::new(x, text_rect.center().y), 1.0, dot_color);
                }
                column += 1;
            }
            '\t' => {
                let next_tab = ((column / 4) + 1) * 4;
                while column < next_tab {
                    let x = text_rect.left() + column as f32 * char_width + char_width * 0.5;
                    if x < text_rect.right() {
                        ui.painter().circle_filled(
                            Pos2::new(x, text_rect.center().y),
                            1.0,
                            dot_color,
                        );
                    }
                    column += 1;
                }
            }
            _ => break,
        }
    }
}

fn diff_hunk_row(ui: &mut Ui, text: &str) {
    let width = ui.available_width().max(560.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 20.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, Color32::from_rgb(242, 247, 255));
    ui.painter().text(
        Pos2::new(rect.left() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        FontId::monospace(12.0),
        Color32::from_rgb(54, 103, 178),
    );
}

fn diff_omitted_row(ui: &mut Ui) {
    let width = ui.available_width().max(560.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, Color32::from_rgb(250, 251, 253));
    ui.painter().line_segment(
        [
            Pos2::new(rect.left(), rect.center().y),
            Pos2::new(rect.right(), rect.center().y),
        ],
        Stroke::new(1.0, Color32::from_rgb(225, 230, 238)),
    );
    ui.painter().text(
        Pos2::new(rect.left() + 48.0, rect.center().y),
        Align2::CENTER_CENTER,
        "...",
        FontId::monospace(12.0),
        Color32::from_rgb(128, 139, 153),
    );
}

fn clean_diff_header(line: &str) -> String {
    let raw = line.strip_prefix("diff --git ").unwrap_or(line);
    let mut parts = raw.split_whitespace();
    let left = parts.next().unwrap_or(raw).trim_start_matches("a/");
    let right = parts.next().unwrap_or(left).trim_start_matches("b/");
    if left == right {
        left.to_owned()
    } else {
        format!("{left}  ->  {right}")
    }
}

fn parse_hunk_header(line: &str) -> (Option<usize>, Option<usize>) {
    let mut parts = line.split_whitespace();
    let _ = parts.next();
    let old = parts
        .next()
        .and_then(|part| part.trim_start_matches('-').split(',').next())
        .and_then(|value| value.parse::<usize>().ok());
    let new = parts
        .next()
        .and_then(|part| part.trim_start_matches('+').split(',').next())
        .and_then(|value| value.parse::<usize>().ok());
    (old, new)
}

fn view_uses_side_details(_view: MainView) -> bool {
    false
}

fn master_detail_split_heights(available_y: f32) -> (f32, f32) {
    let details_height =
        if available_y >= HISTORY_DETAILS_MIN_HEIGHT + HISTORY_LIST_MIN_HEIGHT + 8.0 {
            (available_y * 0.42).clamp(
                HISTORY_DETAILS_MIN_HEIGHT,
                available_y - HISTORY_LIST_MIN_HEIGHT - 8.0,
            )
        } else {
            (available_y * 0.40).max(140.0)
        };
    let list_height = (available_y - details_height - 8.0).max(HISTORY_LIST_MIN_HEIGHT);
    (list_height, details_height)
}

fn commit_message_editor_height(available_body_height: f32) -> f32 {
    (available_body_height - COMMIT_BUTTON_ROW_HEIGHT - COMMIT_MESSAGE_BOTTOM_GAP)
        .max(COMMIT_MESSAGE_EDITOR_MIN_HEIGHT)
}

fn shortcut_pressed(ctx: &egui::Context, key: egui::Key, shift: bool) -> bool {
    ctx.input(|input| {
        input.modifiers.ctrl
            && input.modifiers.shift == shift
            && !input.modifiers.alt
            && input.key_pressed(key)
    })
}

fn stage_toggle_shortcut_pressed(ctx: &egui::Context) -> bool {
    ctx.input(|input| {
        let stage_toggle_modifiers = input.modifiers.shift
            && (input.modifiers.ctrl || input.modifiers.command)
            && !input.modifiers.alt;
        let c_key_pressed = input.key_pressed(egui::Key::C);
        let copy_event = input
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::Copy));

        stage_toggle_modifiers && (c_key_pressed || copy_event)
    })
}

fn shortcut_stage_toggle_action(snapshot: &RepositorySnapshot) -> Option<WorktreeMenuAction> {
    if !snapshot.unstaged.is_empty() {
        Some(WorktreeMenuAction::StageAll)
    } else if !snapshot.staged.is_empty() {
        Some(WorktreeMenuAction::UnstageAll)
    } else {
        None
    }
}

fn upstream_sync_counts(snapshot: Option<&RepositorySnapshot>) -> UpstreamSyncCounts {
    snapshot
        .and_then(|snapshot| snapshot.upstream.as_ref())
        .map(|upstream| UpstreamSyncCounts {
            ahead: upstream.ahead,
            behind: upstream.behind,
        })
        .unwrap_or_default()
}

fn branch_current_for_display(branch: &git::Branch, pending_checkout: Option<&str>) -> bool {
    pending_checkout.map_or(branch.current, |pending| {
        !branch.remote && branch.name == pending
    })
}

fn upstream_sync_counts_for_branch(branch: &git::Branch) -> Option<UpstreamSyncCounts> {
    branch.upstream.as_ref().map(|upstream| UpstreamSyncCounts {
        ahead: upstream.ahead,
        behind: upstream.behind,
    })
}

fn upstream_pull_badge(counts: Option<UpstreamSyncCounts>) -> Option<String> {
    let behind = counts?.behind;
    (behind > 0).then(|| format!("\u{2193}{behind}"))
}

fn upstream_push_badge(counts: Option<UpstreamSyncCounts>) -> Option<String> {
    let ahead = counts?.ahead;
    (ahead > 0).then(|| format!("\u{2191}{ahead}"))
}

fn toolbar_sync_label(label: &str, badge: Option<String>) -> String {
    match badge {
        Some(badge) => format!("{label}  {badge}"),
        None => label.to_owned(),
    }
}

fn empty_state(ui: &mut Ui, loading: bool, language: Language) {
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            if loading {
                ui.spinner();
                ui.add_space(8.0);
                ui.label(
                    RichText::new(i18n::t(language, "status.loading_repo"))
                        .heading()
                        .color(theme::text()),
                );
                return;
            }
            ui.label(
                RichText::new(i18n::t(language, "repo.none"))
                    .heading()
                    .color(theme::text()),
            );
            ui.label(
                RichText::new("Git Agent will render the commit graph with virtualized rows.")
                    .color(theme::muted()),
            );
        });
    });
}

fn no_commits_state(ui: &mut Ui, language: Language) {
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(i18n::t(language, "commit.no_commits"))
                    .heading()
                    .color(theme::text()),
            );
            ui.label(
                RichText::new(i18n::t(language, "commit.no_commits_hint")).color(theme::muted()),
            );
        });
    });
}

fn paths_equal(left: &std::path::Path, right: &std::path::Path) -> bool {
    let left = left
        .canonicalize()
        .unwrap_or_else(|_| left.to_path_buf())
        .to_string_lossy()
        .to_lowercase();
    let right = right
        .canonicalize()
        .unwrap_or_else(|_| right.to_path_buf())
        .to_string_lossy()
        .to_lowercase();
    left == right
}

fn open_command_prompt(root: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new(git_bash_executable())
            .arg(format!("--cd={}", root.display()))
            .spawn()
            .map(|_| ())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("sh")
            .arg("-c")
            .arg("x-terminal-emulator || open -a Terminal . || gnome-terminal .")
            .current_dir(root)
            .spawn()
            .map(|_| ())
    }
}

#[cfg(target_os = "windows")]
fn git_bash_executable() -> PathBuf {
    for path in [
        "C:/Program Files/Git/git-bash.exe",
        "C:/Program Files (x86)/Git/git-bash.exe",
    ] {
        let path = PathBuf::from(path);
        if path.exists() {
            return path;
        }
    }
    PathBuf::from("git-bash.exe")
}

fn open_file_manager(root: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(file_manager_target_arg(root))
            .spawn()
            .map(|_| ())
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(root).spawn().map(|_| ())
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(root).spawn().map(|_| ())
    }
}

fn open_path(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", &path.display().to_string()])
            .spawn()
            .map(|_| ())
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn().map(|_| ())
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(path).spawn().map(|_| ())
    }
}

#[cfg(target_os = "windows")]
fn file_manager_target_arg(root: &Path) -> String {
    root.display().to_string().replace('/', "\\")
}

fn open_url(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
            .map(|_| ())
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn().map(|_| ())
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(url).spawn().map(|_| ())
    }
}

fn remote_web_url(remote_url: &str) -> Option<String> {
    let remote_url = remote_url.trim();
    if remote_url.is_empty() {
        return None;
    }

    if remote_url.starts_with("http://") || remote_url.starts_with("https://") {
        return Some(strip_git_suffix(remote_url).to_owned());
    }

    if let Some(rest) = remote_url.strip_prefix("ssh://") {
        let rest = rest.split_once('@').map(|(_, value)| value).unwrap_or(rest);
        let (host, path) = rest.split_once('/')?;
        return Some(format!(
            "https://{}/{}",
            host,
            strip_git_suffix(path.trim_start_matches('/'))
        ));
    }

    if let Some((_, rest)) = remote_url.split_once('@') {
        if let Some((host, path)) = rest.split_once(':') {
            return Some(format!(
                "https://{}/{}",
                host,
                strip_git_suffix(path.trim_start_matches('/'))
            ));
        }
    }

    Some(strip_git_suffix(remote_url).to_owned())
}

fn branch_compare_url(base_url: &str, current_branch: &str, target_branch: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    let current_branch = url_path_component(current_branch);
    let target_branch = url_path_component(target_branch);
    if base_url.contains("gitlab.") || base_url.contains("gitlab.com") {
        format!("{base_url}/-/compare/{current_branch}...{target_branch}")
    } else {
        format!("{base_url}/compare/{current_branch}...{target_branch}")
    }
}

fn branch_pull_request_url(base_url: &str, branch: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    if base_url.contains("gitlab.") || base_url.contains("gitlab.com") {
        return format!(
            "{base_url}/-/merge_requests/new?merge_request%5Bsource_branch%5D={}",
            url_query_component(branch)
        );
    }
    format!("{base_url}/compare/{}?expand=1", url_path_component(branch))
}

fn split_remote_branch_name(name: &str) -> Option<(String, String)> {
    let (remote, branch) = name.split_once('/')?;
    if remote.trim().is_empty() || branch.trim().is_empty() {
        return None;
    }
    Some((remote.to_owned(), branch.to_owned()))
}

fn push_remote_branch_default(
    branch: &git::Branch,
    remote: &str,
    remote_branches: &[String],
) -> String {
    if let Some(upstream_branch) = branch
        .upstream
        .as_ref()
        .and_then(|upstream| split_remote_branch_name(&upstream.name))
        .and_then(|(upstream_remote, upstream_branch)| {
            (upstream_remote == remote).then_some(upstream_branch)
        })
    {
        return upstream_branch;
    }
    if remote_branches
        .iter()
        .any(|candidate| candidate == &branch.name)
    {
        return branch.name.clone();
    }
    String::new()
}

fn push_remote_branch_default_for_row(
    row: &PushBranchRow,
    remote: &str,
    remote_branches: &[String],
) -> String {
    if let Some(upstream_branch) = row
        .upstream
        .as_deref()
        .and_then(split_remote_branch_name)
        .and_then(|(upstream_remote, upstream_branch)| {
            (upstream_remote == remote).then_some(upstream_branch)
        })
    {
        return upstream_branch;
    }
    if remote_branches
        .iter()
        .any(|candidate| candidate == &row.local_branch)
    {
        return row.local_branch.clone();
    }
    String::new()
}

fn push_remote_branch_choices(
    local_branch: &str,
    selected_branch: &str,
    remote_branches: &[String],
) -> Vec<String> {
    let mut choices = Vec::new();
    for branch in remote_branches {
        if !choices.iter().any(|candidate| candidate == branch) {
            choices.push(branch.clone());
        }
    }
    for branch in [selected_branch, local_branch] {
        if !branch.trim().is_empty() && !choices.iter().any(|candidate| candidate == branch) {
            choices.push(branch.to_owned());
        }
    }
    choices
}

fn push_remote_branch_column_width(table_width: f32) -> f32 {
    (table_width
        - PUSH_SELECT_COLUMN_WIDTH
        - PUSH_LOCAL_BRANCH_COLUMN_WIDTH
        - PUSH_TRACK_COLUMN_WIDTH
        - PUSH_TABLE_COLUMN_GAP * 3.0)
        .max(180.0)
}

fn push_remote_form_row(
    ui: &mut Ui,
    label: &str,
    remote_url_display: &mut String,
    add_remote_selector: impl FnOnce(&mut Ui),
) {
    let width = ui.available_width();
    let (row_rect, _) = ui.allocate_exact_size(
        Vec2::new(width, PUSH_REMOTE_FORM_ROW_HEIGHT),
        Sense::hover(),
    );
    let label_rect = Rect::from_min_size(
        row_rect.min,
        Vec2::new(PUSH_REMOTE_FORM_LABEL_WIDTH, PUSH_REMOTE_FORM_ROW_HEIGHT),
    );
    paint_push_form_label(ui, label_rect, label);

    let selector_rect = Rect::from_center_size(
        Pos2::new(
            label_rect.right() + PUSH_TABLE_COLUMN_GAP + PUSH_REMOTE_FORM_SELECTOR_WIDTH / 2.0,
            row_rect.center().y,
        ),
        Vec2::new(
            PUSH_REMOTE_FORM_SELECTOR_WIDTH,
            PUSH_REMOTE_FORM_CONTROL_HEIGHT,
        ),
    );
    ui.allocate_new_ui(
        egui::UiBuilder::new().max_rect(selector_rect),
        |selector_ui| {
            selector_ui.set_width(PUSH_REMOTE_FORM_SELECTOR_WIDTH);
            selector_ui.spacing_mut().interact_size.y = PUSH_REMOTE_FORM_CONTROL_HEIGHT;
            add_remote_selector(selector_ui);
        },
    );

    let url_left = selector_rect.right() + PUSH_TABLE_COLUMN_GAP;
    let url_width = (row_rect.right() - url_left).max(120.0);
    let url_rect = Rect::from_center_size(
        Pos2::new(url_left + url_width / 2.0, row_rect.center().y),
        Vec2::new(url_width, PUSH_REMOTE_FORM_CONTROL_HEIGHT),
    );
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(url_rect), |url_ui| {
        themed_text_edit_selection(url_ui);
        url_ui.add_enabled_ui(false, |url_ui| {
            url_ui.add_sized(
                [url_width, PUSH_REMOTE_FORM_CONTROL_HEIGHT],
                themed_singleline_text_edit(remote_url_display, "").desired_width(url_width),
            );
        });
    });
}

fn paint_push_form_label(ui: &mut Ui, rect: Rect, label: &str) {
    ui.painter().with_clip_rect(rect).text(
        Pos2::new(rect.left(), rect.center().y + 1.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        theme::muted(),
    );
}

fn push_branch_table_header(
    ui: &mut Ui,
    table_width: f32,
    select_label: &str,
    local_branch_label: &str,
    remote_branch_label: &str,
    track_label: &str,
) {
    let remote_width = push_remote_branch_column_width(table_width);
    let (row_rect, _) = ui.allocate_exact_size(Vec2::new(table_width, 24.0), Sense::hover());
    let (select_rect, local_rect, remote_rect, track_rect) =
        push_branch_cell_rects(row_rect, remote_width);
    paint_push_branch_text_cell(ui, select_rect, select_label, theme::muted(), 12.0);
    paint_push_branch_text_cell(ui, local_rect, local_branch_label, theme::muted(), 12.0);
    paint_push_branch_text_cell(ui, remote_rect, remote_branch_label, theme::muted(), 12.0);
    paint_push_branch_text_cell(ui, track_rect, track_label, theme::muted(), 12.0);
}

fn push_branch_cell_rects(row_rect: Rect, remote_width: f32) -> (Rect, Rect, Rect, Rect) {
    let select_rect = Rect::from_min_size(
        row_rect.min,
        Vec2::new(PUSH_SELECT_COLUMN_WIDTH, row_rect.height()),
    );
    let local_rect = Rect::from_min_size(
        Pos2::new(select_rect.right() + PUSH_TABLE_COLUMN_GAP, row_rect.top()),
        Vec2::new(PUSH_LOCAL_BRANCH_COLUMN_WIDTH, row_rect.height()),
    );
    let remote_rect = Rect::from_min_size(
        Pos2::new(local_rect.right() + PUSH_TABLE_COLUMN_GAP, row_rect.top()),
        Vec2::new(remote_width, row_rect.height()),
    );
    let track_rect = Rect::from_min_size(
        Pos2::new(remote_rect.right() + PUSH_TABLE_COLUMN_GAP, row_rect.top()),
        Vec2::new(PUSH_TRACK_COLUMN_WIDTH, row_rect.height()),
    );
    (select_rect, local_rect, remote_rect, track_rect)
}

fn paint_push_branch_text_cell(ui: &mut Ui, rect: Rect, text: &str, color: Color32, size: f32) {
    ui.painter().with_clip_rect(rect).text(
        Pos2::new(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(size),
        color,
    );
}

fn paint_push_branch_body_text_cell(
    ui: &mut Ui,
    rect: Rect,
    text: &str,
    color: Color32,
    size: f32,
) {
    ui.painter().with_clip_rect(rect).text(
        Pos2::new(rect.left(), rect.center().y + PUSH_TABLE_BODY_TEXT_Y_OFFSET),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(size),
        color,
    );
}

fn push_branch_table_row(
    ui: &mut Ui,
    table_width: f32,
    row: &mut PushBranchRow,
    remote_branches: &[String],
) {
    let remote_width = push_remote_branch_column_width(table_width);
    let (row_rect, _) = ui.allocate_exact_size(
        Vec2::new(table_width, PUSH_TABLE_ROW_HEIGHT),
        Sense::hover(),
    );
    let (select_rect, local_rect, remote_rect, track_rect) =
        push_branch_cell_rects(row_rect, remote_width);

    let was_selected = row.selected;
    ui.put(
        Rect::from_center_size(select_rect.center(), Vec2::splat(20.0)),
        egui::Checkbox::new(&mut row.selected, ""),
    );
    if row.selected && !was_selected && row.remote_branch.trim().is_empty() {
        row.remote_branch = row.local_branch.clone();
    }

    paint_push_branch_body_text_cell(
        ui,
        local_rect,
        row.local_branch.as_str(),
        theme::text(),
        14.0,
    );

    let choices =
        push_remote_branch_choices(&row.local_branch, &row.remote_branch, remote_branches);
    let combo_rect = Rect::from_center_size(
        remote_rect.center(),
        Vec2::new(remote_rect.width(), PUSH_REMOTE_FORM_CONTROL_HEIGHT),
    );
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(combo_rect), |ui| {
        ui.set_width(remote_rect.width());
        egui::ComboBox::from_id_salt(("push_remote_branch_selector", row.local_branch.as_str()))
            .width(remote_rect.width())
            .selected_text(row.remote_branch.as_str())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut row.remote_branch, String::new(), "");
                for branch in choices {
                    ui.selectable_value(&mut row.remote_branch, branch.clone(), branch);
                }
            });
    });

    ui.put(
        Rect::from_center_size(track_rect.center(), Vec2::splat(20.0)),
        egui::Checkbox::new(&mut row.track, ""),
    );
}

fn commit_remote_url(base_url: &str, hash: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    let hash = url_path_component(hash);
    if base_url.contains("gitlab.") || base_url.contains("gitlab.com") {
        format!("{base_url}/-/commit/{hash}")
    } else {
        format!("{base_url}/commit/{hash}")
    }
}

fn url_path_component(value: &str) -> String {
    url_component(value, true)
}

fn url_query_component(value: &str) -> String {
    url_component(value, false)
}

fn url_component(value: &str, keep_slash: bool) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        let safe = byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'.' | b'_' | b'~')
            || (keep_slash && byte == b'/');
        if safe {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn strip_git_suffix(value: &str) -> &str {
    value.strip_suffix(".git").unwrap_or(value)
}

#[cfg(test)]
mod ui_tests {
    use super::*;

    #[test]
    fn top_bar_has_two_fixed_rows() {
        assert_eq!(TOP_BAR_TAB_TOOL_JOIN_OVERLAP, 6.0);
        assert_eq!(TOP_BAR_PANEL_X_INSET, LAYOUT_GAP as f32);
        assert_eq!(
            TOP_BAR_HEIGHT,
            TITLE_BAR_HEIGHT + TOP_BAR_ROW_HEIGHT * 2.0 - TOP_BAR_TAB_TOOL_JOIN_OVERLAP
        );
        let source = include_str!("app.rs");
        let top_bar_start = source.find("fn top_bar_panel(").unwrap();
        let top_bar_end = source[top_bar_start..].find("fn main_layout(").unwrap();
        let top_bar_source = &source[top_bar_start..top_bar_start + top_bar_end];
        assert!(source.contains("exact_height(self.top_bar_height())"));
        assert!(source.contains("if !self.repository_source_active()"));
        assert_eq!(central_panel_margin(false).top, LAYOUT_GAP);
        assert_eq!(central_panel_margin(true).top, 0);
        assert_eq!(central_panel_margin(true).left, LAYOUT_GAP);
        assert_eq!(repository_source_panel_y_margin(), 0);
        assert!(top_bar_source.contains("let top_island_rect = top_island_rect("));
        assert!(top_bar_source.contains("let tab_strip_row = repo_tab_strip_rect("));
        assert!(top_bar_source.contains(".rect_filled(full, CornerRadius::ZERO, theme::bg())"));
        assert!(top_bar_source.contains(".fill(theme::panel_soft())"));
        assert!(top_bar_source.contains(".shadow(panel_shadow())"));
        assert!(top_bar_source.contains(".paint(top_island_rect)"));
        assert!(
            top_bar_source.contains("paint_layout_debug_rect(ui, top_island_rect, \"top.island\"")
        );
        assert!(top_bar_source.contains("paint_layout_debug_rect(ui, tab_strip_row, \"top.tabs\""));
        assert!(!top_bar_source.contains("let menu_row = Rect::from_min_max("));
        assert!(!top_bar_source.contains("max_rect(menu_row)"));
        let title_bar_start = source.find("fn custom_title_bar(").unwrap();
        let title_bar_end = source[title_bar_start..]
            .find("fn desktop_menu_bar(")
            .unwrap();
        let title_bar_source = &source[title_bar_start..title_bar_start + title_bar_end];
        assert!(title_bar_source.contains("app_title_logo(ui);"));
        assert!(title_bar_source.contains("self.desktop_menu_bar(ui, has_repo, has_remote);"));
        assert!(title_bar_source.contains("custom_title_drag_rect(rect, controls_width)"));
        assert!(source.contains("TITLE_MENU_RESERVED_WIDTH"));
        assert!(!title_bar_source.contains("RichText::new(\"Git Agent\")"));
        assert!(top_bar_source.contains("let tool_row_panel_rect = (!source_active).then(|| {"));
        assert!(top_bar_source.contains("Rect::from_min_max("));
        assert!(top_bar_source.contains("TOP_BAR_TAB_TOOL_JOIN_OVERLAP"));
        assert!(top_bar_source.contains("TOP_BAR_PANEL_X_INSET"));
        assert!(top_bar_source.contains("tab_row.bottom() - TOP_BAR_TAB_TOOL_JOIN_OVERLAP"));
        assert!(top_bar_source.contains("tool_row_panel_rect"));
        assert!(top_bar_source.contains("tool_row_corners()"));
        assert!(top_bar_source.contains("theme::panel()"));
        assert!(top_bar_source.contains(".paint(tool_row_panel_rect)"));
        assert!(
            top_bar_source
                .contains("paint_layout_debug_rect(ui, tool_row_panel_rect, \"top.toolbar\"")
        );
        assert_eq!(TOP_BAR_GLOBAL_ACTION_Y_OFFSET, -1.0);
        assert!(
            top_bar_source
                .contains("let global_action_row = tab_right.translate(Vec2::new(0.0, TOP_BAR_GLOBAL_ACTION_Y_OFFSET));")
        );
        assert!(top_bar_source.contains("top_bar_drag_region(ctx, ui, tab_right"));
        assert!(top_bar_source.contains("let tool_content_row = tool_row_panel_rect"));
        assert!(top_bar_source.contains(".map(|rect| Rect::from_min_max(rect.left_top()"));
        assert!(top_bar_source.contains("if let Some(tool_content_row) = tool_content_row"));
        assert!(top_bar_source.contains("max_rect(tool_content_row)"));
        assert_eq!(REPO_TAB_STRIP_LEFT_PADDING, TOP_BAR_PANEL_X_INSET);
        assert!(!top_bar_source.contains(".rect_filled(full, CornerRadius::ZERO, theme::panel())"));
        assert!(source.contains("fn toolbar_button_normal_fill("));
        assert!(source.contains("fn toolbar_button_hover_fill("));
        assert!(!top_bar_source.contains("MENU_BAR_HEIGHT"));
        let menu_button_start = source.find("fn menu_button(").unwrap();
        let menu_button_end = source[menu_button_start..].find("fn menu_label(").unwrap();
        let menu_button_source = &source[menu_button_start..menu_button_start + menu_button_end];
        assert!(
            menu_button_source.contains("ui.spacing_mut().button_padding = Vec2::new(6.0, 2.0)")
        );
        assert!(menu_button_source.contains("widgets.inactive.bg_fill = Color32::TRANSPARENT"));
        assert!(
            menu_button_source.contains("widgets.inactive.weak_bg_fill = Color32::TRANSPARENT")
        );
        let tab_right_start = source
            .find("ui.allocate_new_ui(egui::UiBuilder::new().max_rect(global_action_row)")
            .unwrap();
        let tab_right_end = source[tab_right_start..]
            .find("if !self.repository_source_active()")
            .unwrap();
        let tab_right_source = &source[tab_right_start..tab_right_start + tab_right_end];
        assert!(tab_right_source.contains("toolbar_button(ui, \"settings\""));
        assert!(tab_right_source.contains("toolbar_button(ui, \"git-flow\""));
        assert!(tab_right_source.contains("toolbar_button(ui, \"remote\""));
        assert!(tab_right_source.contains("toolbar_button(ui, \"terminal\""));
        assert!(tab_right_source.contains(
            "toolbar_button(\n                    ui,\n                    \"resource-manager\""
        ));
        assert!(!tab_right_source.contains("toolbar_button(ui, \"open\""));
        assert!(
            !tab_right_source.contains(
                "toolbar_button(\n                    ui,\n                    \"refresh\""
            )
        );
        assert_eq!(
            menu_label(Language::Chinese, "tools"),
            "\u{5de5}\u{5177}(T)"
        );
        assert_eq!(
            menu_label(Language::Chinese, "options"),
            "\u{9009}\u{9879}(O)"
        );
    }

    #[test]
    fn layout_uses_custom_titlebar_and_no_side_panel_splitters() {
        let source = include_str!("app.rs");
        assert!(source.contains("ViewportCommand::StartDrag"));
        assert!(source.contains("ViewportCommand::Minimized"));
        assert!(source.contains("ViewportCommand::Maximized"));
        assert!(source.contains("ViewportCommand::Close"));
        assert!(!source.contains(concat!("SidePanel", "::left")));
        assert!(!source.contains(concat!("SidePanel", "::right")));
    }

    #[test]
    fn layout_uses_gap_shadow_and_no_content_logo() {
        assert!(LAYOUT_GAP >= 8);
        let shadow = panel_shadow();
        assert!(shadow.blur <= 10);
        assert_eq!(shadow.spread, 0);
        assert!(shadow.offset[0] > 0);
        assert!(shadow.offset[1] > shadow.offset[0]);
        assert!(shadow.color.a() > 0);
        let green = theme::palette_for(theme::ThemeMode::Light, theme::ThemeAccent::Green);
        let blue = theme::palette_for(theme::ThemeMode::Light, theme::ThemeAccent::Blue);
        assert_ne!(green.accent, blue.accent);
        assert_ne!(green.accent_soft, blue.accent_soft);
        assert_ne!(green.accent_shadow, blue.accent_shadow);
        assert_ne!(green.panel, blue.panel);
        assert_ne!(green.panel_soft, blue.panel_soft);
        assert_ne!(green.scroll_track, Color32::BLACK);
        let source = include_str!("app.rs");
        let theme_source = include_str!("theme.rs");
        assert!(source.contains("theme::accent_deep()"));
        assert!(source.contains("theme::accent_soft()"));
        assert!(source.contains("settings_accent_button("));
        assert!(source.contains("let clip_pad = gap;"));
        assert!(theme_source.contains("style.interaction.tooltip_delay = 0.12"));
        assert_ne!(
            theme::palette(theme::ThemeMode::Dark).bg,
            theme::palette(theme::ThemeMode::Light).bg
        );
    }

    #[test]
    fn resizable_layout_uses_percentages_and_inner_frame_sizes() {
        let prefs = LayoutPrefs::parse(
            "sidebar_pct=0.20\ndetails_pct=0.31\nworkspace_list_pct=0.70\nworkspace_staged_pct=0.60\nworkspace_diff_pct=0.40\n",
        )
        .unwrap();
        assert!((prefs.sidebar_pct - 0.20).abs() < f32::EPSILON);
        assert!((prefs.details_pct - 0.31).abs() < f32::EPSILON);
        assert!((prefs.workspace_list_pct - 0.70).abs() < f32::EPSILON);
        assert!((prefs.workspace_staged_pct - 0.60).abs() < f32::EPSILON);
        assert!((prefs.workspace_diff_pct - 0.40).abs() < f32::EPSILON);
        assert_eq!(prefs.history_top_pct, 0.0);

        let inner = frame_inner_size(260.0, 300.0, LAYOUT_GAP, LAYOUT_GAP);
        assert!(inner.x < 260.0);
        assert!(inner.y < 300.0);
        assert_eq!(safe_ui_length(-1.0), 0.0);
        assert_eq!(safe_ui_length(f32::NAN), 0.0);
        assert_eq!(safe_ui_size(Vec2::new(-4.0, f32::NAN)), Vec2::ZERO);

        let gap = Rect::from_min_max(Pos2::new(260.0, 0.0), Pos2::new(268.0, 600.0));
        let handle = resize_handle_rect(gap, true);
        assert_eq!(handle.width(), RESIZE_HANDLE_THICKNESS);
        assert_eq!(handle.height(), gap.height());
    }

    #[test]
    fn main_layout_outer_panels_use_exact_shared_rect_heights() {
        let full = Rect::from_min_size(Pos2::new(0.3, 11.7), Vec2::new(1365.6, 701.4));
        let with_details = main_layout_rects(full, 0.21, 0.32, true);
        assert_eq!(with_details.sidebar.top(), with_details.content.top());
        assert_eq!(with_details.center.top(), with_details.content.top());
        assert_eq!(with_details.details.top(), with_details.content.top());
        assert_eq!(with_details.sidebar.bottom(), with_details.content.bottom());
        assert_eq!(with_details.center.bottom(), with_details.content.bottom());
        assert_eq!(with_details.details.bottom(), with_details.content.bottom());
        assert_eq!(
            with_details.sidebar_center_gap.height(),
            with_details.content.height()
        );
        assert_eq!(
            with_details.center_details_gap.height(),
            with_details.content.height()
        );
        assert_eq!(with_details.content.bottom(), full.bottom().round());

        let without_details = main_layout_rects(full, 0.21, 0.32, false);
        assert_eq!(
            without_details.sidebar.bottom(),
            without_details.center.bottom()
        );
        assert_eq!(
            without_details.center.right(),
            without_details.content.right()
        );
        assert_eq!(without_details.details.width(), 0.0);
    }

    #[test]
    fn main_layout_outer_panels_use_exact_painted_rects_not_frame_sizing() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let layout_start = implementation_source.find("fn main_layout(").unwrap();
        let layout_end = implementation_source[layout_start..]
            .find("fn top_bar(")
            .unwrap();
        let layout_source = &implementation_source[layout_start..layout_start + layout_end];

        assert!(implementation_source.contains("fn exact_panel_at_rect("));
        assert!(
            layout_source
                .contains("exact_panel_at_rect(\n            ui,\n            sidebar_rect")
        );
        assert!(
            layout_source
                .contains("exact_panel_at_rect(\n            ui,\n            center_rect")
        );
        assert!(
            layout_source.contains(
                "exact_panel_at_rect(\n                ui,\n                details_rect"
            )
        );
        assert!(!layout_source.contains("content_panel_frame(theme::panel()).show"));
        assert!(
            !layout_source
                .contains("soft_panel_frame(theme::panel(), LAYOUT_GAP, LAYOUT_GAP).show")
        );
    }

    #[test]
    fn layout_debug_overlay_can_trace_panel_and_workspace_rects() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        assert!(implementation_source.contains("GIT_AGENT_LAYOUT_DEBUG"));
        assert!(implementation_source.contains("fn layout_debug_enabled("));
        assert!(implementation_source.contains("fn paint_layout_debug_rect("));
        assert!(implementation_source.contains("fn log_layout_debug_once("));
        assert!(implementation_source.contains("paint_layout_debug_rect(ui, layout.sidebar"));
        assert!(implementation_source.contains("paint_layout_debug_rect(ui, layout.center"));
        assert!(
            implementation_source.contains("paint_layout_debug_rect(ui, layout.staged_rect")
                || implementation_source.contains(
                    "paint_layout_debug_rect(\n            ui,\n            layout.staged_rect"
                )
        );
        assert!(implementation_source.contains("paint_layout_debug_rect(ui, layout.commit_rect"));
    }

    #[test]
    fn worktree_file_rows_show_full_paths_and_gitignore_menu() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let start = implementation_source.find("fn worktree_file_row(").unwrap();
        let end = implementation_source[start..]
            .find("fn draw_file_row_content(")
            .unwrap();
        let row_source = &implementation_source[start..start + end];

        assert!(row_source.contains("draw_worktree_file_row_content("));
        assert!(implementation_source.contains("fn draw_worktree_file_row_content("));
        assert!(!row_source.contains("split_file_display_path(path)"));
        assert!(!row_source.contains("history_file_column_widths("));
        assert!(implementation_source.contains("draw_clipped_cell("));
        assert!(implementation_source.contains("WorktreeDisplayMode::Tree"));
        assert!(implementation_source.contains("WorktreeMenuAction::AddToGitIgnore"));
        assert!(implementation_source.contains("worktree.add_gitignore"));
        assert!(implementation_source.contains("WorktreeRowClick"));
        assert!(implementation_source.contains("WorktreeSelectionState"));
        assert!(implementation_source.contains("input.modifiers.ctrl"));
        assert!(implementation_source.contains("input.modifiers.shift"));
    }

    #[test]
    fn worktree_tree_rows_include_dirs_and_hide_collapsed_children() {
        let files = ["src/app.rs", "src/git.rs", "README.md"]
            .into_iter()
            .map(|path| WorktreeFile {
                path: path.to_owned(),
                display_path: path.to_owned(),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let rows = worktree_tree_rows(&files, &HashSet::new());

        assert!(rows.iter().any(|row| matches!(
            row,
            WorktreeTreeRow::Directory { path, depth } if path == "src" && *depth == 0
        )));
        assert!(rows.iter().any(|row| matches!(
            row,
            WorktreeTreeRow::File { file, depth } if file.display_path == "src/app.rs" && *depth == 1
        )));

        let mut collapsed = HashSet::new();
        collapsed.insert("src".to_owned());
        let collapsed_rows = worktree_tree_rows(&files, &collapsed);
        assert!(collapsed_rows.iter().any(|row| matches!(
            row,
            WorktreeTreeRow::Directory { path, .. } if path == "src"
        )));
        assert!(!collapsed_rows.iter().any(|row| matches!(
            row,
            WorktreeTreeRow::File { file, .. } if file.display_path == "src/app.rs"
        )));
    }

    #[test]
    fn worktree_selection_uses_windows_ctrl_and_shift_ranges() {
        let files = ["a.txt", "b.txt", "c.txt", "d.txt"]
            .into_iter()
            .map(|path| WorktreeFile {
                path: path.to_owned(),
                display_path: path.to_owned(),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let mut selection = WorktreeSelectionState::default();

        selection.apply(
            &files,
            "b.txt",
            false,
            WorktreeSelectionModifiers::default(),
        );
        assert_worktree_selection(&selection, false, &["b.txt"]);

        selection.apply(
            &files,
            "d.txt",
            false,
            WorktreeSelectionModifiers {
                ctrl: false,
                shift: true,
            },
        );
        assert_worktree_selection(&selection, false, &["b.txt", "c.txt", "d.txt"]);

        selection.apply(
            &files,
            "c.txt",
            false,
            WorktreeSelectionModifiers {
                ctrl: true,
                shift: false,
            },
        );
        assert_worktree_selection(&selection, false, &["b.txt", "d.txt"]);

        selection.apply(
            &files,
            "a.txt",
            false,
            WorktreeSelectionModifiers {
                ctrl: true,
                shift: true,
            },
        );
        assert_worktree_selection(&selection, false, &["a.txt", "b.txt", "c.txt", "d.txt"]);

        selection.apply(&files, "a.txt", true, WorktreeSelectionModifiers::default());
        assert_worktree_selection(&selection, true, &["a.txt"]);
        assert_worktree_selection(&selection, false, &[]);
    }

    fn assert_worktree_selection(
        selection: &WorktreeSelectionState,
        staged: bool,
        expected: &[&str],
    ) {
        let actual = selection
            .paths(staged)
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let expected = expected.iter().copied().collect::<HashSet<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn app_settings_are_json_and_dialog_window_has_no_outer_frame() {
        let settings = AppSettings {
            theme: SettingsThemeMode::Light,
            theme_accent: SettingsThemeAccent::Purple,
            language: SettingsLanguage::English,
            remote_accounts: vec![RemoteAccountSettings::default()],
        };
        let raw = serde_json::to_string(&settings).unwrap();
        assert!(raw.contains("\"theme\":\"Light\""));
        assert!(raw.contains("\"theme_accent\":\"Purple\""));
        assert!(raw.contains("\"language\":\"English\""));
        let source = include_str!("app.rs");
        let app_data_start = source.find("fn app_data_dir()").unwrap();
        let app_data_end = source[app_data_start..]
            .find("fn app_settings_path()")
            .unwrap();
        let app_data_source = &source[app_data_start..app_data_start + app_data_end];
        assert!(app_data_source.contains("env::current_exe()"));
        assert!(app_data_source.contains("base.join(\"data\")"));
        assert!(!app_data_source.contains("LOCALAPPDATA"));

        let frame = dialog_window_frame();
        assert_eq!(frame.fill, Color32::TRANSPARENT);
        assert_eq!(frame.stroke, Stroke::NONE);
        assert_eq!(frame.inner_margin.left, 0);
    }

    #[test]
    fn sidebar_cards_fit_three_items_without_overlap() {
        let available = 212.0;
        let width = sidebar_nav_card_width(available, 3);
        assert!(width <= 70.0);
        assert!(width * 3.0 + 12.0 <= available);
    }

    #[test]
    fn sidebar_tabs_and_tree_headers_avoid_past_regressions() {
        let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(12.0, 18.0));
        let open_points = tree_arrow_points(true, rect);
        let closed_points = tree_arrow_points(false, rect);
        assert!(open_points[2].y > open_points[0].y);
        assert!(closed_points[2].x > closed_points[0].x);

        let source = include_str!("app.rs");
        assert!(!source.contains("small_button(self.tr(\"branch.create\")"));
        assert!(!source.contains("small_button(self.tr(\"tag.create\")"));

        let start = source.find("fn sidebar_nav_card(").unwrap();
        let end = source.find("fn repo_tab_with_close(").unwrap();
        let sidebar_nav_card_source = &source[start..end];
        assert!(sidebar_nav_card_source.contains("Sense::click()"));
        assert!(!sidebar_nav_card_source.contains("allocate_new_ui"));
        assert!(!sidebar_nav_card_source.contains("ui.add("));
        assert!(!sidebar_nav_card_source.contains("ui.add_sized("));
    }

    #[test]
    fn sidebar_scroll_children_preserve_parent_clip_rect() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        assert!(implementation_source.contains("fn allocate_clipped_ui_at_rect("));
        assert!(implementation_source.contains("rect.intersect(ui.clip_rect())"));

        for (start_marker, end_marker) in [
            ("fn tree_header_inner(", "fn tree_empty("),
            ("fn remote_branch_group_row(", "fn remote_branch_row("),
            ("fn remote_branch_row(", "fn remote_empty_label("),
            ("fn stash_row(", "fn tag_row("),
            ("fn tag_row(", "enum WorktreeTreeRow"),
        ] {
            let start = implementation_source
                .find(start_marker)
                .unwrap_or_else(|| panic!("{start_marker}"));
            let end = implementation_source[start..]
                .find(end_marker)
                .unwrap_or_else(|| panic!("{end_marker}"));
            let block = &implementation_source[start..start + end];
            assert!(
                block.contains("allocate_clipped_ui_at_rect(ui, rect"),
                "{start_marker} must keep row rendering clipped to the parent ScrollArea"
            );
            assert!(!block.contains("ui.set_clip_rect(rect)"));
            assert!(!block.contains("egui::UiBuilder::new().max_rect(rect)"));
        }

        let panel_start = implementation_source
            .find("fn exact_panel_at_rect(")
            .unwrap();
        let panel_end = implementation_source[panel_start..]
            .find("fn panel_shadow(")
            .unwrap();
        let panel_source = &implementation_source[panel_start..panel_start + panel_end];
        assert!(panel_source.contains("allocate_clipped_ui_at_rect(ui, inner_rect"));
    }

    #[test]
    fn sidebar_branch_tree_owns_current_branch_state() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        let branch_tree_start = implementation_source
            .find("let branches_open_before = self.branches_open")
            .unwrap();
        let branch_tree_end = implementation_source[branch_tree_start..]
            .find("let tag_create_label")
            .unwrap();
        let branch_tree_source =
            &implementation_source[branch_tree_start..branch_tree_start + branch_tree_end];
        assert!(!branch_tree_source.contains("RichText::new(&snapshot.branch)"));

        let branch_row_start = implementation_source.find("fn branch_row(").unwrap();
        let branch_row_end = implementation_source[branch_row_start..]
            .find("fn remote_branch_row(")
            .unwrap();
        let branch_row_source =
            &implementation_source[branch_row_start..branch_row_start + branch_row_end];
        assert!(implementation_source.contains("const BRANCH_CURRENT_BADGE_RIGHT_GAP: f32 = 4.0;"));
        assert!(implementation_source.contains("const BRANCH_CURRENT_BADGE_Y_OFFSET: f32 = 0.0;"));
        assert!(branch_row_source.contains("paint_branch_row_badges("));
        assert!(!branch_row_source.contains("Layout::right_to_left(Align::Center)"));
        assert!(branch_row_source.contains("branch_current_indicator_rect("));
        assert!(branch_row_source.contains("branch.current_badge"));
        assert!(branch_row_source.contains("theme::accent_deep()"));
        assert!(branch_row_source.contains("name_text = name_text.strong()"));
        assert!(branch_row_source.contains("paint_branch_row_badges("));
        assert!(branch_row_source.contains("paint_branch_text_badge_at("));
        assert!(!branch_row_source.contains("RichText::new(if current { \"*\" } else { \" \" })"));
        assert!(!branch_row_source.contains("i18n::t(language, \"common.local\")"));
    }

    #[test]
    fn branch_sync_badges_are_painted_inside_row_right_edge() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let branch_row_start = implementation_source.find("fn branch_row(").unwrap();
        let branch_row_end = implementation_source[branch_row_start..]
            .find("fn remote_branch_row(")
            .unwrap();
        let branch_row_source =
            &implementation_source[branch_row_start..branch_row_start + branch_row_end];
        let Some(badge_start) = implementation_source.find("fn paint_branch_row_badges(") else {
            panic!("branch badges must be painted from the row rect right edge");
        };
        let badge_end = implementation_source[badge_start..]
            .find("#[derive(Clone, Debug, Default)]")
            .unwrap();
        let badge_source = &implementation_source[badge_start..badge_start + badge_end];
        let compact_branch_row_source = branch_row_source
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        assert!(compact_branch_row_source.contains("let badge_left = paint_branch_row_badges("));
        assert!(branch_row_source.contains("let name_right = (badge_left - 6.0).max(name_left);"));
        assert!(branch_row_source.contains("paint_branch_name(ui, name_rect, name_text);"));
        assert!(!branch_row_source.contains("egui::Label::new(name_text).truncate()"));
        assert!(!branch_row_source.contains("branch_sync_badges(ui, sync_counts)"));
        assert!(
            badge_source
                .contains("let mut right = row_rect.right() - BRANCH_CURRENT_BADGE_RIGHT_GAP;")
        );
        assert!(badge_source.contains("paint_branch_badge_at("));
        assert!(badge_source.contains("Rect::from_center_size("));
        assert!(badge_source.contains("right - width / 2.0"));
        assert!(badge_source.contains("row_rect.center().y + BRANCH_CURRENT_BADGE_Y_OFFSET"));
        assert!(!badge_source.contains("BRANCH_CURRENT_BADGE_Y_OFFSET: f32 = 2.5"));
    }

    #[test]
    fn branch_names_stay_left_aligned_when_badges_are_right_aligned() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let branch_row_start = implementation_source.find("fn branch_row(").unwrap();
        let branch_row_end = implementation_source[branch_row_start..]
            .find("fn remote_branch_row(")
            .unwrap();
        let branch_row_source =
            &implementation_source[branch_row_start..branch_row_start + branch_row_end];

        assert!(branch_row_source.contains("paint_branch_name(ui, name_rect, name_text);"));
        assert!(!branch_row_source.contains("egui::Label::new(name_text).truncate()"));
        assert!(!branch_row_source.contains("ui.add_sized("));
        let paint_name_start = implementation_source.find("fn paint_branch_name(").unwrap();
        let paint_name_end = implementation_source[paint_name_start..]
            .find("fn paint_branch_row_badges(")
            .unwrap();
        let paint_name_source =
            &implementation_source[paint_name_start..paint_name_start + paint_name_end];
        let compact_paint_name_source = paint_name_source
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(paint_name_source.contains("layout_job.wrap.max_width = name_rect.width();"));
        assert!(paint_name_source.contains("layout_job.wrap.max_rows = 1;"));
        assert!(paint_name_source.contains("layout_job.wrap.break_anywhere = true;"));
        assert!(paint_name_source.contains("Align::Min"));
        assert!(!paint_name_source.contains("Align::Center"));
        assert!(paint_name_source.contains("Align2::LEFT_CENTER"));
        assert!(paint_name_source.contains("anchor_size(name_rect.left_center(), galley.size())"));
        assert!(compact_paint_name_source.contains(".min;"));
        assert!(paint_name_source.contains("with_clip_rect(name_rect)"));
    }

    #[test]
    fn local_branch_rows_double_click_checkout_non_current_branch() {
        let source = include_str!("app.rs");
        let branch_row_start = source.find("fn branch_row(").unwrap();
        let branch_row_end = source[branch_row_start..]
            .find("#[derive(Clone, Debug, Default)]")
            .unwrap();
        let branch_row_source = &source[branch_row_start..branch_row_start + branch_row_end];

        assert!(branch_row_source.contains("response.double_clicked()"));
        assert!(branch_row_source.contains("if remote {"));
        assert!(branch_row_source.contains("} else if !current {"));
        assert!(branch_row_source.contains("BranchMenuAction::Checkout {"));
        assert!(branch_row_source.contains("name: name.to_owned()"));
    }

    #[test]
    fn branch_sidebar_rows_use_full_row_overlay_interaction() {
        let source = include_str!("app.rs");
        assert!(source.contains("fn full_row_click_response("));

        for (start_marker, end_marker) in [
            ("fn branch_row(", "#[derive(Clone, Debug, Default)]"),
            ("fn remote_branch_group_row(", "fn remote_branch_row("),
            ("fn remote_branch_row(", "fn remote_empty_label("),
        ] {
            let start = source.find(start_marker).unwrap();
            let end = source[start..].find(end_marker).unwrap();
            let block = &source[start..start + end];
            assert!(
                block.contains("full_row_click_response(ui, rect")
                    || block.contains("full_row_click_response_enabled(ui, rect")
            );
        }
    }

    #[test]
    fn remote_sidebar_empty_state_distinguishes_missing_remote_from_empty_refs() {
        let without_remote = RepositorySnapshot {
            root: PathBuf::from("D:/repo"),
            ..Default::default()
        };
        let with_remote = RepositorySnapshot {
            root: PathBuf::from("D:/repo"),
            remotes: vec![git::Remote {
                name: "origin".to_owned(),
                fetch_url: "https://example/repo.git".to_owned(),
                push_url: "https://example/repo.git".to_owned(),
            }],
            ..Default::default()
        };

        assert_eq!(
            remote_empty_label(Language::English, &without_remote),
            "No remote repositories"
        );
        assert_eq!(
            remote_empty_label(Language::English, &with_remote),
            "No fetched remote branches"
        );
        assert_eq!(
            i18n::t(Language::Chinese, "remote.no_branches"),
            "\u{672a}\u{83b7}\u{53d6}\u{5230}\u{8fdc}\u{7aef}\u{5206}\u{652f}"
        );
    }

    #[test]
    fn remote_branch_tree_groups_slash_separated_paths() {
        let branches = [
            git::Branch {
                name: "origin/feature/login".to_owned(),
                remote: true,
                current: false,
                upstream: None,
            },
            git::Branch {
                name: "origin/main".to_owned(),
                remote: true,
                current: false,
                upstream: None,
            },
            git::Branch {
                name: "upstream/release/1.0".to_owned(),
                remote: true,
                current: false,
                upstream: None,
            },
        ];
        let branch_refs = branches.iter().collect::<Vec<_>>();

        let tree = remote_branch_tree(&branch_refs);

        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].name, "origin");
        assert!(tree[0].children.contains_key("feature"));
        assert_eq!(
            tree[0].children["feature"].children["login"]
                .full_name
                .as_deref(),
            Some("origin/feature/login")
        );
        assert_eq!(
            tree[0].children["main"].full_name.as_deref(),
            Some("origin/main")
        );
        assert_eq!(tree[1].name, "upstream");

        let source = include_str!("app.rs");
        assert!(source.contains("remote_branch_collapsed_groups: HashSet<String>"));
        assert!(source.contains("remote_branch_tree_rows("));
    }

    #[test]
    fn local_branch_tree_groups_slash_separated_paths() {
        let branches = [
            git::Branch {
                name: "f/UI-2.1".to_owned(),
                remote: false,
                current: true,
                upstream: None,
            },
            git::Branch {
                name: "f/amp-chat".to_owned(),
                remote: false,
                current: false,
                upstream: None,
            },
            git::Branch {
                name: "master".to_owned(),
                remote: false,
                current: false,
                upstream: None,
            },
        ];
        let branch_refs = branches.iter().collect::<Vec<_>>();

        let tree = local_branch_tree(&branch_refs);

        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].name, "f");
        assert!(tree[0].children.contains_key("UI-2.1"));
        assert_eq!(
            tree[0].children["UI-2.1"].full_name.as_deref(),
            Some("f/UI-2.1")
        );
        assert_eq!(
            tree[0].children["amp-chat"].full_name.as_deref(),
            Some("f/amp-chat")
        );
        assert_eq!(tree[1].full_name.as_deref(), Some("master"));

        let source = include_str!("app.rs");
        assert!(source.contains("local_branch_collapsed_groups: HashSet<String>"));
        assert!(source.contains("local_branch_tree_rows("));
        assert!(source.contains("branch_row("));
        assert!(source.contains("display_name"));
    }

    #[test]
    fn workspace_does_not_keep_side_details_panel() {
        assert!(!view_uses_side_details(MainView::Workspace));
        assert!(!view_uses_side_details(MainView::History));
        assert!(!view_uses_side_details(MainView::Search));
        assert!(!view_uses_side_details(MainView::Branches));
        assert!(!view_uses_side_details(MainView::Tags));
        assert!(!view_uses_side_details(MainView::Stashes));
    }

    #[test]
    fn workspace_file_selection_opens_resizable_diff_panel() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let workspace_start = implementation_source.find("fn workspace_view(").unwrap();
        let workspace_end = implementation_source[workspace_start..]
            .find("fn search_view(")
            .unwrap();
        let workspace_source =
            &implementation_source[workspace_start..workspace_start + workspace_end];
        assert!(implementation_source.contains("workspace_diff_pct"));
        assert!(workspace_source.contains("self.selected_worktree_file.is_some()"));
        assert!(workspace_source.contains("workspace_diff_resize"));
        assert!(workspace_source.contains("vertical_resize_delta"));
        assert!(workspace_source.contains("let workspace_content_rect = ui.allocate_exact_size"));
        assert!(workspace_source.contains("egui::UiBuilder::new().max_rect(left_rect)"));
        assert!(workspace_source.contains("egui::UiBuilder::new().max_rect(right_rect)"));
        assert!(!workspace_source.contains("ui.horizontal_top(|ui|"));
        assert!(!workspace_source.contains("source_tree_panel_frame().show(ui, |ui|"));
        assert!(!workspace_source.contains("frame_inner_size("));
        assert!(workspace_source.contains("safe_set_min_size(ui, right_rect.size());"));
        assert!(workspace_source.contains("self.workspace_main_panel("));
        assert!(workspace_source.contains("self.worktree_diff_viewer(ui);"));
        assert!(implementation_source.contains("request_selected_worktree_diff()"));
        assert!(implementation_source.contains("worktree_diff_scroll"));
        let worktree_diff_start = implementation_source
            .find("fn worktree_diff_viewer(")
            .unwrap();
        let worktree_diff_end = implementation_source[worktree_diff_start..]
            .find("fn commit_action_modal(")
            .unwrap();
        let worktree_diff_source =
            &implementation_source[worktree_diff_start..worktree_diff_start + worktree_diff_end];
        assert!(!worktree_diff_source.contains("panel_heading_inline"));
        assert!(!worktree_diff_source.contains("RichText::new(&selected.display_path)"));
        assert!(worktree_diff_source.contains("worktree_diff_panel_frame().show(ui, |ui|"));
        assert!(worktree_diff_source.contains("let available_diff_size = safe_ui_size"));
        assert!(worktree_diff_source.contains("let inner_size = safe_ui_size"));
        assert!(worktree_diff_source.contains("safe_set_min_size(ui, inner_size)"));
        assert!(worktree_diff_source.contains("let diff_rect = diff_response.response.rect"));
        assert!(worktree_diff_source.contains("paint_workspace_card_inset_shadow(ui, diff_rect)"));

        let worktree_frame_start = implementation_source
            .find("fn worktree_diff_panel_frame(")
            .unwrap();
        let worktree_frame_end = implementation_source[worktree_frame_start..]
            .find("fn diff_display_mode_salt(")
            .unwrap();
        let worktree_frame_source =
            &implementation_source[worktree_frame_start..worktree_frame_start + worktree_frame_end];
        assert!(worktree_frame_source.contains(".fill(theme::panel())"));
        assert!(!worktree_frame_source.contains("theme::panel_recessed()"));
    }

    #[test]
    fn workspace_header_uses_compact_spacing_and_smaller_title() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let workspace_start = implementation_source.find("fn workspace_view(").unwrap();
        let workspace_end = implementation_source[workspace_start..]
            .find("if status_count == 0")
            .unwrap();
        let workspace_source =
            &implementation_source[workspace_start..workspace_start + workspace_end];

        assert!(implementation_source.contains("const WORKSPACE_HEADER_TOP_GAP: f32 = 4.0;"));
        assert!(implementation_source.contains("const WORKSPACE_HEADER_BOTTOM_GAP: f32 = 6.0;"));
        assert!(implementation_source.contains("const WORKSPACE_HEADER_TITLE_SIZE: f32 = 20.0;"));
        assert!(workspace_source.contains("ui.add_space(WORKSPACE_HEADER_TOP_GAP);"));
        assert!(workspace_source.contains("ui.add_space(WORKSPACE_HEADER_BOTTOM_GAP);"));
        assert!(workspace_source.contains(".size(WORKSPACE_HEADER_TITLE_SIZE)"));
        assert!(!workspace_source.contains(".heading()"));
    }

    #[test]
    fn master_detail_split_preserves_usable_list_and_details() {
        let (list, details) = master_detail_split_heights(900.0);
        assert!(list >= HISTORY_LIST_MIN_HEIGHT);
        assert!(details >= HISTORY_DETAILS_MIN_HEIGHT);
        assert!(list + details <= 900.0);

        let (small_list, small_details) = master_detail_split_heights(420.0);
        assert!(small_list >= HISTORY_LIST_MIN_HEIGHT);
        assert!(small_details >= 140.0);
    }

    #[test]
    fn history_layout_uses_resizable_top_region_and_shadow_splitter() {
        let source = include_str!("app.rs");
        let start = source.find("fn history_view(").unwrap();
        let end = source[start..].find("fn history_commit_table(").unwrap();
        let history_view_source = &source[start..start + end];
        assert!(history_view_source.contains("let min_top_height = history_top_min_height()"));
        assert!(
            history_view_source
                .contains("let top_height = if self.layout_prefs.history_top_pct > 0.0")
        );
        assert!(!history_view_source.contains("history_top_content_height"));
        assert!(!history_view_source.contains("max_top_height.min(content_top_height)"));
        assert!(!history_view_source.contains(".clamp(150.0"));
        assert!(history_view_source.contains("history_table_splitter(ui, available.x)"));
        assert!(history_view_source.contains("self.layout_prefs.history_top_pct"));
        assert!(!history_view_source.contains("draw_bottom_edge_shadow"));

        let table_start = source.find("fn history_commit_table(").unwrap();
        let table_end = source[table_start..]
            .find("fn history_bottom_pane(")
            .unwrap();
        let table_source = &source[table_start..table_start + table_end];
        assert!(table_source.contains("let body_height = ui.available_height().max(0.0)"));
        assert!(
            table_source
                .contains("ui.allocate_exact_size(Vec2::new(ui.available_width(), body_height)")
        );
        assert!(table_source.contains("egui::UiBuilder::new().max_rect(body_rect)"));
        assert!(table_source.contains("safe_set_min_size(ui, body_rect.size())"));
        assert!(table_source.contains("ui.set_max_height(body_rect.height())"));
        assert!(table_source.contains(".auto_shrink([false, false])"));
        assert!(table_source.contains("self.refresh_history_rows_cache()"));
        assert!(table_source.contains(".show_viewport(ui, |ui, viewport|"));
        assert!(table_source.contains("history_virtual_row_range("));
        assert!(
            table_source
                .contains("ui.set_min_height(visible_row_count as f32 * HISTORY_TABLE_ROW_HEIGHT)")
        );
        assert!(table_source.contains("self.history_rows_cache.index_at(row_index)"));
        assert!(!table_source.contains(".show_rows("));

        let splitter_start = source.find("fn history_table_splitter(").unwrap();
        let splitter_end = source[splitter_start..]
            .find("fn soft_panel_frame(")
            .unwrap();
        let splitter_source = &source[splitter_start..splitter_start + splitter_end];
        assert!(splitter_source.contains("Sense::click_and_drag()"));
        assert!(splitter_source.contains("CursorIcon::ResizeVertical"));
        assert!(splitter_source.contains("theme::accent_shadow()"));
        assert!(splitter_source.contains("line_segment"));
        assert!(!splitter_source.contains("rect_filled"));
    }

    #[test]
    fn history_scroll_areas_have_distinct_ids() {
        let source = include_str!("app.rs");
        assert!(source.contains("history_commit_graph_scroll"));
        assert!(source.contains("self.history_branch_scope"));
        assert!(source.contains("self.history_sort_order"));
        assert!(source.contains("self.search.as_str()"));
        assert!(source.contains("history_changed_files_scroll"));
        assert!(source.contains("search_results_scroll"));
        let search_start = source.find("fn search_view(").unwrap();
        let search_end = source[search_start..].find("fn history_view(").unwrap();
        let search_source = &source[search_start..search_start + search_end];
        assert!(!search_source.contains("search_details_scroll"));
    }

    #[test]
    fn search_view_uses_table_layout_and_history_bottom_pane() {
        let source = include_str!("app.rs");
        let start = source.find("fn search_view(").unwrap();
        let end = source[start..].find("fn history_view(").unwrap();
        let search_source = &source[start..start + end];
        assert!(search_source.contains("filter_rect"));
        assert!(search_source.contains("dropdown_rect"));
        assert!(search_source.contains("search_rect"));
        assert!(search_source.contains(".vertical_align(Align::Center)"));
        assert!(search_source.contains("search_dimension_dropdown("));
        assert!(search_source.contains("search_submit_button("));
        assert!(search_source.contains("file_search_busy"));
        assert!(search_source.contains("add_enabled_ui(!file_search_busy"));
        assert!(!search_source.contains("content_panel_frame(theme::bg())"));
        assert!(search_source.contains("start_file_change_search()"));
        assert!(search_source.contains("key_pressed(egui::Key::Enter)"));
        assert!(search_source.contains("self.file_search_hashes.clear()"));
        assert!(source.contains("FILE_SEARCH_TIMEOUT"));
        assert!(search_source.contains("self.search_bottom_pane(ui)"));
        assert!(source.contains("search_view_query: String"));
        assert!(source.contains("search_selected_commit: Option<usize>"));
        assert!(source.contains("fn selected_commit_for_search("));
        assert!(source.contains("fn search_diff_viewer("));
        assert!(source.contains("fn select_first_search_changed_file_if_cached("));
        assert!(source.contains("fn select_first_history_changed_file_if_cached("));
        assert!(!search_source.contains("ComboBox::from_id_salt(\"search_dimension\")"));
        assert!(!search_source.contains("commit_details_only(ui)"));

        let header_start = source.find("fn search_table_header(").unwrap();
        let row_end = source[header_start..].find("fn empty_list_panel(").unwrap();
        let table_source = &source[header_start..header_start + row_end];
        assert!(table_source.contains("search_column_widths("));
        assert!(table_source.contains("draw_clipped_cell("));
        assert!(table_source.contains("ui.ctx().copy_text(commit.hash.clone())"));
        assert!(table_source.contains("on_hover_text(commit.hash.as_str())"));
        assert!(!table_source.contains("ui.horizontal(|ui|"));
        assert!(table_source.contains("HISTORY_TABLE_ROW_HEIGHT"));

        let dropdown_start = source.find("fn search_dimension_dropdown(").unwrap();
        let dropdown_end = source[dropdown_start..]
            .find("#[derive(Clone, Copy, Debug)]")
            .unwrap();
        let dropdown_source = &source[dropdown_start..dropdown_start + dropdown_end];
        assert!(dropdown_source.contains("history_toolbar_popup_option("));
        assert!(dropdown_source.contains("fn search_submit_button("));
        assert!(dropdown_source.contains("UiIcon::Search"));
        assert!(dropdown_source.contains("ui.spinner()"));
    }

    #[test]
    fn file_search_matches_only_current_hash_result_set() {
        let commit = Commit {
            hash: "abc123".to_owned(),
            short_hash: "abc123".to_owned(),
            subject: "unrelated subject".to_owned(),
            ..Commit::default()
        };
        let mut hashes = HashSet::new();
        hashes.insert("abc123".to_owned());

        assert!(history_commit_matches_search(
            &commit,
            SearchDimension::Files,
            "pretty",
            &HashMap::new(),
            "pretty",
            &hashes,
        ));
        assert!(!history_commit_matches_search(
            &commit,
            SearchDimension::Files,
            "other",
            &HashMap::new(),
            "pretty",
            &hashes,
        ));
    }

    #[test]
    fn history_virtual_rows_use_viewport_math_and_hash_cache() {
        let viewport = Rect::from_min_size(Pos2::new(0.0, 220.0), Vec2::new(800.0, 110.0));
        let range = history_virtual_row_range(viewport, HISTORY_TABLE_ROW_HEIGHT, 10_000);
        assert!(range.start < 10);
        assert!(range.end > range.start);
        assert!(range.end - range.start <= 16);

        let source = include_str!("app.rs");
        let cache_start = source.find("struct HistoryRowsCache").unwrap();
        let cache_end = source[cache_start..].find("impl HistoryRowsCache").unwrap();
        let cache_struct = &source[cache_start..cache_start + cache_end];
        assert!(cache_struct.contains("visible_hashes: Vec<String>"));
        assert!(cache_struct.contains("hash_to_index: HashMap<String, usize>"));
        assert!(source.contains("self.history_rows_cache.clear();"));
    }

    #[test]
    fn history_graph_nodes_are_small_and_unbordered_with_wider_lanes() {
        let source = include_str!("app.rs");
        let start = source.find("fn draw_history_graph_cell(").unwrap();
        let end = source[start..]
            .find("fn draw_history_description(")
            .unwrap();
        let graph_source = &source[start..start + end];
        assert!(graph_source.contains(".clamp(7.0, 22.0)"));
        assert!(graph_source.contains("circle_filled(node, 2.8, color)"));
        assert!(!graph_source.contains("circle_stroke(node"));
    }

    #[test]
    fn history_ref_badges_use_measured_text_and_inner_clip() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let start = implementation_source
            .find("fn draw_history_description(")
            .unwrap();
        let end = implementation_source[start..]
            .find("fn commit_refs_for_display(")
            .unwrap();
        let desc_source = &implementation_source[start..start + end];

        assert!(desc_source.contains("history_ref_badge_label_for_width("));
        assert!(desc_source.contains("layout_no_wrap("));
        assert!(desc_source.contains("galley.size().x"));
        assert!(desc_source.contains("with_clip_rect(text_clip"));
        assert!(!desc_source.contains("label.chars().count() as f32 * 6.6"));
        assert!(!desc_source.contains("Align2::CENTER_CENTER"));
    }

    #[test]
    fn history_sort_dropdown_offers_date_and_topology_order() {
        assert_eq!(
            history_sort_order_label(Language::Chinese, HistorySortOrder::Date),
            "\u{6309}\u{65e5}\u{671f}\u{6392}\u{5e8f}"
        );
        assert_eq!(
            history_sort_order_label(Language::Chinese, HistorySortOrder::Topology),
            "\u{5c42}\u{7ea7}\u{6392}\u{5e8f}"
        );

        let source = include_str!("app.rs");
        assert!(source.contains("history_sort_order: HistorySortOrder::Date"));
        assert!(source.contains("history_branch_scope: HistoryBranchScope::Current"));
        assert_eq!(
            history_branch_scope_label(Language::Chinese, HistoryBranchScope::Current),
            "\u{5f53}\u{524d}\u{5206}\u{652f}"
        );
        assert_eq!(
            history_branch_scope_label(Language::Chinese, HistoryBranchScope::All),
            "\u{6240}\u{6709}\u{5206}\u{652f}"
        );
        assert!(source.contains("self.set_history_sort_order(order)"));
        assert!(source.contains("self.set_history_branch_scope(scope)"));
        assert!(source.contains("HistorySortOrder::Topology"));
        assert!(source.contains("fn history_sort_order_dropdown("));
        assert!(source.contains("fn history_branch_scope_dropdown("));
        assert!(source.contains("Color32::WHITE"));
        assert!(
            !source.contains("ui.selectable_value(\n                            &mut sort_order")
        );

        let git_source = include_str!("git.rs");
        assert!(git_source.contains("CommitOrder::Date => \"--date-order\""));
        assert!(git_source.contains("CommitOrder::Topology => \"--topo-order\""));
        assert!(git_source.contains("topology_commits"));
        assert!(git_source.contains("all_topology_commits"));
        assert!(git_source.contains("args.push(\"--all\")"));
    }

    #[test]
    fn history_filter_controls_share_one_centered_row() {
        let source = include_str!("app.rs");
        let start = source
            .find("Vec2::new(ui.available_width(), 28.0)")
            .unwrap();
        let end = source[start..].find("ui.add_space(4.0);").unwrap();
        let controls_source = &source[start..start + end];
        assert!(!controls_source.contains("ui.allocate_ui_with_layout("));
        assert!(!controls_source.contains("egui::ComboBox::from_id_salt(\"history_sort_order\")"));
        assert!(controls_source.contains("let control_top = toolbar_rect.top() + 2.0"));
        assert!(controls_source.contains("let control_height = 24.0"));
        assert!(controls_source.contains("history_branch_scope_dropdown("));
        assert!(controls_source.contains("history_sort_order_dropdown("));
        assert!(controls_source.contains("history_toolbar_checkbox_at("));
        assert!(!controls_source.contains("egui::Checkbox::new("));

        let dropdown_start = source.find("fn history_toolbar_dropdown_button(").unwrap();
        let dropdown_end = source[dropdown_start..]
            .find("fn history_toolbar_popup_option(")
            .unwrap();
        let dropdown_source = &source[dropdown_start..dropdown_start + dropdown_end];
        assert!(dropdown_source.contains("(id_salt, \"button\")"));
        assert!(dropdown_source.contains("(id_salt, \"popup\")"));
        assert!(dropdown_source.contains("ui.interact(rect, button_id"));
        assert!(dropdown_source.contains("toggle_popup(popup_id)"));

        let checkbox_start = source.find("fn history_toolbar_checkbox_at(").unwrap();
        let checkbox_end = source[checkbox_start..]
            .find("fn history_graph_width(")
            .unwrap();
        let checkbox_source = &source[checkbox_start..checkbox_start + checkbox_end];
        assert!(checkbox_source.contains("ui.interact("));
        assert!(checkbox_source.contains("Align2::LEFT_CENTER"));
    }

    #[test]
    fn history_supports_batch_cherry_pick_selection_mode() {
        let source = include_str!("app.rs");
        assert!(source.contains("history_cherry_pick_mode: bool"));
        assert!(source.contains("selected_cherry_pick_hashes: HashSet<String>"));
        assert!(source.contains("ConfirmCherryPickBatch"));
        assert!(source.contains("fn selected_cherry_pick_commits_in_apply_order("));
        assert!(source.contains("fn clear_cherry_pick_selection("));

        let table_start = source.find("fn history_commit_table(").unwrap();
        let table_end = source[table_start..]
            .find("fn history_bottom_pane(")
            .unwrap();
        let table_source = &source[table_start..table_start + table_end];
        assert!(table_source.contains("self.history_cherry_pick_mode"));
        assert!(table_source.contains("self.tr(\"commit.cherry_pick_batch\")"));
        assert!(table_source.contains("self.tr(\"commit.cherry_pick_confirm\")"));
        assert!(table_source.contains("self.tr(\"dialog.cancel\")"));
        assert!(table_source.contains("select_for_cherry_pick"));
        assert!(table_source.contains("self.toggle_cherry_pick_hash("));

        let row_start = source.find("fn history_commit_table_row(").unwrap();
        let row_end = source[row_start..]
            .find("fn draw_history_graph_cell(")
            .unwrap();
        let row_source = &source[row_start..row_start + row_end];
        assert!(row_source.contains("history_cherry_pick_checkbox_at"));
    }

    #[test]
    fn commit_panel_supports_persisted_options_and_message_history() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        assert!(implementation_source.contains("struct RepoCommitState"));
        assert!(implementation_source.contains("commit_state: RepoCommitState"));
        assert!(implementation_source.contains("fn load_commit_state_for_active_repo("));
        assert!(implementation_source.contains("fn save_commit_state_for_active_repo("));
        assert!(implementation_source.contains("push_immediately"));
        assert!(implementation_source.contains("amend"));
        assert!(implementation_source.contains("no_verify"));
        assert!(implementation_source.contains("gpg_sign"));
        assert!(implementation_source.contains("message_history"));
        assert!(implementation_source.contains("commit_history_menu("));
        assert!(implementation_source.contains("commit_history_icon_menu("));
        assert!(implementation_source.contains("commit_options_menu("));
        assert!(implementation_source.contains("commit_checkbox("));
        assert!(implementation_source.contains("UiIcon::History"));
        assert!(implementation_source.contains("git::CommitOptions"));
        assert!(
            implementation_source.contains("git::commit_with_options(root, &message, options)")
        );
        assert!(implementation_source.contains("git::push(root)"));
        assert!(implementation_source.contains("self.add_commit_message_history(message.clone())"));

        let panel_start = implementation_source.find("fn commit_panel(").unwrap();
        let panel_end = implementation_source[panel_start..]
            .find("fn diff_viewer(")
            .unwrap();
        let panel_source = &implementation_source[panel_start..panel_start + panel_end];
        assert!(
            panel_source.contains("commit_checkbox(ui, &mut self.commit_state.push_immediately")
        );
        assert!(panel_source.contains("commit_checkbox(ui, &mut self.commit_state.amend"));
        assert!(panel_source.contains("Layout::right_to_left(Align::Center)"));
        assert!(panel_source.contains("self.tr(\"commit.button.short\")"));
        assert!(panel_source.contains("commit_history_menu("));
        assert!(panel_source.contains("commit_options_menu("));
    }

    #[test]
    fn commit_context_menu_groups_compare_actions_in_submenu() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let menu_start = implementation_source
            .find("fn commit_context_menu(")
            .unwrap();
        let menu_end = implementation_source[menu_start..]
            .find("fn detail_line(")
            .unwrap();
        let menu_source = &implementation_source[menu_start..menu_start + menu_end];

        assert!(menu_source.contains("ui.menu_button(i18n::t(language, \"menu.compare\")"));
        assert!(menu_source.contains("CommitMenuAction::CompareWithWorktree"));
        assert!(menu_source.contains("CommitMenuAction::ExternalDiff"));
        assert!(menu_source.contains("CommitMenuAction::OpenRemote"));
        assert!(!menu_source.contains("ui.add_enabled(\n        false"));
        assert!(
            !menu_source
                .contains("egui::Button::new(i18n::t(language, \"menu.compare_worktree\"))")
        );
        assert!(
            !menu_source.contains("egui::Button::new(i18n::t(language, \"menu.open_remote\"))")
        );

        let handler_start = implementation_source
            .find("fn handle_commit_menu_action(")
            .unwrap();
        let handler_end = implementation_source[handler_start..]
            .find("fn handle_worktree_action(")
            .unwrap();
        let handler_source = &implementation_source[handler_start..handler_start + handler_end];
        assert!(handler_source.contains("CommitMenuAction::CompareWithWorktree"));
        assert!(handler_source.contains("CommitMenuAction::ExternalDiff"));
        assert!(handler_source.contains("CommitMenuAction::OpenRemote"));
        assert!(handler_source.contains("self.open_commit_diff_tool("));
        assert!(handler_source.contains("self.open_commit_remote_url("));

        let diff_tool_start = implementation_source
            .find("fn open_commit_diff_tool(")
            .unwrap();
        let diff_tool_end = implementation_source[diff_tool_start..]
            .find("fn default_remote_web_url(")
            .unwrap();
        let diff_tool_source =
            &implementation_source[diff_tool_start..diff_tool_start + diff_tool_end];
        assert!(diff_tool_source.contains(".arg(\"worktree\")"));
        assert!(!diff_tool_source.contains(".arg(\"working tree\")"));
    }

    #[test]
    fn commit_submission_uses_async_loading_gate() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        let commit_start = implementation_source
            .find("fn commit_current_message(&mut self")
            .unwrap();
        let commit_end = implementation_source[commit_start..]
            .find("fn show_toast(")
            .unwrap();
        let commit_source = &implementation_source[commit_start..commit_start + commit_end];
        assert!(commit_source.contains("self.loading_repo || self.remote_git_busy()"));
        assert!(commit_source.contains("self.start_remote_git_action(move |root|"));
        assert!(!commit_source.contains("self.execute_git_action(move |root|"));

        let panel_start = implementation_source.find("fn commit_panel(").unwrap();
        let panel_end = implementation_source[panel_start..]
            .find("fn commit_action_row(")
            .unwrap();
        let panel_source = &implementation_source[panel_start..panel_start + panel_end];
        assert!(panel_source.contains("&& !self.loading_repo"));
        assert!(panel_source.contains("&& !self.remote_git_busy()"));
    }

    #[test]
    fn generic_git_actions_use_loading_gate() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        let action_start = implementation_source
            .find("fn execute_git_action(")
            .unwrap();
        let action_end = implementation_source[action_start..]
            .find("fn branch_checkout_busy(")
            .unwrap();
        let action_source = &implementation_source[action_start..action_start + action_end];
        assert!(action_source.contains("Send + 'static"));
        assert!(action_source.contains("self.start_remote_git_action(action);"));
        assert!(!action_source.contains("match action(&root)"));
        assert!(!action_source.contains("self.load_repository(root)"));
    }

    #[test]
    fn commit_panel_keeps_submit_button_visible_when_short() {
        assert_eq!(
            commit_message_editor_height(120.0),
            120.0 - COMMIT_BUTTON_ROW_HEIGHT - COMMIT_MESSAGE_BOTTOM_GAP
        );
        assert!(commit_message_editor_height(220.0) > 150.0);

        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let panel_start = implementation_source.find("fn commit_panel(").unwrap();
        let panel_end = implementation_source[panel_start..]
            .find("fn commit_history_icon_menu(")
            .unwrap();
        let panel_source = &implementation_source[panel_start..panel_start + panel_end];

        assert!(panel_source.contains("commit_message_editor_ui("));
        assert!(implementation_source.contains("fn commit_message_editor_ui("));
        assert!(implementation_source.contains("fn commit_submit_button("));
        assert!(implementation_source.contains("COMMIT_SUBMIT_BUTTON_SIZE"));
        assert!(!panel_source.contains("commit.staged_files"));
        assert!(!panel_source.contains("staged_count}"));
        assert!(!panel_source.contains("egui::Button::new(commit_button_text)"));
        assert!(panel_source.contains(
            "fn commit_panel(&mut self, ui: &mut Ui, staged_count: usize, panel_height: f32)"
        ));
        assert!(!panel_source.contains("let panel_height = ui.available_height()"));
        assert!(panel_source.contains("ui.allocate_exact_size("));
        assert!(panel_source.contains("Vec2::new(ui.available_width(), panel_height)"));
        assert!(
            panel_source.contains(
                "let message_height = commit_message_editor_height(ui.available_height())"
            )
        );
        assert!(panel_source.contains("COMMIT_BUTTON_ROW_HEIGHT"));
        assert!(panel_source.contains("ui.available_height() - COMMIT_BUTTON_ROW_HEIGHT"));
        assert!(panel_source.contains("COMMIT_MESSAGE_BOTTOM_GAP"));
        assert!(panel_source.contains("commit_action_row("));
        assert!(
            !panel_source.contains("ui.add_space(6.0);\r\n                ui.horizontal(|ui|")
                && !panel_source.contains("ui.add_space(6.0);\n                ui.horizontal(|ui|")
        );
    }

    #[test]
    fn workspace_commit_panel_starts_without_wasted_top_gap() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let workspace_start = implementation_source
            .find("fn workspace_main_panel(")
            .unwrap();
        let workspace_end = implementation_source[workspace_start..]
            .find("fn search_view(")
            .unwrap();
        let workspace_source =
            &implementation_source[workspace_start..workspace_start + workspace_end];
        let panel_start = implementation_source.find("fn commit_panel(").unwrap();
        let panel_end = implementation_source[panel_start..]
            .find("fn commit_history_icon_menu(")
            .unwrap();
        let panel_source = &implementation_source[panel_start..panel_start + panel_end];

        assert!(implementation_source.contains("const WORKSPACE_LIST_COMMIT_GAP: f32 = 2.0;"));
        assert!(
            implementation_source
                .contains("let list_commit_gap = WORKSPACE_LIST_COMMIT_GAP.min(available_height);")
        );
        assert!(workspace_source.contains("workspace_main_layout("));
        assert!(
            workspace_source
                .contains("self.commit_panel(ui, staged.len(), layout.commit_rect.height())")
        );
        assert!(!panel_source.contains("ui.add_space(14.0)"));
        assert!(!panel_source.contains("let panel_height = ui.available_height()"));
        assert!(panel_source.contains("panel_rect"));
        assert!(panel_source.contains("ui.allocate_exact_size"));
        assert!(panel_source.contains("egui::UiBuilder::new().max_rect(panel_rect)"));
    }

    #[test]
    fn commit_message_editor_uses_theme_text_and_readable_selection() {
        let app_source = include_str!("app.rs");
        let implementation_source = &app_source[..app_source.find("#[cfg(test)]").unwrap()];
        let editor_start = implementation_source
            .find("fn commit_message_text_edit")
            .unwrap();
        let editor_end = implementation_source[editor_start..]
            .find("fn commit_checkbox(")
            .unwrap();
        let editor_source = &implementation_source[editor_start..editor_start + editor_end];
        let editor_ui_start = implementation_source
            .find("fn commit_message_editor_ui")
            .unwrap();
        let editor_ui_end = implementation_source[editor_ui_start..]
            .find("fn commit_submit_button(")
            .unwrap();
        let editor_ui_source =
            &implementation_source[editor_ui_start..editor_ui_start + editor_ui_end];
        let submit_start = implementation_source
            .find("fn commit_submit_button(")
            .unwrap();
        let submit_end = implementation_source[submit_start..]
            .find("fn commit_checkbox(")
            .unwrap();
        let submit_source = &implementation_source[submit_start..submit_start + submit_end];
        let helper_start = implementation_source
            .find("fn themed_text_edit_selection")
            .unwrap();
        let helper_end = implementation_source[helper_start..]
            .find("fn commit_message_text_edit")
            .unwrap();
        let helper_source = &implementation_source[helper_start..helper_start + helper_end];
        let theme_source = include_str!("theme.rs");

        assert!(editor_source.contains(".text_color(theme::text())"));
        assert!(editor_ui_source.contains("themed_text_edit_selection(ui);"));
        assert!(helper_source.contains("selection.bg_fill = theme::accent_soft()"));
        assert!(
            helper_source.contains("selection.stroke = Stroke::new(1.0, theme::accent_deep())")
        );
        assert!(submit_source.contains("theme::accent_deep()"));
        assert!(submit_source.contains("Color32::WHITE"));
        assert!(submit_source.contains("Sense::click()"));
        assert!(submit_source.contains("Sense::hover()"));
        assert!(theme_source.contains("visuals.selection.stroke"));
        assert!(
            theme_source.contains("visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE)")
        );
    }

    #[test]
    fn repository_source_search_uses_readable_text_input_and_no_top_gap() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let view_start = implementation_source
            .find("fn repository_source_view")
            .unwrap();
        let view_end = implementation_source[view_start..]
            .find("fn local_repository_source_page")
            .unwrap();
        let view_source = &implementation_source[view_start..view_start + view_end];
        let search_start = implementation_source
            .find("fn local_repository_source_page")
            .unwrap();
        let search_end = implementation_source[search_start..]
            .find("fn remote_repository_source_page")
            .unwrap();
        let search_source = &implementation_source[search_start..search_start + search_end];
        let helper_start = implementation_source
            .find("fn themed_text_edit_selection")
            .unwrap();
        let helper_end = implementation_source[helper_start..]
            .find("fn commit_message_text_edit")
            .unwrap();
        let helper_source = &implementation_source[helper_start..helper_start + helper_end];

        assert!(!view_source.contains("ui.add_space(12.0);"));
        assert!(search_source.contains("themed_text_edit_selection(ui);"));
        assert!(search_source.contains("themed_singleline_text_edit("));
        assert!(helper_source.contains(".text_color(theme::text())"));
        assert!(helper_source.contains("selection.bg_fill = theme::accent_soft()"));
        assert!(
            helper_source.contains("selection.stroke = Stroke::new(1.0, theme::accent_deep())")
        );
    }

    #[test]
    fn source_tab_strip_is_bottom_aligned_without_tool_row_gap() {
        let tab_row = Rect::from_min_max(Pos2::new(0.0, 32.0), Pos2::new(900.0, 72.0));
        let source_strip = repo_tab_strip_rect(tab_row, true);
        let repo_strip = repo_tab_strip_rect(tab_row, false);

        assert_eq!(source_strip.height(), REPO_TAB_HEIGHT);
        assert_eq!(source_strip.bottom(), tab_row.bottom());
        assert_eq!(repo_strip, tab_row);

        let source_island = top_island_rect(
            Rect::from_min_max(Pos2::ZERO, Pos2::new(900.0, 72.0)),
            tab_row,
            tab_row,
            true,
        );
        assert_eq!(source_island.bottom(), tab_row.bottom());
    }

    #[test]
    fn source_tabs_leave_empty_tab_strip_available_for_window_drag() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let top_bar_start = implementation_source.find("fn top_bar_panel(").unwrap();
        let top_bar_end = implementation_source[top_bar_start..]
            .find("fn repository_source_view(")
            .unwrap();
        let top_bar_source = &implementation_source[top_bar_start..top_bar_start + top_bar_end];

        assert!(top_bar_source.contains("repo_tab_strip_rect(tab_row, source_active)"));
        assert!(top_bar_source.contains("source_tab_left_drag_region"));
        assert!(top_bar_source.contains("if source_active {"));
    }

    #[test]
    fn source_tab_drag_regions_avoid_hot_path_pointer_diagnostics() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let top_bar_start = implementation_source.find("fn top_bar_panel(").unwrap();
        let top_bar_end = implementation_source[top_bar_start..]
            .find("fn repository_source_view(")
            .unwrap();
        let top_bar_source = &implementation_source[top_bar_start..top_bar_start + top_bar_end];
        let drag_start = implementation_source
            .find("fn top_bar_drag_region(")
            .unwrap();
        let drag_end = implementation_source[drag_start..]
            .find("fn custom_title_bar(")
            .unwrap();
        let drag_source = &implementation_source[drag_start..drag_start + drag_end];
        let title_bar_start = implementation_source.find("fn custom_title_bar(").unwrap();
        let title_bar_end = implementation_source[title_bar_start..]
            .find("fn desktop_menu_bar(")
            .unwrap();
        let title_bar_source =
            &implementation_source[title_bar_start..title_bar_start + title_bar_end];

        assert!(!implementation_source.contains("fn append_drag_log("));
        assert!(!implementation_source.contains("fn drag_logging_enabled("));
        assert!(!implementation_source.contains("GIT_AGENT_DRAG_DEBUG"));
        assert!(!top_bar_source.contains("source-tab pointer-down"));
        assert!(!top_bar_source.contains("tab_left.contains(pos)"));
        assert!(!top_bar_source.contains("tab_right.contains(pos)"));
        assert!(!drag_source.contains("drag-region"));
        assert!(implementation_source.contains("fn custom_title_drag_rect("));
        assert!(!implementation_source.contains("fn top_bar_press_drag_region("));
        assert!(top_bar_source.contains("source_title_gap_drag_region"));
        assert!(!top_bar_source.contains("source_title_row_drag_region"));
        assert!(
            title_bar_source
                .contains("top_bar_drag_region(ctx, ui, drag_rect, \"custom_title_drag_region\")")
        );
        assert!(top_bar_source.contains("top_bar_drag_region("));
        assert!(!top_bar_source.contains("top_bar_press_drag_region("));
    }

    #[test]
    fn custom_title_bar_drag_is_independent_from_active_tab_kind() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let title_bar_start = implementation_source.find("fn custom_title_bar(").unwrap();
        let title_bar_end = implementation_source[title_bar_start..]
            .find("fn desktop_menu_bar(")
            .unwrap();
        let title_bar_source =
            &implementation_source[title_bar_start..title_bar_start + title_bar_end];
        let top_bar_start = implementation_source.find("fn top_bar_panel(").unwrap();
        let top_bar_end = implementation_source[top_bar_start..]
            .find("fn repository_source_view(")
            .unwrap();
        let top_bar_source = &implementation_source[top_bar_start..top_bar_start + top_bar_end];

        assert!(!title_bar_source.contains("source_active"));
        assert!(
            title_bar_source
                .contains("top_bar_drag_region(ctx, ui, drag_rect, \"custom_title_drag_region\")")
        );
        assert!(top_bar_source.contains("self.custom_title_bar(ctx, ui, has_repo, has_remote);"));
        assert!(
            !top_bar_source
                .contains("self.custom_title_bar(ctx, ui, has_repo, has_remote, source_active);")
        );
    }

    #[test]
    fn window_drag_request_keeps_content_painting() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let update_start = implementation_source
            .find("impl App for GitAgentApp")
            .unwrap();
        let update_end = implementation_source[update_start..]
            .find("impl GitAgentApp")
            .unwrap();
        let update_source = &implementation_source[update_start..update_start + update_end];
        let drag_start = implementation_source
            .find("fn top_bar_drag_region(")
            .unwrap();
        let drag_end = implementation_source[drag_start..]
            .find("fn custom_title_bar(")
            .unwrap();
        let drag_source = &implementation_source[drag_start..drag_start + drag_end];

        assert!(drag_source.contains("ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);"));
        assert!(drag_source.contains("response.drag_started()"));
        assert!(!drag_source.contains("if pointer_pressed && response.hovered()"));
        assert!(!update_source.contains("if self.window_drag_requested_this_frame"));
        assert!(update_source.contains("egui::CentralPanel::default()"));
    }

    #[test]
    fn repository_source_list_filters_cached_repositories_without_disk_scan() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let app_start = implementation_source
            .find("pub struct GitAgentApp")
            .unwrap();
        let app_end = implementation_source[app_start..]
            .find("#[derive(Clone, Debug)]")
            .unwrap();
        let app_source = &implementation_source[app_start..app_start + app_end];
        let filter_start = implementation_source
            .find("fn filtered_known_repositories(")
            .unwrap();
        let filter_end = implementation_source[filter_start..]
            .find("fn refresh_known_repositories(")
            .unwrap();
        let filter_source = &implementation_source[filter_start..filter_start + filter_end];
        let refresh_start = implementation_source
            .find("fn refresh_known_repositories(")
            .unwrap();
        let refresh_end = implementation_source[refresh_start..]
            .find("fn sidebar(")
            .unwrap();
        let refresh_source = &implementation_source[refresh_start..refresh_start + refresh_end];
        let compact_filter_source = filter_source.split_whitespace().collect::<String>();

        assert!(app_source.contains("known_repositories: Vec<KnownRepository>"));
        assert!(compact_filter_source.contains("self.known_repositories.iter()"));
        assert!(!filter_source.contains("scan_repository_children("));
        assert!(refresh_source.contains("scan_repository_children("));
    }

    #[test]
    fn repository_source_file_perf_logging_is_removed_after_diagnosis() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        assert!(!implementation_source.contains("fn perf_logging_enabled("));
        assert!(!implementation_source.contains("fn append_perf_log("));
        assert!(!implementation_source.contains("fn log_perf_sample("));
        assert!(!implementation_source.contains("GIT_AGENT_PERF_DEBUG"));
        assert!(!implementation_source.contains("perf.log"));
    }

    #[test]
    fn workspace_main_layout_cuts_children_from_one_body_rect() {
        let body = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(1000.0, 700.0));
        let layout = workspace_main_layout(body, 0.58, 0.5);

        assert_eq!(layout.staged_rect.left(), body.left());
        assert_eq!(layout.unstaged_rect.left(), body.left());
        assert_eq!(layout.commit_rect.left(), body.left());
        assert_eq!(layout.staged_rect.right(), body.right());
        assert_eq!(layout.unstaged_rect.right(), body.right());
        assert_eq!(layout.commit_rect.right(), body.right());
        assert_eq!(layout.commit_rect.bottom(), body.bottom());
        assert_eq!(
            layout.staged_unstaged_splitter_rect.top(),
            layout.staged_rect.bottom()
        );
        assert_eq!(
            layout.list_commit_splitter_rect.top(),
            layout.unstaged_rect.bottom()
        );

        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let workspace_start = implementation_source
            .find("fn workspace_main_panel(")
            .unwrap();
        let workspace_end = implementation_source[workspace_start..]
            .find("fn search_view(")
            .unwrap();
        let workspace_source =
            &implementation_source[workspace_start..workspace_start + workspace_end];
        assert!(workspace_source.contains("let (body_rect, _) = ui.allocate_exact_size"));
        assert!(workspace_source.contains("workspace_main_layout("));
        assert!(workspace_source.contains("max_rect(layout.staged_rect)"));
        assert!(workspace_source.contains("max_rect(layout.unstaged_rect)"));
        assert!(workspace_source.contains("max_rect(layout.commit_rect)"));
    }

    #[test]
    fn workspace_main_layout_never_returns_negative_child_rects() {
        let body = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(310.0, 110.0));
        let layout = workspace_main_layout(body, 0.58, 0.5);

        for rect in [
            layout.staged_rect,
            layout.staged_unstaged_splitter_rect,
            layout.unstaged_rect,
            layout.list_commit_splitter_rect,
            layout.commit_rect,
        ] {
            assert!(
                rect.width() >= 0.0 && rect.height() >= 0.0,
                "workspace child rect must be non-negative: {rect:?}"
            );
            assert!(rect.top() >= body.top());
            assert!(rect.bottom() <= body.bottom());
        }
    }

    #[test]
    fn workspace_cards_use_recessed_shadow_inside_raised_workspace() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let table_start = implementation_source.find("fn worktree_table(").unwrap();
        let table_end = implementation_source[table_start..]
            .find("fn clean_worktree_state(")
            .unwrap();
        let table_source = &implementation_source[table_start..table_start + table_end];
        let panel_start = implementation_source.find("fn commit_panel(").unwrap();
        let panel_end = implementation_source[panel_start..]
            .find("fn commit_history_icon_menu(")
            .unwrap();
        let panel_source = &implementation_source[panel_start..panel_start + panel_end];
        let frame_start = implementation_source
            .find("fn workspace_card_frame(")
            .unwrap();
        let frame_end = implementation_source[frame_start..]
            .find("fn soft_panel_frame(")
            .unwrap();
        let frame_source = &implementation_source[frame_start..frame_start + frame_end];
        let clean_start = implementation_source
            .find("fn clean_worktree_state(")
            .unwrap();
        let clean_end = implementation_source[clean_start..]
            .find("fn branch_table_row(")
            .unwrap();
        let clean_source = &implementation_source[clean_start..clean_start + clean_end];

        assert!(table_source.contains("workspace_card_clip_rect(rect)"));
        assert!(table_source.contains("workspace_card_frame(10, 8)"));
        assert!(table_source.contains("let panel_rect = rect;"));
        assert!(table_source.contains("paint_workspace_card_inset_shadow(ui, panel_rect)"));
        assert!(!table_source.contains("rect.shrink(2.0)"));
        assert!(panel_source.contains("workspace_card_frame(12, 10)"));
        assert!(panel_source.contains("paint_workspace_card_inset_shadow(ui, card_rect)"));
        assert!(!panel_source.contains("theme::accent_soft()"));
        assert!(frame_source.contains(".fill(theme::panel_recessed())"));
        assert!(frame_source.contains(".corner_radius(CornerRadius::same(WORKSPACE_CARD_RADIUS))"));
        assert!(!frame_source.contains(".shadow(panel_shadow())"));
        assert!(frame_source.contains("fn paint_workspace_card_inset_shadow("));
        assert!(frame_source.contains("workspace_card_shadow_dark()"));
        assert!(frame_source.contains("workspace_card_shadow_light()"));
        assert!(frame_source.contains("WorkspaceInsetEdge::Top"));
        assert!(frame_source.contains("WorkspaceInsetEdge::Left"));
        assert!(frame_source.contains("WorkspaceInsetEdge::Bottom"));
        assert!(frame_source.contains("WorkspaceInsetEdge::Right"));
        assert!(!frame_source.contains("painter.rect_stroke"));
        assert!(!frame_source.contains("egui::StrokeKind::Inside"));
        assert!(!frame_source.contains("paint_workspace_card_edge_shadow("));
        assert!(!frame_source.contains("fn add_arc_points("));
        assert!(
            clean_source.contains(
                "ui.allocate_exact_size(safe_ui_size(ui.available_size()), Sense::hover())"
            )
        );
        assert!(clean_source.contains("ui.painter().text("));
        assert!(!clean_source.contains("soft_panel_frame("));
        assert!(!clean_source.contains("panel_shadow()"));
        assert!(!clean_source.contains("paint_workspace_card_inset_shadow("));
    }

    #[test]
    fn workspace_main_layout_quantizes_card_edges_to_whole_pixels() {
        let body = Rect::from_min_size(Pos2::new(8.0, 148.0), Vec2::new(1013.0, 697.3));
        let layout = workspace_main_layout(body, 0.58, 0.5);

        for value in [
            layout.staged_rect.top(),
            layout.staged_rect.bottom(),
            layout.staged_unstaged_splitter_rect.top(),
            layout.staged_unstaged_splitter_rect.bottom(),
            layout.unstaged_rect.top(),
            layout.unstaged_rect.bottom(),
            layout.list_commit_splitter_rect.top(),
            layout.list_commit_splitter_rect.bottom(),
            layout.commit_rect.top(),
            layout.commit_rect.bottom(),
        ] {
            assert_eq!(value, value.round());
        }
    }

    #[test]
    fn workspace_inset_shadow_uses_directional_recessed_light() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let frame_start = implementation_source
            .find("fn paint_workspace_card_inset_shadow(")
            .unwrap();
        let frame_end = implementation_source[frame_start..]
            .find("fn workspace_main_layout(")
            .unwrap();
        let frame_source = &implementation_source[frame_start..frame_start + frame_end];

        assert!(implementation_source.contains("fn workspace_inset_edge_segment("));
        assert!(implementation_source.contains("fn workspace_inset_arc_points("));
        assert!(implementation_source.contains("enum WorkspaceInsetEdge"));
        assert!(implementation_source.contains("enum WorkspaceInsetCorner"));
        assert!(frame_source.contains("WorkspaceInsetEdge::Top"));
        assert!(frame_source.contains("WorkspaceInsetEdge::Left"));
        assert!(frame_source.contains("WorkspaceInsetEdge::Bottom"));
        assert!(frame_source.contains("WorkspaceInsetEdge::Right"));
        assert!(frame_source.contains("WorkspaceInsetCorner::TopLeft"));
        assert!(frame_source.contains("WorkspaceInsetCorner::BottomLeft"));
        assert!(frame_source.contains("WorkspaceInsetCorner::TopRight"));
        assert!(frame_source.contains("WorkspaceInsetCorner::BottomRight"));
        assert!(implementation_source.contains("color.gamma_multiply(alpha)"));
        assert!(!implementation_source.contains("fn workspace_card_inset_clip_rect("));
        assert!(!implementation_source.contains("fn workspace_card_inset_side_clip_rect("));
        assert!(!frame_source.contains("with_clip_rect"));
        assert!(!frame_source.contains("rect_stroke"));
        assert!(!frame_source.contains("translate(Vec2::new(0.35, 0.35))"));
    }

    #[test]
    fn workspace_inset_highlight_does_not_bleed_into_left_bottom_corner() {
        fn close_pos(actual: Option<Pos2>, expected: Pos2) {
            let actual = actual.unwrap();
            assert!((actual.x - expected.x).abs() <= 0.2);
            assert!((actual.y - expected.y).abs() <= 0.2);
        }

        let rect = Rect::from_min_size(Pos2::new(20.0, 30.0), Vec2::new(200.0, 120.0));
        let inset_rect = workspace_inset_rect(rect, 0);
        let radius = workspace_inset_corner_radius(inset_rect, 0);

        let (top_start, top_end) = workspace_inset_edge_segment(rect, WorkspaceInsetEdge::Top, 0);
        let (left_start, left_end) =
            workspace_inset_edge_segment(rect, WorkspaceInsetEdge::Left, 0);
        let (bottom_start, bottom_end) =
            workspace_inset_edge_segment(rect, WorkspaceInsetEdge::Bottom, 0);
        let (right_start, right_end) =
            workspace_inset_edge_segment(rect, WorkspaceInsetEdge::Right, 0);

        assert_eq!(radius, WORKSPACE_CARD_RADIUS as f32);
        assert_eq!(
            top_start,
            Pos2::new(inset_rect.left() + radius, inset_rect.top())
        );
        assert_eq!(
            top_end,
            Pos2::new(inset_rect.right() - radius, inset_rect.top())
        );
        assert_eq!(
            left_start,
            Pos2::new(inset_rect.left(), inset_rect.top() + radius)
        );
        assert_eq!(
            left_end,
            Pos2::new(inset_rect.left(), inset_rect.bottom() - radius)
        );
        assert_eq!(
            bottom_start,
            Pos2::new(inset_rect.left() + radius, inset_rect.bottom())
        );
        assert_eq!(
            bottom_end,
            Pos2::new(inset_rect.right() - radius, inset_rect.bottom())
        );
        assert_eq!(
            right_start,
            Pos2::new(inset_rect.right(), inset_rect.top() + radius)
        );
        assert_eq!(
            right_end,
            Pos2::new(inset_rect.right(), inset_rect.bottom() - radius)
        );

        let top_left_arc = workspace_inset_arc_points(rect, WorkspaceInsetCorner::TopLeft, 0);
        let bottom_left_arc = workspace_inset_arc_points(rect, WorkspaceInsetCorner::BottomLeft, 0);
        let top_right_arc = workspace_inset_arc_points(rect, WorkspaceInsetCorner::TopRight, 0);
        let bottom_right_arc =
            workspace_inset_arc_points(rect, WorkspaceInsetCorner::BottomRight, 0);

        close_pos(top_left_arc.first().copied(), left_start);
        close_pos(top_left_arc.last().copied(), top_start);
        close_pos(bottom_left_arc.first().copied(), bottom_start);
        close_pos(bottom_left_arc.last().copied(), left_end);
        close_pos(top_right_arc.first().copied(), top_end);
        close_pos(top_right_arc.last().copied(), right_start);
        close_pos(bottom_right_arc.first().copied(), right_end);
        close_pos(bottom_right_arc.last().copied(), bottom_end);
    }

    #[test]
    fn commit_panel_uses_layout_rect_height_not_available_height() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let workspace_start = implementation_source
            .find("fn workspace_main_panel(")
            .unwrap();
        let workspace_end = implementation_source[workspace_start..]
            .find("fn search_view(")
            .unwrap();
        let workspace_source =
            &implementation_source[workspace_start..workspace_start + workspace_end];
        let panel_start = implementation_source.find("fn commit_panel(").unwrap();
        let panel_end = implementation_source[panel_start..]
            .find("fn commit_action_row(")
            .unwrap();
        let panel_source = &implementation_source[panel_start..panel_start + panel_end];

        assert!(
            workspace_source
                .contains("self.commit_panel(ui, staged.len(), layout.commit_rect.height())")
        );
        assert!(panel_source.contains(
            "fn commit_panel(&mut self, ui: &mut Ui, staged_count: usize, panel_height: f32)"
        ));
        assert!(!panel_source.contains("let panel_height = ui.available_height();"));
    }

    #[test]
    fn commit_workflow_keyboard_shortcuts_are_wired() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        assert!(implementation_source.contains("focus_commit_message: bool"));
        assert!(implementation_source.contains("fn handle_global_shortcuts("));
        assert!(implementation_source.contains("fn shortcut_pressed("));
        assert!(implementation_source.contains("egui::Key::C"));
        assert!(implementation_source.contains("egui::Key::P"));
        assert!(implementation_source.contains("egui::Key::L"));
        assert!(implementation_source.contains("egui::Key::F"));
        assert!(implementation_source.contains("shortcut_stage_toggle_action"));
        assert!(implementation_source.contains("WorktreeMenuAction::StageAll"));
        assert!(implementation_source.contains("WorktreeMenuAction::UnstageAll"));
        assert!(implementation_source.contains("self.focus_commit_message = true"));
        assert!(implementation_source.contains("self.push_current();"));
        assert!(implementation_source.contains("self.pull_current();"));
        assert!(implementation_source.contains("self.fetch_all();"));

        let panel_start = implementation_source.find("fn commit_panel(").unwrap();
        let panel_end = implementation_source[panel_start..]
            .find("fn commit_history_icon_menu(")
            .unwrap();
        let panel_source = &implementation_source[panel_start..panel_start + panel_end];
        assert!(panel_source.contains("commit_message_input"));
        assert!(panel_source.contains("request_focus()"));
        assert!(panel_source.contains("message_response.has_focus()"));
        assert!(panel_source.contains("egui::Key::Enter"));
        let compact_panel_source = panel_source
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(compact_panel_source.contains(
            "let commit_shortcut = input.modifiers.ctrl && input.key_pressed(egui::Key::Enter);"
        ));
        assert!(
            !compact_panel_source
                .contains("let commit_shortcut = input.modifiers.ctrl && input.modifiers.shift")
        );
        assert!(panel_source.contains("&& !input.modifiers.shift"));
        assert!(panel_source.contains("self.commit_current_message(staged_count)"));
        assert!(panel_source.contains("toggle_push_immediately"));
        assert!(panel_source.contains("toggle_amend"));
    }

    #[test]
    fn upstream_sync_counts_feed_toolbar_and_branch_badges() {
        let counts = UpstreamSyncCounts {
            ahead: 3,
            behind: 5,
        };

        assert_eq!(
            upstream_push_badge(Some(counts)),
            Some("\u{2191}3".to_owned())
        );
        assert_eq!(
            upstream_pull_badge(Some(counts)),
            Some("\u{2193}5".to_owned())
        );
        assert_eq!(
            toolbar_sync_label("Pull", upstream_pull_badge(Some(counts))),
            "Pull  \u{2193}5"
        );
        assert_eq!(
            toolbar_sync_label("Push", upstream_push_badge(Some(counts))),
            "Push  \u{2191}3"
        );
        assert_eq!(
            upstream_pull_badge(Some(UpstreamSyncCounts::default())),
            None
        );
        assert_eq!(
            upstream_push_badge(Some(UpstreamSyncCounts::default())),
            None
        );

        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let toolbar_start = implementation_source.find("fn top_bar_panel(").unwrap();
        let toolbar_end = implementation_source[toolbar_start..]
            .find("fn repository_source_view(")
            .unwrap();
        let toolbar_source = &implementation_source[toolbar_start..toolbar_start + toolbar_end];
        assert!(toolbar_source.contains("upstream_sync_counts(self.snapshot.as_ref())"));
        assert!(toolbar_source.contains("let pull_label = toolbar_sync_label("));
        assert!(toolbar_source.contains("let push_label = toolbar_sync_label("));

        let local_branch_tree_start = implementation_source
            .find("fn local_branch_tree_rows(")
            .unwrap();
        let local_branch_tree_end = implementation_source[local_branch_tree_start..]
            .find("fn remote_branch_tree_rows(")
            .unwrap();
        let local_branch_tree_source = &implementation_source
            [local_branch_tree_start..local_branch_tree_start + local_branch_tree_end];
        assert!(!local_branch_tree_source.contains("branch.current.then_some(upstream_counts)"));
        assert!(local_branch_tree_source.contains("upstream_sync_counts_for_branch(branch)"));

        let branch_row_start = implementation_source.find("fn branch_row(").unwrap();
        let branch_row_end = implementation_source[branch_row_start..]
            .find("fn remote_branch_row(")
            .unwrap();
        let branch_row_source =
            &implementation_source[branch_row_start..branch_row_start + branch_row_end];
        assert!(branch_row_source.contains("paint_branch_row_badges("));
        assert!(branch_row_source.contains("upstream_pull_badge("));
        assert!(branch_row_source.contains("upstream_push_badge("));

        let branch_counts = git::Branch {
            name: "develop".to_owned(),
            current: false,
            remote: false,
            upstream: Some(git::UpstreamStatus {
                name: "origin/develop".to_owned(),
                ahead: 0,
                behind: 8,
            }),
        };
        assert_eq!(
            upstream_pull_badge(upstream_sync_counts_for_branch(&branch_counts)),
            Some("\u{2193}8".to_owned())
        );
    }

    #[test]
    fn ctrl_shift_c_toggles_stage_all_then_unstage_all() {
        let unstaged_file = WorktreeFile {
            worktree_status: 'M',
            path: "src/app.rs".to_owned(),
            display_path: "src/app.rs".to_owned(),
            ..Default::default()
        };
        let staged_file = WorktreeFile {
            index_status: 'M',
            path: "src/git.rs".to_owned(),
            display_path: "src/git.rs".to_owned(),
            ..Default::default()
        };

        let mut snapshot = RepositorySnapshot {
            unstaged: vec![unstaged_file.clone()],
            ..Default::default()
        };
        assert!(matches!(
            shortcut_stage_toggle_action(&snapshot),
            Some(WorktreeMenuAction::StageAll)
        ));

        snapshot.staged = vec![staged_file.clone()];
        snapshot.unstaged = Vec::new();
        assert!(matches!(
            shortcut_stage_toggle_action(&snapshot),
            Some(WorktreeMenuAction::UnstageAll)
        ));

        snapshot.unstaged = vec![unstaged_file];
        assert!(matches!(
            shortcut_stage_toggle_action(&snapshot),
            Some(WorktreeMenuAction::StageAll)
        ));

        snapshot.staged = Vec::new();
        snapshot.unstaged = Vec::new();
        assert!(shortcut_stage_toggle_action(&snapshot).is_none());
    }

    #[test]
    fn ctrl_shift_c_stage_toggle_uses_reserved_shortcut_path() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        let helper_start = implementation_source
            .find("fn stage_toggle_shortcut_pressed(")
            .unwrap();
        let helper_end = implementation_source[helper_start..]
            .find("fn shortcut_stage_toggle_action(")
            .unwrap();
        let helper_source = &implementation_source[helper_start..helper_start + helper_end];
        assert!(helper_source.contains("input.key_pressed(egui::Key::C)"));
        assert!(helper_source.contains("egui::Event::Copy"));
        assert!(helper_source.contains("input.modifiers.shift"));
        assert!(helper_source.contains("input.modifiers.ctrl || input.modifiers.command"));
        assert!(helper_source.contains("!input.modifiers.alt"));

        let shortcuts_start = implementation_source
            .find("fn handle_global_shortcuts(")
            .unwrap();
        let shortcuts_end = implementation_source[shortcuts_start..]
            .find("fn poll_tasks(")
            .unwrap();
        let shortcuts_source =
            &implementation_source[shortcuts_start..shortcuts_start + shortcuts_end];
        let stage_shortcut_index = shortcuts_source
            .find("stage_toggle_shortcut_pressed(ctx)")
            .unwrap();
        let text_input_guard_index = shortcuts_source.find("ctx.wants_keyboard_input()").unwrap();
        assert!(stage_shortcut_index < text_input_guard_index);

        let stage_shortcut_block = &shortcuts_source[stage_shortcut_index..text_input_guard_index];
        assert!(stage_shortcut_block.contains("self.pending_toolbar_single_click = None;"));
        assert!(stage_shortcut_block.contains("shortcut_stage_toggle_action"));
        assert!(stage_shortcut_block.contains("self.handle_worktree_action(action);"));
    }

    #[test]
    fn ctrl_shift_c_stage_toggle_accepts_egui_copy_event() {
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput {
            modifiers: egui::Modifiers {
                ctrl: true,
                shift: true,
                command: true,
                ..Default::default()
            },
            events: vec![egui::Event::Copy],
            ..Default::default()
        });

        assert!(stage_toggle_shortcut_pressed(&ctx));
        let _ = ctx.end_pass();

        let plain_copy_ctx = egui::Context::default();
        plain_copy_ctx.begin_pass(egui::RawInput {
            modifiers: egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            },
            events: vec![egui::Event::Copy],
            ..Default::default()
        });

        assert!(!stage_toggle_shortcut_pressed(&plain_copy_ctx));
        let _ = plain_copy_ctx.end_pass();
    }

    #[test]
    fn history_bottom_pane_uses_top_aligned_source_tree_layout() {
        let source = include_str!("app.rs");
        let start = source.find("fn history_bottom_pane(").unwrap();
        let end = source[start..]
            .find("fn selected_commit_for_history(")
            .unwrap();
        let bottom_source = &source[start..start + end];

        assert!(bottom_source.contains("ui.allocate_exact_size(available"));
        assert!(bottom_source.contains("Layout::top_down(Align::Min)"));
        assert!(!bottom_source.contains("ui.horizontal(|ui|"));

        let panel_start = source.find("fn source_tree_panel_frame(").unwrap();
        let panel_end = source[panel_start..].find("fn diff_panel_frame(").unwrap();
        let panel_source = &source[panel_start..panel_start + panel_end];
        assert!(panel_source.contains(".shadow(panel_shadow())"));
        assert!(!panel_source.contains(".stroke("));

        let start = source.find("fn history_commit_summary(").unwrap();
        let end = source[start..].find("fn history_file_table(").unwrap();
        let summary_source = &source[start..start + end];
        assert!(!summary_source.contains("history_details_scroll"));
        assert!(
            summary_source.contains("details_height = ui.available_height().clamp(96.0, 132.0)")
        );
    }

    #[test]
    fn history_diff_header_reserves_path_width_and_uses_buttons() {
        let source = include_str!("app.rs");
        let start = source.find("fn history_diff_pane(").unwrap();
        let end = source[start..].find("fn diff_viewer(").unwrap();
        let header_source = &source[start..start + end];

        assert!(header_source.contains("let switch_rect = Rect::from_min_size("));
        assert!(header_source.contains("let path_rect = Rect::from_min_max("));
        assert!(header_source.contains("let switch_w = 122.0"));
        assert!(header_source.contains("let rollback_w = 78.0"));
        assert!(header_source.contains("draw_elided_path_label(ui, path_rect, path)"));
        assert!(header_source.contains("diff_display_mode_switch("));
        assert!(
            header_source.contains("header_action_button_at(ui, rollback_rect, rollback_label)")
        );
        assert!(!header_source.contains("ui.horizontal_centered"));

        let helper_start = source.find("fn draw_elided_path_label(").unwrap();
        let helper_end = source[helper_start..]
            .find("fn history_sort_order_label(")
            .unwrap();
        let helper_source = &source[helper_start..helper_start + helper_end];
        assert!(helper_source.contains("elide_start_to_width(ui, path"));
        assert!(helper_source.contains("fn elide_start_to_width("));
        assert!(helper_source.contains("layout_no_wrap"));
        assert!(helper_source.contains("response.on_hover_text(path)"));
        assert!(helper_source.contains("Align2::RIGHT_CENTER"));

        let action_start = source.find("fn header_action_button_at(").unwrap();
        let action_end = source[action_start..]
            .find("fn draw_elided_path_label(")
            .unwrap();
        let action_source = &source[action_start..action_start + action_end];
        assert!(action_source.contains("Align2::CENTER_CENTER"));
        assert!(!action_source.contains("egui::Button::new"));
    }

    #[test]
    fn copied_hash_uses_transient_toast() {
        let source = include_str!("app.rs");
        assert!(source.contains("toast_notice: Option<(String, Instant)>"));
        assert!(source.contains("Instant::now() + Duration::from_secs(1)"));
        assert!(source.contains("self.show_toast(self.tr(\"status.hash_copied\"))"));
    }

    #[test]
    fn completed_actions_do_not_show_success_noise() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        assert!(
            !implementation_source
                .contains("self.show_toast(self.tr(\"status.action_completed\"))")
        );
        assert!(
            !implementation_source.contains(
                "self.last_notice = Some(self.tr(\"status.action_completed\").to_owned())"
            )
        );
    }

    #[test]
    fn git_errors_use_scrollable_dialog_instead_of_sidebar_footer() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        assert!(implementation_source.contains("fn error_modal(&mut self, ctx: &egui::Context)"));
        assert!(implementation_source.contains("self.error_modal(ctx);"));

        let layout_start = implementation_source.find("fn main_layout(").unwrap();
        let layout_end = implementation_source[layout_start..]
            .find("fn workspace_view(")
            .unwrap();
        let layout_source = &implementation_source[layout_start..layout_start + layout_end];
        assert!(!layout_source.contains("if let Some(error) = &self.error"));
        assert!(!layout_source.contains("ui.colored_label(theme::warning(), error)"));

        let modal_start = implementation_source.find("fn error_modal(").unwrap();
        let modal_end = implementation_source[modal_start..]
            .find("fn main_layout(")
            .unwrap();
        let modal_source = &implementation_source[modal_start..modal_start + modal_end];
        assert!(modal_source.contains("ScrollArea::vertical()"));
        assert!(modal_source.contains("TextEdit::multiline(&mut message)"));
        assert!(modal_source.contains("self.tr(\"dialog.error.title\")"));
        assert!(modal_source.contains("self.tr(\"dialog.close\")"));
        assert!(modal_source.contains("self.error = None"));
    }

    #[test]
    fn action_modals_use_compact_borderless_dialog_shell() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        assert!(implementation_source.contains("const ACTION_DIALOG_TITLE_HEIGHT: f32 = 34.0"));
        assert!(implementation_source.contains("const ACTION_DIALOG_TITLE_SIZE: f32 = 16.0"));
        assert!(implementation_source.contains("fn compact_action_dialog("));
        assert!(implementation_source.contains(".title_bar(false)"));
        assert!(implementation_source.contains(".shadow(panel_shadow())"));
        assert!(implementation_source.contains("compact_dialog_title_bar("));

        let error_start = implementation_source.find("fn error_modal(").unwrap();
        let error_end = implementation_source[error_start..]
            .find("fn main_layout(")
            .unwrap();
        let error_source = &implementation_source[error_start..error_start + error_end];
        assert!(error_source.contains("compact_action_dialog("));

        for (start, end) in [
            ("fn commit_action_modal(", "fn worktree_action_modal("),
            ("fn worktree_action_modal(", "fn stash_action_modal("),
            ("fn stash_action_modal(", "fn branch_action_modal("),
            ("fn branch_action_modal(", "fn tag_action_modal("),
            ("fn tag_action_modal(", "fn settings_modal("),
        ] {
            let modal_start = implementation_source.find(start).unwrap();
            let modal_end = implementation_source[modal_start..].find(end).unwrap();
            let modal_source = &implementation_source[modal_start..modal_start + modal_end];
            assert!(modal_source.contains("compact_action_dialog("));
            assert!(!modal_source.contains(".anchor(Align2::CENTER_CENTER"));
        }
    }

    #[test]
    fn action_modals_submit_default_action_on_enter() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        assert!(implementation_source.contains("fn dialog_default_submit_requested("));
        assert!(
            implementation_source
                .contains("input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)")
        );

        for (start, end) in [
            ("fn fetch_action_modal(", "fn pull_current("),
            ("fn pull_action_modal(", "fn push_action_modal("),
            ("fn push_action_modal(", "fn commit_action_modal("),
            ("fn commit_action_modal(", "fn worktree_action_modal("),
            (
                "WorktreeActionDialog::ConfirmDiscard { path, untracked } =>",
                "WorktreeActionDialog::ResolveConflicts",
            ),
            ("fn stash_action_modal(", "fn branch_action_modal("),
            ("fn branch_action_modal(", "fn tag_action_modal("),
            ("fn tag_action_modal(", "fn settings_modal("),
            ("fn repo_remote_action_modal(", "fn remote_settings_table("),
        ] {
            let modal_start = implementation_source
                .find(start)
                .unwrap_or_else(|| panic!("{start}"));
            let modal_end = implementation_source[modal_start..]
                .find(end)
                .unwrap_or_else(|| panic!("{end}"));
            let modal_source = &implementation_source[modal_start..modal_start + modal_end];
            assert!(
                modal_source.contains("dialog_default_submit_requested(ui)"),
                "{start} should submit its default action on Enter"
            );
        }

        let conflict_start = implementation_source
            .find("WorktreeActionDialog::ResolveConflicts { selected_path } =>")
            .unwrap();
        let conflict_end = implementation_source[conflict_start..]
            .find("fn stash_action_modal(")
            .unwrap();
        let conflict_source = &implementation_source[conflict_start..conflict_start + conflict_end];
        assert!(!conflict_source.contains("dialog_default_submit_requested(ui)"));
    }

    #[test]
    fn unified_diff_uses_single_layer_code_review_colors() {
        let source = include_str!("app.rs");
        let diff_panel_start = source.find("fn diff_panel_frame(").unwrap();
        let diff_frame_end = source[diff_panel_start..]
            .find("fn diff_display_mode_salt(")
            .unwrap();
        let diff_frame_source = &source[diff_panel_start..diff_panel_start + diff_frame_end];
        assert!(diff_frame_source.contains("theme::panel_recessed()"));
        assert!(!diff_frame_source.contains("theme::accent_soft()"));
        assert!(
            diff_frame_source.contains(".corner_radius(CornerRadius::same(WORKSPACE_CARD_RADIUS))")
        );
        assert!(!diff_frame_source.contains(".shadow(panel_shadow())"));
        assert!(!diff_frame_source.contains(".stroke("));
        assert!(source.contains("paint_workspace_card_inset_shadow(ui, diff_rect)"));

        let diff_row_start = source.find("fn diff_row(").unwrap();
        let hunk_start = source[diff_row_start..].find("fn diff_hunk_row(").unwrap();
        let diff_row_source = &source[diff_row_start..diff_row_start + hunk_start];
        assert!(diff_row_source.contains("Color32::from_rgb(214, 250, 221)"));
        assert!(diff_row_source.contains("Color32::from_rgb(255, 226, 226)"));
        assert!(!diff_row_source.contains("from_rgba_unmultiplied(55, 135, 75"));
    }

    #[test]
    fn unified_diff_rows_have_no_extra_vertical_spacing() {
        let source = include_str!("app.rs");
        let start = source.find("fn render_unified_diff(").unwrap();
        let end = source[start..]
            .find("#[derive(Clone, Debug, Eq, PartialEq)]")
            .unwrap();
        let render_source = &source[start..start + end];
        assert!(render_source.contains("ui.spacing_mut().item_spacing.y = 0.0"));
        assert!(!render_source.contains("ui.add_space(3.0)"));
    }

    #[test]
    fn unified_diff_draws_indent_guide_dots_inside_rows() {
        let source = include_str!("app.rs");
        let diff_row_start = source.find("fn diff_row(").unwrap();
        let hunk_start = source[diff_row_start..].find("fn diff_hunk_row(").unwrap();
        let diff_row_source = &source[diff_row_start..diff_row_start + hunk_start];
        assert!(diff_row_source.contains("draw_diff_indent_guides(ui, text_rect, &line.body)"));
        assert!(diff_row_source.contains("if !selected"));

        let guide_start = source.find("fn draw_diff_indent_guides(").unwrap();
        let guide_end = source[guide_start..].find("fn diff_hunk_row(").unwrap();
        let guide_source = &source[guide_start..guide_start + guide_end];
        assert!(guide_source.contains("circle_filled"));
        assert!(guide_source.contains("layout_no_wrap"));
        assert!(guide_source.contains("_ => break"));
    }

    #[test]
    fn diff_selection_supports_copyable_selected_rows() {
        let line = DiffLine {
            left_no: String::new(),
            right_no: "12".to_owned(),
            body: "    let value = 1;".to_owned(),
            kind: DiffKind::Added,
        };
        let selected_rows = vec![line.key()];
        assert_eq!(
            selected_diff_rows_text(&selected_rows),
            "    let value = 1;"
        );

        let source = include_str!("app.rs");
        let start = source.find("fn diff_row(").unwrap();
        let end = source[start..].find("fn selected_diff_rows_text(").unwrap();
        let diff_row_source = &source[start..start + end];
        assert!(diff_row_source.contains("PointerButton::Primary"));
        assert!(diff_row_source.contains("input.modifiers.ctrl"));
        assert!(diff_row_source.contains("PointerButton::Secondary"));
        assert!(diff_row_source.contains("Color32::from_rgb(82, 168, 236)"));
        assert!(diff_row_source.contains("copy_text(selected_diff_rows_text(selected_rows))"));
    }

    #[test]
    fn diff_blocks_collapse_unchanged_context_and_keep_nearby_changes_together() {
        let diff = "\
diff --git a/file.txt b/file.txt
@@ -1,18 +1,18 @@
 line 1
 line 2
 line 3
-old 4
+new 4
 line 5
 line 6
 line 7
 line 8
-old 9
+new 9
 line 10
 line 11
 line 12
 line 13
 line 14
 line 15
 line 16
 line 17
 line 18";
        let full = collect_unified_diff_items(diff, DiffDisplayMode::Full);
        let blocks = collect_unified_diff_items(diff, DiffDisplayMode::Blocks);

        assert!(blocks.len() < full.len());
        assert!(
            blocks
                .iter()
                .any(|item| matches!(item, DiffRenderItem::Omitted))
        );
        let visible_lines = blocks
            .iter()
            .filter(|item| matches!(item, DiffRenderItem::Line(_)))
            .count();
        assert!(visible_lines >= 10);
    }

    #[test]
    fn history_file_selection_defaults_to_diff_blocks() {
        let source = include_str!("app.rs");
        assert!(source.contains("history_diff_display_mode: DiffDisplayMode::Blocks"));
        let start = source.find("fn select_changed_file_for_diff(").unwrap();
        let end = source[start..]
            .find("fn request_selected_file_diff(")
            .unwrap();
        let helper_source = &source[start..start + end];
        assert!(helper_source.contains("self.history_diff_display_mode = DiffDisplayMode::Blocks"));
    }

    #[test]
    fn history_commit_column_can_reexpand_after_being_shrunk() {
        let mut prefs = LayoutPrefs::default();
        let table_width = 960.0;
        let graph_width = 48.0;
        let remaining = table_width - graph_width - 8.0;

        for _ in 0..8 {
            let cols = history_column_widths(table_width, graph_width, &prefs);
            adjust_history_column_widths(&mut prefs, &cols, remaining, 2, 120.0);
        }
        let shrunk = history_column_widths(table_width, graph_width, &prefs);
        assert!(shrunk.hash <= 22.0);

        let cols = history_column_widths(table_width, graph_width, &prefs);
        adjust_history_column_widths(&mut prefs, &cols, remaining, 2, -180.0);
        let expanded = history_column_widths(table_width, graph_width, &prefs);

        assert!(expanded.hash > shrunk.hash + 120.0);
        assert!(expanded.author >= history_column_min_widths()[2]);
    }

    #[test]
    fn history_description_resize_does_not_snap_back() {
        let mut prefs = LayoutPrefs::default();
        let table_width = 1120.0;
        let graph_width = 48.0;
        let remaining = table_width - graph_width - 8.0;
        let before = history_column_widths(table_width, graph_width, &prefs);

        adjust_history_column_widths(&mut prefs, &before, remaining, 0, 56.0);
        let after = history_column_widths(table_width, graph_width, &prefs);

        assert!(after.desc > before.desc + 48.0);
        assert!(after.date < before.date - 48.0);
    }

    #[test]
    fn toolbar_icons_use_raw_actions() {
        assert_eq!(toolbar_icon("commit", ""), UiIcon::Commit);
        assert_eq!(toolbar_icon("pull", ""), UiIcon::Pull);
        assert_eq!(toolbar_icon("branch", ""), UiIcon::Branch);
        assert_eq!(toolbar_icon("tag", ""), UiIcon::Tag);
        assert_eq!(toolbar_icon("stash", ""), UiIcon::Stash);
        assert_eq!(toolbar_icon("git-flow", ""), UiIcon::Branch);
        assert_eq!(toolbar_icon("remote", ""), UiIcon::Globe);
        assert_eq!(toolbar_icon("terminal", ""), UiIcon::Terminal);
        assert_eq!(
            toolbar_icon("resource-manager", ""),
            UiIcon::ResourceManager
        );
        assert_eq!(toolbar_icon("clone", ""), UiIcon::Fetch);
        assert_eq!(toolbar_icon("add", ""), UiIcon::Folder);
        assert_eq!(toolbar_icon("create", ""), UiIcon::Plus);
        assert_eq!(toolbar_icon("+", ""), UiIcon::Plus);
    }

    #[test]
    fn toolbar_branch_button_opens_create_branch_dialog() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let panel_start = implementation_source.find("fn top_bar_panel(").unwrap();
        let panel_end = implementation_source[panel_start..]
            .find("if let Some(index) = close_repo_tab")
            .unwrap();
        let panel_source = &implementation_source[panel_start..panel_start + panel_end];
        let branch_label_start = panel_source.find("self.tr(\"branch.title\")").unwrap();
        let toolbar_start = panel_source[..branch_label_start]
            .rfind("if toolbar_button(")
            .unwrap();
        let toolbar_end = panel_source[toolbar_start..]
            .find("if toolbar_button(ui, \"tag\", self.tr(\"tag.title\"), has_repo)")
            .unwrap();
        let toolbar_source = &panel_source[toolbar_start..toolbar_start + toolbar_end];

        assert!(toolbar_source.contains("self.tr(\"branch.title\")"));
        assert!(toolbar_source.contains("has_repo && branch_actions_enabled"));
        assert!(toolbar_source.contains("self.handle_branch_action(BranchMenuAction::Create)"));
        assert!(!toolbar_source.contains("self.tr(\"branch.local\")"));
        assert!(!toolbar_source.contains("self.active_view = MainView::Branches"));
    }

    #[test]
    fn tags_view_uses_compact_action_table_with_delete_entry() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let header_start = implementation_source.find("fn tag_table_header(").unwrap();
        let header_end = implementation_source[header_start..]
            .find("fn stash_table_header(")
            .unwrap();
        let header_source = &implementation_source[header_start..header_start + header_end];
        let row_start = implementation_source.find("fn tag_table_row(").unwrap();
        let row_end = implementation_source[row_start..]
            .find("fn tag_context_menu(")
            .unwrap();
        let row_source = &implementation_source[row_start..row_start + row_end];

        assert!(implementation_source.contains("fn tag_table_columns("));
        assert!(header_source.contains("let columns = tag_table_columns(ui.available_width())"));
        assert!(row_source.contains("let columns = tag_table_columns(ui.available_width())"));
        assert!(header_source.contains("resource_label(language, \"action\")"));
        assert!(row_source.contains("tag.delete"));
        assert!(row_source.contains("TagMenuAction::Delete"));
        assert!(row_source.contains("tag_table_cell("));
        assert!(implementation_source.contains("Layout::left_to_right(Align::Center)"));
        assert!(row_source.contains("ui.with_layout(Layout::right_to_left(Align::Center)"));
        assert!(!row_source.contains("ui.add_sized(\r\n                [columns.subject"));
        assert!(!row_source.contains("width - name_w - target_w"));
    }

    #[test]
    fn tags_can_push_to_selected_remote_from_create_dialog_or_push_action() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let action_start = implementation_source.find("enum TagMenuAction").unwrap();
        let action_end = implementation_source[action_start..]
            .find("fn panel_heading")
            .unwrap();
        let action_source = &implementation_source[action_start..action_start + action_end];
        let dialog_start = implementation_source.find("enum TagActionDialog").unwrap();
        let dialog_end = implementation_source[dialog_start..]
            .find("#[derive(Clone, Debug, Eq, PartialEq)]")
            .unwrap();
        let dialog_source = &implementation_source[dialog_start..dialog_start + dialog_end];
        let modal_start = implementation_source.find("fn tag_action_modal").unwrap();
        let modal_end = implementation_source[modal_start..]
            .find("fn settings_modal")
            .unwrap();
        let modal_source = &implementation_source[modal_start..modal_start + modal_end];
        let selector_start = implementation_source
            .find("fn tag_remote_selector")
            .unwrap();
        let selector_end = implementation_source[selector_start..]
            .find("fn panel_heading")
            .unwrap();
        let selector_source = &implementation_source[selector_start..selector_start + selector_end];
        let row_start = implementation_source.find("fn tag_table_row(").unwrap();
        let row_end = implementation_source[row_start..]
            .find("fn tag_context_menu(")
            .unwrap();
        let row_source = &implementation_source[row_start..row_start + row_end];
        let context_start = implementation_source.find("fn tag_context_menu(").unwrap();
        let context_end = implementation_source[context_start..]
            .find("fn stash_table_row(")
            .unwrap();
        let context_source = &implementation_source[context_start..context_start + context_end];

        assert!(action_source.contains("Push { name: String }"));
        assert!(dialog_source.contains("Push {"));
        assert!(dialog_source.contains("name: String"));
        assert!(dialog_source.contains("remote: String"));
        assert!(dialog_source.contains("push_after_create: bool"));
        assert!(dialog_source.contains("remote: String"));
        assert!(modal_source.contains("git::push_tag(root, &remote, &name)"));
        assert!(modal_source.contains("git::create_tag_at_head(root, &name)"));
        assert!(modal_source.contains("tag.push_after_create"));
        assert!(modal_source.contains("tag_remote_selector("));
        assert!(selector_source.contains("tag.remote"));
        assert!(row_source.contains("tag.push"));
        assert!(row_source.contains("TagMenuAction::Push"));
        assert!(context_source.contains("tag.push"));
        assert!(context_source.contains("TagMenuAction::Push"));
    }

    #[test]
    fn clickable_custom_controls_use_pointing_hand_cursor() {
        let source = include_str!("app.rs");
        assert!(source.contains("fn pointing_hand_cursor(response: egui::Response)"));

        for (start_marker, end_marker) in [
            ("fn window_control_button(", "fn menu_button("),
            ("fn show(self, ui: &mut Ui)", "fn inline_button_width("),
            (
                "fn history_commit_table_row(",
                "fn draw_history_graph_cell(",
            ),
            (
                "fn history_toolbar_dropdown_button(",
                "fn history_toolbar_popup_option(",
            ),
            (
                "fn history_toolbar_popup_option(",
                "fn history_toolbar_checkbox_at(",
            ),
            ("fn history_toolbar_checkbox_at(", "fn history_graph_width("),
            (
                "fn history_file_table_row(",
                "fn history_file_column_widths(",
            ),
            ("fn resource_row_response(", "fn search_dimension_label("),
            ("fn tree_header_inner(", "fn tree_empty("),
            ("fn branch_row(", "#[derive(Clone, Debug, Default)]"),
            ("fn remote_branch_group_row(", "fn remote_branch_row("),
            ("fn remote_branch_row(", "fn remote_empty_label("),
            ("fn stash_row(", "fn tag_row("),
            ("fn tag_row(", "enum WorktreeTreeRow"),
        ] {
            let start = source
                .find(start_marker)
                .unwrap_or_else(|| panic!("{start_marker}"));
            let end = source[start..]
                .find(end_marker)
                .unwrap_or_else(|| panic!("{end_marker}"));
            let block = &source[start..start + end];
            assert!(
                block.contains("pointing_hand_cursor(")
                    || block.contains("full_row_click_response(")
                    || block.contains("full_row_click_response_enabled("),
                "{start_marker} should set pointing hand cursor"
            );
        }
    }

    #[test]
    fn remote_git_toolbar_actions_use_busy_state_without_success_noise() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        assert!(
            implementation_source
                .contains("remote_git_task: Option<Receiver<RemoteGitTaskResult>>")
        );
        assert!(implementation_source.contains("fn remote_git_busy(&self) -> bool"));
        assert!(implementation_source.contains("fn start_remote_git_action("));
        assert!(
            implementation_source
                .contains("let repo_action_busy = self.loading_repo || self.remote_git_busy()")
        );
        assert!(implementation_source.contains("!repo_action_busy && has_repo && has_remote"));
        assert!(implementation_source.contains("if self.remote_git_busy()"));

        let remote_action_start = implementation_source
            .find("fn start_remote_git_action(")
            .unwrap();
        let remote_action_end = implementation_source[remote_action_start..]
            .find("fn fetch_all(")
            .unwrap();
        let remote_action_source =
            &implementation_source[remote_action_start..remote_action_start + remote_action_end];
        assert!(remote_action_source.contains("self.loading_repo = true;"));

        let remote_task_start = implementation_source
            .find("if let Some(receiver) = self.remote_git_task.take()")
            .unwrap();
        let remote_task_end = implementation_source[remote_task_start..]
            .find("if let Some(receiver) = self.repo_source_task.take()")
            .unwrap();
        let remote_task_source =
            &implementation_source[remote_task_start..remote_task_start + remote_task_end];
        assert!(remote_task_source.contains("self.load_repository_uncached(root)"));
        assert!(remote_task_source.contains("self.loading_repo = false"));
        assert!(
            !remote_task_source.contains("self.show_toast(self.tr(\"status.action_completed\"))")
        );
        assert!(
            !remote_task_source.contains(
                "self.last_notice = Some(self.tr(\"status.action_completed\").to_owned())"
            )
        );
    }

    #[test]
    fn pull_action_uses_dialog_with_remote_branch_and_options() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        assert!(implementation_source.contains("pending_pull_action: Option<PullActionDialog>"));
        assert!(implementation_source.contains("struct PullActionDialog"));
        assert!(implementation_source.contains("fn open_pull_dialog("));
        assert!(implementation_source.contains("fn pull_action_modal("));
        assert!(implementation_source.contains("git::pull_from_remote("));
        assert!(implementation_source.contains("git::fetch_remote("));

        let pull_current_start = implementation_source.find("fn pull_current(").unwrap();
        let pull_current_end = implementation_source[pull_current_start..]
            .find("fn quick_pull_current(")
            .unwrap();
        let pull_current_source =
            &implementation_source[pull_current_start..pull_current_start + pull_current_end];
        assert!(pull_current_source.contains("self.open_pull_dialog(None);"));

        let pull_dialog_start = implementation_source.find("fn open_pull_dialog(").unwrap();
        let pull_dialog_end = implementation_source[pull_dialog_start..]
            .find("fn remote_names(")
            .unwrap();
        let pull_dialog_source =
            &implementation_source[pull_dialog_start..pull_dialog_start + pull_dialog_end];
        assert!(!pull_dialog_source.contains("git::pull(root)"));
        assert!(
            pull_dialog_source
                .contains(".find(|branch| !branch.remote && branch.name == local_branch)")
        );
        assert!(pull_dialog_source.contains("split_remote_branch_name(&upstream.name)"));

        let toolbar_start = implementation_source.find("fn top_bar_panel(").unwrap();
        let toolbar_end = implementation_source[toolbar_start..]
            .find("fn refresh_known_repositories(")
            .unwrap();
        let toolbar_source = &implementation_source[toolbar_start..toolbar_start + toolbar_end];
        assert!(toolbar_source.contains("RepoToolbarAction::Pull"));

        let branch_handler_start = implementation_source
            .find("fn handle_branch_action(&mut self, action: BranchMenuAction)")
            .unwrap();
        let branch_handler_end = implementation_source[branch_handler_start..]
            .find("fn handle_tag_action(")
            .unwrap();
        let branch_handler_source =
            &implementation_source[branch_handler_start..branch_handler_start + branch_handler_end];
        assert!(branch_handler_source.contains("self.open_pull_dialog(Some(name));"));

        let modal_start = implementation_source.find("fn pull_action_modal(").unwrap();
        let modal_end = implementation_source[modal_start..]
            .find("fn commit_action_modal(")
            .unwrap();
        let modal_source = &implementation_source[modal_start..modal_start + modal_end];
        for needle in [
            "pull.remote",
            "pull.remote_branch",
            "pull.local_branch",
            "pull.commit_merge",
            "pull.include_tags",
            "pull.force_merge_commit",
            "pull.rebase",
            "pull.refresh",
        ] {
            assert!(modal_source.contains(needle));
        }
    }

    #[test]
    fn fetch_action_uses_dialog_with_options_icon_and_loading_gate() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        assert!(implementation_source.contains("pending_fetch_action: Option<FetchActionDialog>"));
        assert!(implementation_source.contains("struct FetchActionDialog"));
        assert!(implementation_source.contains("fn open_fetch_dialog("));
        assert!(implementation_source.contains("fn fetch_action_modal("));
        assert!(implementation_source.contains("git::fetch_with_options("));

        let fetch_start = implementation_source.find("fn fetch_all(").unwrap();
        let fetch_end = implementation_source[fetch_start..]
            .find("fn pull_current(")
            .unwrap();
        let fetch_source = &implementation_source[fetch_start..fetch_start + fetch_end];
        assert!(fetch_source.contains("self.open_fetch_dialog();"));
        assert!(!fetch_source.contains("git::fetch(root)"));

        let modal_start = implementation_source
            .find("fn fetch_action_modal(")
            .unwrap();
        let modal_end = implementation_source[modal_start..]
            .find("fn pull_action_modal(")
            .unwrap();
        let modal_source = &implementation_source[modal_start..modal_start + modal_end];
        for needle in [
            "fetch.title",
            "fetch.all_remotes",
            "fetch.prune_tracking",
            "fetch.tags",
            "fetch.force_tags",
            "dialog.ok",
            "dialog.cancel",
            "FetchOptions",
        ] {
            assert!(modal_source.contains(needle), "{needle}");
        }
        assert!(modal_source.contains("add_enabled_ui(dialog.fetch_tags"));

        let toolbar_start = implementation_source.find("fn top_bar_panel(").unwrap();
        let toolbar_end = implementation_source[toolbar_start..]
            .find("fn refresh_known_repositories(")
            .unwrap();
        let toolbar_source = &implementation_source[toolbar_start..toolbar_start + toolbar_end];
        assert!(toolbar_source.contains("RepoToolbarAction::Fetch"));

        let update_start = implementation_source
            .find("impl App for GitAgentApp")
            .unwrap();
        let update_end = implementation_source[update_start..]
            .find("impl GitAgentApp")
            .unwrap();
        let update_source = &implementation_source[update_start..update_start + update_end];
        assert!(update_source.contains("self.fetch_action_modal(ctx);"));

        let icon_source = include_str!("../assets/icons/fetch.svg");
        assert!(icon_source.contains("stroke-dasharray"));
        assert!(icon_source.contains("M12"));
        assert!(!icon_source.contains("M3 12a9 9"));
    }

    #[test]
    fn repo_toolbar_actions_support_double_click_and_shift_shortcut_quick_paths() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        assert!(implementation_source.contains("const TOOLBAR_DOUBLE_CLICK_DELAY"));
        assert!(
            implementation_source
                .contains("pending_toolbar_single_click: Option<PendingToolbarClick>")
        );
        assert!(implementation_source.contains("struct PendingToolbarClick"));
        assert!(implementation_source.contains("enum RepoToolbarAction"));
        assert!(implementation_source.contains("fn handle_repo_toolbar_action_response("));
        assert!(implementation_source.contains("fn flush_pending_toolbar_single_click("));
        assert!(implementation_source.contains("fn run_quick_toolbar_action("));
        assert!(implementation_source.contains("fn open_toolbar_action_dialog("));
        assert!(implementation_source.contains("self.flush_pending_toolbar_single_click(ctx);"));

        let handler_start = implementation_source
            .find("fn handle_repo_toolbar_action_response(")
            .unwrap();
        let handler_end = implementation_source[handler_start..]
            .find("fn flush_pending_toolbar_single_click(")
            .unwrap();
        let handler_source = &implementation_source[handler_start..handler_start + handler_end];
        assert!(handler_source.contains("response.double_clicked()"));
        assert!(handler_source.contains("self.run_quick_toolbar_action(action);"));
        assert!(handler_source.contains("response.clicked()"));
        assert!(
            handler_source.contains("self.pending_toolbar_single_click = Some(PendingToolbarClick")
        );

        let flush_start = implementation_source
            .find("fn flush_pending_toolbar_single_click(")
            .unwrap();
        let flush_end = implementation_source[flush_start..]
            .find("fn open_toolbar_action_dialog(")
            .unwrap();
        let flush_source = &implementation_source[flush_start..flush_start + flush_end];
        assert!(flush_source.contains("self.open_toolbar_action_dialog(pending.action);"));

        let dialog_start = implementation_source
            .find("fn open_toolbar_action_dialog(")
            .unwrap();
        let dialog_end = implementation_source[dialog_start..]
            .find("fn run_quick_toolbar_action(")
            .unwrap();
        let dialog_source = &implementation_source[dialog_start..dialog_start + dialog_end];
        for needle in [
            "self.pull_current()",
            "self.push_current()",
            "self.fetch_all()",
        ] {
            assert!(dialog_source.contains(needle), "{needle}");
        }

        let quick_start = implementation_source
            .find("fn run_quick_toolbar_action(")
            .unwrap();
        let quick_end = implementation_source[quick_start..]
            .find("fn open_push_dialog(")
            .unwrap();
        let quick_source = &implementation_source[quick_start..quick_start + quick_end];
        for needle in [
            "self.quick_pull_current()",
            "self.quick_push_current()",
            "self.quick_fetch_all()",
            "git::pull(root)",
            "git::push(root)",
            "git::fetch_with_options(root, git::FetchOptions::default())",
        ] {
            assert!(quick_source.contains(needle), "{needle}");
        }

        let shortcuts_start = implementation_source
            .find("fn handle_global_shortcuts(")
            .unwrap();
        let shortcuts_end = implementation_source[shortcuts_start..]
            .find("fn poll_tasks(")
            .unwrap();
        let shortcuts_source =
            &implementation_source[shortcuts_start..shortcuts_start + shortcuts_end];
        for needle in [
            "shortcut_pressed(ctx, egui::Key::P, true)",
            "self.quick_push_current();",
            "shortcut_pressed(ctx, egui::Key::P, false)",
            "self.push_current();",
            "shortcut_pressed(ctx, egui::Key::L, true)",
            "self.quick_pull_current();",
            "shortcut_pressed(ctx, egui::Key::L, false)",
            "self.pull_current();",
            "shortcut_pressed(ctx, egui::Key::F, true)",
            "self.quick_fetch_all();",
            "shortcut_pressed(ctx, egui::Key::F, false)",
            "self.fetch_all();",
        ] {
            assert!(shortcuts_source.contains(needle), "{needle}");
        }

        let toolbar_start = implementation_source.find("fn top_bar_panel(").unwrap();
        let toolbar_end = implementation_source[toolbar_start..]
            .find("fn refresh_known_repositories(")
            .unwrap();
        let toolbar_source = &implementation_source[toolbar_start..toolbar_start + toolbar_end];
        for needle in [
            "let pull_response = toolbar_button(",
            "RepoToolbarAction::Pull",
            "let push_response = toolbar_button(",
            "RepoToolbarAction::Push",
            "let fetch_response = toolbar_button(",
            "RepoToolbarAction::Fetch",
        ] {
            assert!(toolbar_source.contains(needle), "{needle}");
        }
    }

    #[test]
    fn push_action_uses_dialog_with_branch_rows_options_and_loading_gate() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        assert!(implementation_source.contains("pending_push_action: Option<PushActionDialog>"));
        assert!(implementation_source.contains("struct PushActionDialog"));
        assert!(implementation_source.contains("struct PushBranchRow"));
        assert!(implementation_source.contains("fn open_push_dialog("));
        assert!(implementation_source.contains("fn push_action_modal("));
        assert!(implementation_source.contains("git::push_selected("));

        let push_start = implementation_source.find("fn push_current(").unwrap();
        let push_end = implementation_source[push_start..]
            .find("fn quick_push_current(")
            .unwrap();
        let push_source = &implementation_source[push_start..push_start + push_end];
        assert!(push_source.contains("self.open_push_dialog(None, None);"));
        assert!(!push_source.contains("git::push(root)"));
        assert!(!push_source.contains("git::push_set_upstream(root"));

        let branch_handler_start = implementation_source
            .find("fn handle_branch_action(&mut self, action: BranchMenuAction)")
            .unwrap();
        let branch_handler_end = implementation_source[branch_handler_start..]
            .find("fn handle_tag_action(")
            .unwrap();
        let branch_handler_source =
            &implementation_source[branch_handler_start..branch_handler_start + branch_handler_end];
        assert!(branch_handler_source.contains("self.open_push_dialog(Some(name), None);"));
        assert!(branch_handler_source.contains("self.open_push_dialog(Some(name), Some(remote));"));
        assert!(!branch_handler_source.contains("git::push_branch_to_remote(root"));

        let modal_start = implementation_source.find("fn push_action_modal(").unwrap();
        let modal_end = implementation_source[modal_start..]
            .find("fn commit_action_modal(")
            .unwrap();
        let modal_source = &implementation_source[modal_start..modal_start + modal_end];
        for needle in [
            "push.remote",
            "push.local_branch",
            "push.remote_branch",
            "push.track",
            "push.select",
            "push.select_all",
            "push.push_tags",
            "push.force",
            "selected_push_branches",
        ] {
            assert!(modal_source.contains(needle), "{needle}");
        }
        assert!(modal_source.contains("push_remote_form_row("));
        assert!(modal_source.contains("push_branch_table_header("));
        assert!(modal_source.contains("push_branch_table_row("));
        assert!(!modal_source.contains("egui::Grid::new(\"push_branch_grid\")"));
        assert!(implementation_source.contains(".desired_width(url_width)"));
        assert!(implementation_source.contains("PUSH_SELECT_COLUMN_WIDTH"));
        assert!(implementation_source.contains("PUSH_LOCAL_BRANCH_COLUMN_WIDTH"));
        assert!(implementation_source.contains("PUSH_TRACK_COLUMN_WIDTH"));
        assert!(implementation_source.contains("const PUSH_TABLE_ROW_HEIGHT: f32 = 28.0;"));
        assert!(implementation_source.contains("PUSH_TABLE_BODY_TEXT_Y_OFFSET"));
        assert!(implementation_source.contains("fn push_branch_cell_rects("));
        assert!(implementation_source.contains("fn paint_push_branch_text_cell("));
        assert!(implementation_source.contains("fn push_remote_form_row("));
        assert!(implementation_source.contains("fn paint_push_form_label("));
        assert!(implementation_source.contains("paint_push_branch_body_text_cell("));
        assert!(implementation_source.contains(
            "selector_ui.spacing_mut().interact_size.y = PUSH_REMOTE_FORM_CONTROL_HEIGHT"
        ));
        assert!(implementation_source.contains("url_ui.add_sized("));
        assert!(implementation_source.contains("[url_width, PUSH_REMOTE_FORM_CONTROL_HEIGHT]"));
        assert!(implementation_source.contains("Rect::from_center_size"));
        assert!(implementation_source.contains("Align2::LEFT_CENTER"));

        let i18n_source = include_str!("i18n.rs");
        assert!(
            i18n_source.contains("(\"push.select\", \"\\u{662f}\\u{5426}\\u{63a8}\\u{9001}\")")
        );
    }

    #[test]
    fn repo_toolbar_loading_replaces_entire_action_row() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        assert!(implementation_source.contains("fn repo_toolbar_loading_busy(&self) -> bool"));
        assert!(implementation_source.contains("fn repo_toolbar_loading_indicator("));
        assert!(implementation_source.contains("UiIcon::Loading"));
        assert!(implementation_source.contains("../assets/icons/loading.svg"));

        let toolbar_start = implementation_source.find("fn top_bar_panel(").unwrap();
        let toolbar_end = implementation_source[toolbar_start..]
            .find("if let Some(index) = close_repo_tab")
            .unwrap();
        let toolbar_source = &implementation_source[toolbar_start..toolbar_start + toolbar_end];
        assert!(
            toolbar_source.contains("let repo_toolbar_loading = self.repo_toolbar_loading_busy();")
        );
        assert!(toolbar_source.contains("if repo_toolbar_loading {"));
        assert!(toolbar_source.contains("repo_toolbar_loading_indicator(ui);"));
        assert!(toolbar_source.contains("} else {"));

        let loading_branch_start = toolbar_source.find("if repo_toolbar_loading {").unwrap();
        let loading_branch_end = toolbar_source[loading_branch_start..]
            .find("} else {")
            .unwrap();
        let loading_branch =
            &toolbar_source[loading_branch_start..loading_branch_start + loading_branch_end];
        assert!(!loading_branch.contains("toolbar_button("));
        assert!(!loading_branch.contains("status.loading_repo"));
    }

    #[test]
    fn local_branch_checkout_uses_pending_async_state_and_uncached_reload() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        assert!(
            implementation_source
                .contains("type BranchCheckoutTaskResult = (PathBuf, String, anyhow::Result<()>)")
        );
        assert!(
            implementation_source
                .contains("branch_checkout_task: Option<Receiver<BranchCheckoutTaskResult>>")
        );
        assert!(implementation_source.contains("pending_branch_checkout: Option<String>"));
        assert!(implementation_source.contains("fn branch_checkout_busy(&self) -> bool"));
        assert!(
            implementation_source.contains("fn request_branch_checkout(&mut self, name: String)")
        );
        assert!(
            implementation_source.contains(
                "fn start_branch_checkout(&mut self, name: String, discard_changes: bool)"
            )
        );
        assert!(implementation_source.contains("self.request_branch_checkout(name);"));
        assert!(
            implementation_source.contains("fn load_repository_uncached(&mut self, path: PathBuf)")
        );

        let branch_task_start = implementation_source
            .find("if let Some(receiver) = self.branch_checkout_task.take()")
            .unwrap();
        let branch_task_end = implementation_source[branch_task_start..]
            .find("if let Some(receiver) = self.remote_git_task.take()")
            .unwrap();
        let branch_task_source =
            &implementation_source[branch_task_start..branch_task_start + branch_task_end];
        assert!(branch_task_source.contains("self.pending_branch_checkout = None"));
        assert!(branch_task_source.contains("self.load_repository_uncached(root)"));
    }

    #[test]
    fn dirty_local_branch_checkout_prompts_before_checkout_and_can_discard() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        assert!(implementation_source.contains(
            "ConfirmCheckout {\n        name: String,\n        discard_changes: bool,\n    }"
        ));
        assert!(
            implementation_source.contains("fn request_branch_checkout(&mut self, name: String)")
        );
        assert!(
            implementation_source.contains(
                "fn start_branch_checkout(&mut self, name: String, discard_changes: bool)"
            )
        );

        let request_start = implementation_source
            .find("fn request_branch_checkout(&mut self, name: String)")
            .unwrap();
        let request_end = implementation_source[request_start..]
            .find("fn start_branch_checkout(")
            .unwrap();
        let request_source = &implementation_source[request_start..request_start + request_end];
        assert!(request_source.contains("!snapshot.status.is_empty()"));
        assert!(
            request_source
                .contains("self.pending_branch_action = Some(BranchActionDialog::ConfirmCheckout")
        );
        assert!(request_source.contains("self.start_branch_checkout(name, false);"));

        let checkout_start = implementation_source
            .find("fn start_branch_checkout(&mut self, name: String, discard_changes: bool)")
            .unwrap();
        let checkout_end = implementation_source[checkout_start..]
            .find("fn remote_git_busy(&self)")
            .unwrap();
        let checkout_source = &implementation_source[checkout_start..checkout_start + checkout_end];
        assert!(checkout_source.contains("if discard_changes {"));
        assert!(checkout_source.contains("git::discard_all_changes(&root)?;"));
        assert!(checkout_source.contains("git::checkout_branch(&root, &name)"));

        let dialog_start = implementation_source
            .find("fn branch_action_modal(&mut self")
            .unwrap();
        let dialog_end = implementation_source[dialog_start..]
            .find("fn tag_action_modal(")
            .unwrap();
        let dialog_source = &implementation_source[dialog_start..dialog_start + dialog_end];
        assert!(dialog_source.contains("BranchActionDialog::ConfirmCheckout"));
        assert!(dialog_source.contains("branch.confirm_checkout"));
        assert!(dialog_source.contains("branch.discard_before_checkout"));
        assert!(dialog_source.contains("ui.checkbox(discard_changes"));
        assert!(dialog_source.contains("self.start_branch_checkout(name, discard_changes);"));
    }

    #[test]
    fn branch_rows_disable_branch_actions_while_checkout_or_reload_is_busy() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        assert!(implementation_source.contains("fn branch_actions_busy(&self) -> bool"));
        let busy_start = implementation_source
            .find("fn branch_actions_busy(&self) -> bool")
            .unwrap();
        let busy_end = implementation_source[busy_start..]
            .find("fn repo_toolbar_loading_busy(")
            .unwrap();
        let busy_source = &implementation_source[busy_start..busy_start + busy_end];
        assert!(busy_source.contains("self.loading_repo"));
        assert!(busy_source.contains("self.branch_checkout_busy()"));
        assert!(busy_source.contains("self.remote_git_busy()"));
        assert!(busy_source.contains("self.merge_tool_busy()"));
        assert!(
            implementation_source
                .contains("let branch_actions_enabled = !self.branch_actions_busy();")
        );

        let branch_row_start = implementation_source.find("fn branch_row(").unwrap();
        let branch_row_end = implementation_source[branch_row_start..]
            .find("fn branch_current_indicator_rect(")
            .unwrap();
        let branch_row_source =
            &implementation_source[branch_row_start..branch_row_start + branch_row_end];
        assert!(branch_row_source.contains("enabled: bool"));
        assert!(branch_row_source.contains("full_row_click_response_enabled("));
        assert!(branch_row_source.contains("if enabled && response.double_clicked()"));
        assert!(branch_row_source.contains("branch_context_menu("));

        let context_menu_start = implementation_source
            .find("fn branch_context_menu(")
            .unwrap();
        let context_menu_end = implementation_source[context_menu_start..]
            .find("fn branch_checkout_menu_label(")
            .unwrap();
        let context_menu_source =
            &implementation_source[context_menu_start..context_menu_start + context_menu_end];
        assert!(context_menu_source.contains("enabled && !current"));

        let table_row_start = implementation_source.find("fn branch_table_row(").unwrap();
        let table_row_end = implementation_source[table_row_start..]
            .find("fn branch_context_menu(")
            .unwrap();
        let table_row_source =
            &implementation_source[table_row_start..table_row_start + table_row_end];
        assert!(table_row_source.contains("enabled: bool"));
        assert!(table_row_source.contains("branch_context_menu("));

        let remote_row_start = implementation_source.find("fn remote_branch_row(").unwrap();
        let remote_row_end = implementation_source[remote_row_start..]
            .find("fn remote_empty_label(")
            .unwrap();
        let remote_row_source =
            &implementation_source[remote_row_start..remote_row_start + remote_row_end];
        assert!(remote_row_source.contains("enabled: bool"));
        assert!(remote_row_source.contains("if enabled && response.double_clicked()"));
    }

    #[test]
    fn local_branch_context_menu_exposes_sourcetree_style_actions() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        let menu_start = implementation_source
            .find("fn branch_context_menu(")
            .unwrap();
        let menu_end = implementation_source[menu_start..]
            .find("fn branch_checkout_menu_label(")
            .unwrap();
        let menu_source = &implementation_source[menu_start..menu_start + menu_end];

        for required in [
            "BranchMenuAction::Checkout",
            "BranchMenuAction::MergeIntoCurrent",
            "BranchMenuAction::RebaseCurrentOnto",
            "BranchMenuAction::FetchTracked",
            "BranchMenuAction::PullTracked",
            "BranchMenuAction::PushTracked",
            "BranchMenuAction::PushToRemote",
            "BranchMenuAction::TrackRemote",
            "BranchMenuAction::CompareWithCurrent",
            "BranchMenuAction::Rename",
            "BranchMenuAction::Delete",
            "BranchMenuAction::CreatePullRequest",
            "branch.push_to",
            "branch.track_remote",
            "branch.no_remote_tracking",
            "branch.compare_with_current",
            "branch.create_pull_request",
            "remote_branch_names",
            "remotes",
        ] {
            assert!(menu_source.contains(required), "{required}");
        }

        assert!(menu_source.contains("branch_checkout_menu_label(language, name)"));
        assert!(menu_source.contains("branch_merge_menu_label(language, name)"));
        assert!(menu_source.contains("branch_rebase_menu_label(language, name)"));
        assert!(menu_source.contains("branch_rename_menu_label(language, name)"));
        assert!(menu_source.contains("branch_delete_menu_label(language, name)"));
    }

    #[test]
    fn local_branch_context_menu_is_shared_by_sidebar_and_table_rows() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        let branch_row_start = implementation_source.find("fn branch_row(").unwrap();
        let branch_row_end = implementation_source[branch_row_start..]
            .find("fn branch_current_indicator_rect(")
            .unwrap();
        let branch_row_source =
            &implementation_source[branch_row_start..branch_row_start + branch_row_end];
        let table_row_start = implementation_source.find("fn branch_table_row(").unwrap();
        let table_row_end = implementation_source[table_row_start..]
            .find("fn branch_context_menu(")
            .unwrap();
        let table_row_source =
            &implementation_source[table_row_start..table_row_start + table_row_end];

        assert!(branch_row_source.contains("branch_context_menu("));
        assert!(branch_row_source.contains("remote_branch_names"));
        assert!(branch_row_source.contains("remotes"));
        assert!(branch_row_source.contains("upstream"));
        assert!(table_row_source.contains("branch_context_menu("));
        assert!(table_row_source.contains("remote_branch_names"));
        assert!(table_row_source.contains("remotes"));
        assert!(table_row_source.contains("upstream"));
    }

    #[test]
    fn branch_menu_actions_wire_to_async_git_operations_and_dialogs() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        let handler_start = implementation_source
            .find("fn handle_branch_action(&mut self, action: BranchMenuAction)")
            .unwrap();
        let handler_end = implementation_source[handler_start..]
            .find("fn handle_tag_action(")
            .unwrap();
        let handler_source = &implementation_source[handler_start..handler_start + handler_end];

        for required in [
            "git::merge_branch(root, &name)",
            "git::rebase_current_onto(root, &name)",
            "git::fetch_remote_branch(root, &remote_branch)",
            "git::set_branch_upstream(root, &name, &remote_branch)",
            "git::unset_branch_upstream(root, &name)",
            "self.commit_message.clear();",
            "self.start_remote_git_action(move |root|",
            "self.active_view = MainView::Workspace;",
            "self.open_pull_dialog(Some(name));",
            "self.open_push_dialog(Some(name), None);",
            "self.open_push_dialog(Some(name), Some(remote));",
            "self.open_branch_compare_url(&name);",
            "self.open_branch_pull_request_url(&name);",
            "BranchActionDialog::Rename",
        ] {
            assert!(handler_source.contains(required), "{required}");
        }

        let dialog_start = implementation_source
            .find("fn branch_action_modal(&mut self")
            .unwrap();
        let dialog_end = implementation_source[dialog_start..]
            .find("fn tag_action_modal(")
            .unwrap();
        let dialog_source = &implementation_source[dialog_start..dialog_start + dialog_end];
        assert!(dialog_source.contains("BranchActionDialog::Rename"));
        assert!(dialog_source.contains("branch.rename_title"));
        assert!(dialog_source.contains("branch.new_name"));
        assert!(dialog_source.contains("git::rename_branch(root, &old_name, &new_name)"));
    }

    #[test]
    fn merge_snapshot_message_becomes_default_commit_message() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];

        assert!(implementation_source.contains("fn apply_merge_commit_message_default("));
        let apply_start = implementation_source
            .find("fn apply_repository_snapshot(&mut self")
            .unwrap();
        let apply_end = implementation_source[apply_start..]
            .find("fn refresh(")
            .unwrap();
        let apply_source = &implementation_source[apply_start..apply_start + apply_end];
        assert!(apply_source.contains("self.apply_merge_commit_message_default(&snapshot);"));
    }

    #[test]
    fn plus_tab_opens_repository_source_page() {
        let source = include_str!("app.rs");
        assert!(source.contains("fn open_repository_source_tab("));
        assert!(source.contains("fn repository_source_view("));
        assert!(source.contains("self.open_repository_source_tab();"));
        assert!(source.contains("fn repo_tab_fill("));
        assert!(source.contains("fn repo_tab_text_color("));
        assert!(source.contains("fn repo_tab_icon_color("));
        assert!(source.contains("fn top_corners("));
        assert!(source.contains("fn repo_tab_with_close("));
        assert!(source.contains("let mut close_source_tab = false;"));
        assert!(source.contains("Pos2::new(close_rect.left() - 2.0, rect.bottom())"));
        assert!(source.contains("fn close_repo_tab("));
        assert!(source.contains("fn save_repo_tabs("));

        let plus_start = source
            .find("if icon_button(ui, UiIcon::Plus, &new_tab_label, !loading_repo).clicked()")
            .unwrap();
        let plus_end = source[plus_start..]
            .find("ui.allocate_new_ui(egui::UiBuilder::new().max_rect(global_action_row)")
            .unwrap();
        let plus_block = &source[plus_start..plus_start + plus_end];
        assert!(!plus_block.contains("pick_folder"));
        assert!(!plus_block.contains("RichText::new(\"\\u{00d7}\")"));

        let repo_tab_start = source.find("fn repo_tab_with_close(").unwrap();
        let repo_tab_end = source[repo_tab_start..]
            .find("fn source_tab_button(")
            .unwrap();
        let repo_tab_source = &source[repo_tab_start..repo_tab_start + repo_tab_end];
        assert!(repo_tab_source.contains("paint_repo_tab_shadow(ui, rect, selected);"));
        assert!(repo_tab_source.contains("repo_tab_close_hovered(ui, close_rect)"));
        assert!(repo_tab_source.contains("let tab_hovered = response.hovered() || close_hovered"));
        assert!(repo_tab_source.contains("let show_close = selected || tab_hovered"));

        let button_start = source.find("fn show(self, ui: &mut Ui)").unwrap();
        let button_end = source[button_start..]
            .find("fn inline_button_width(")
            .unwrap();
        let button_source = &source[button_start..button_start + button_end];
        assert!(button_source.contains("paint_repo_tab_shadow(ui, response.rect, selected);"));

        let shadow_start = source.find("fn paint_repo_tab_shadow(").unwrap();
        let shadow_end = source[shadow_start..]
            .find("fn repo_tab_with_close(")
            .unwrap();
        let shadow_source = &source[shadow_start..shadow_start + shadow_end];
        assert!(shadow_source.contains("paint_repo_tab_shadow_side("));
        assert!(shadow_source.contains("RepoTabShadowSide::Top"));
        assert!(shadow_source.contains("RepoTabShadowSide::Left"));
        assert!(shadow_source.contains("RepoTabShadowSide::Right"));
        assert!(shadow_source.contains("offset: [2, 3]"));
        assert!(shadow_source.contains("repo_tab_highlight_color("));
        assert!(shadow_source.contains("Color32::WHITE"));
        assert!(shadow_source.contains("highlight_spread"));
        assert!(shadow_source.contains("shadow_spread"));
        assert!(shadow_source.contains("Pos2::new(rect.right(), rect.top() + 2.0)"));
        assert!(shadow_source.contains("rect.bottom() - 0.6"));
    }

    #[test]
    fn repo_tab_shadow_geometry_reads_as_top_left_light_source() {
        let rect = Rect::from_min_size(Pos2::new(100.0, 20.0), Vec2::new(120.0, 28.0));
        let top = repo_tab_shadow_rect(rect, RepoTabShadowSide::Top, 2);
        let left = repo_tab_shadow_rect(rect, RepoTabShadowSide::Left, 2);
        let right = repo_tab_shadow_rect(rect, RepoTabShadowSide::Right, 2);

        assert!(top.left() >= rect.left() - 2.0);
        assert!(top.right() <= rect.right() + 2.0);
        assert!(top.height() <= 3.0);
        assert!(left.width() <= 3.0);
        assert!(left.bottom() <= rect.bottom() + 1.0);
        assert!(right.left() >= rect.right());
        assert!(right.top() >= rect.top() + 1.0);
        assert!(right.bottom() <= rect.bottom());
        assert!(right.right() <= rect.right() + 3.0);
    }

    #[test]
    fn remote_clone_requires_valid_checked_url_before_destination() {
        let source = include_str!("app.rs");
        assert!(source.contains("git::validate_remote_url(&url)"));
        assert!(source.contains("self.clone_destination.clear();"));
        assert!(source.contains("let url_valid = self.clone_url_is_valid();"));
        assert!(source.contains(".add_enabled(url_valid, egui::Button::new(browse_label))"));
        assert!(source.contains("!busy && url_valid && !self.clone_destination.trim().is_empty()"));

        let git_source = include_str!("git.rs");
        assert!(git_source.contains("ls-remote"));
        assert!(git_source.contains("GIT_TERMINAL_PROMPT"));
    }

    #[test]
    fn remote_button_opens_repository_url() {
        let source = include_str!("app.rs");
        let top_bar_start = source.find("fn top_bar_panel(").unwrap();
        let top_bar_end = source[top_bar_start..]
            .find("fn repository_source_view(")
            .unwrap();
        let top_bar_source = &source[top_bar_start..top_bar_start + top_bar_end];

        assert!(top_bar_source.contains("self.open_remote_url();"));
        assert!(!top_bar_source.contains("self.open_remote_panel();"));
    }

    #[test]
    fn remote_web_url_normalizes_common_git_urls() {
        assert_eq!(
            remote_web_url("git@github.com:owner/repo.git"),
            Some("https://github.com/owner/repo".to_owned())
        );
        assert_eq!(
            remote_web_url("https://github.com/owner/repo.git"),
            Some("https://github.com/owner/repo".to_owned())
        );
        assert_eq!(
            remote_web_url("ssh://git@gitlab.com/owner/repo.git"),
            Some("https://gitlab.com/owner/repo".to_owned())
        );
        assert_eq!(remote_web_url(""), None);
    }

    #[test]
    fn branch_remote_urls_target_compare_and_pull_request_pages() {
        assert_eq!(
            branch_compare_url("https://github.com/owner/repo", "main", "feature/batch"),
            "https://github.com/owner/repo/compare/main...feature/batch"
        );
        assert_eq!(
            branch_pull_request_url("https://github.com/owner/repo", "feature/batch"),
            "https://github.com/owner/repo/compare/feature/batch?expand=1"
        );
        assert_eq!(
            branch_compare_url("https://gitlab.com/owner/repo", "main", "feature/batch"),
            "https://gitlab.com/owner/repo/-/compare/main...feature/batch"
        );
        assert_eq!(
            branch_pull_request_url("https://gitlab.com/owner/repo", "feature/batch"),
            "https://gitlab.com/owner/repo/-/merge_requests/new?merge_request%5Bsource_branch%5D=feature%2Fbatch"
        );
        assert_eq!(
            commit_remote_url("https://github.com/owner/repo", "abc123"),
            "https://github.com/owner/repo/commit/abc123"
        );
        assert_eq!(
            commit_remote_url("https://gitlab.com/owner/repo", "abc123"),
            "https://gitlab.com/owner/repo/-/commit/abc123"
        );
    }

    #[test]
    fn command_button_opens_git_bash_on_windows() {
        let source = include_str!("app.rs");
        let start = source.find("fn open_command_prompt(").unwrap();
        let end = source[start..].find("fn open_file_manager(").unwrap();
        let command_source = &source[start..start + end];

        assert!(command_source.contains("git_bash_executable()"));
        assert!(command_source.contains("\"--cd={}\""));
        assert!(!command_source.contains("\"cmd\", \"/K\""));
    }

    #[test]
    fn repo_tabs_state_deduplicates_paths_and_keeps_active_source_state() {
        let state = RepoTabsState {
            tabs: vec![
                "D:/workspace/git-Agent".to_owned(),
                "D:/workspace/git-Agent".to_owned(),
            ],
            active_repo_tab: Some(0),
            source_tab_open: true,
            source_tab_active: true,
            sidebar_tree_states: HashMap::new(),
        };

        assert_eq!(state.repo_tabs().len(), 1);
        assert!(state.source_tab_open);
        assert!(state.source_tab_active);
    }

    #[test]
    fn repo_tab_overflow_keeps_active_tab_visible() {
        let widths = vec![120.0; 7];
        let visibility = repo_tab_visibility(&widths, Some(120.0), Some(5), false, 470.0);

        assert!(visibility.visible_repo_indices.contains(&5));
        assert!(!visibility.overflow_repo_indices.contains(&5));
        assert!(!visibility.overflow_repo_indices.is_empty());
        assert!(visibility.has_leading_overflow() || visibility.has_trailing_overflow());
    }

    #[test]
    fn repo_tab_overflow_button_is_compact_and_uses_more_icon() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let overflow_button_start = implementation_source
            .find("fn repo_tab_overflow_button(")
            .unwrap();
        let overflow_button_end = implementation_source[overflow_button_start..]
            .find("fn repo_tab_overflow_option(")
            .unwrap();
        let overflow_button_source = &implementation_source
            [overflow_button_start..overflow_button_start + overflow_button_end];

        assert_eq!(REPO_TAB_OVERFLOW_WIDTH, 82.0);
        assert!(implementation_source.contains("UiIcon::More"));
        assert!(implementation_source.contains("../assets/icons/more.svg"));
        assert!(overflow_button_source.contains("AppButton::repo_tab(UiIcon::More"));
    }

    #[test]
    fn repo_tab_overflow_can_appear_before_visible_tabs() {
        let widths = vec![96.0; 7];
        let visibility = repo_tab_visibility(&widths, None, Some(5), false, 570.0);

        assert_eq!(
            visibility.visible_items,
            vec![
                RepoTabVisibilityItem::Repo(3),
                RepoTabVisibilityItem::Repo(4),
                RepoTabVisibilityItem::Repo(5),
                RepoTabVisibilityItem::Repo(6),
            ]
        );
        assert_eq!(
            visibility.leading_overflow_items,
            vec![
                RepoTabVisibilityItem::Repo(0),
                RepoTabVisibilityItem::Repo(1),
                RepoTabVisibilityItem::Repo(2),
            ]
        );
        assert!(visibility.trailing_overflow_items.is_empty());
    }

    #[test]
    fn repo_tab_overflow_keeps_linear_order_when_activating_overflow_tab() {
        let widths = vec![96.0; 7];
        let visibility = repo_tab_visibility(&widths, None, Some(1), false, 570.0);

        assert!(visibility.leading_overflow_items.is_empty());
        assert_eq!(
            visibility.visible_items,
            vec![
                RepoTabVisibilityItem::Repo(0),
                RepoTabVisibilityItem::Repo(1),
                RepoTabVisibilityItem::Repo(2),
                RepoTabVisibilityItem::Repo(3),
            ]
        );
        assert_eq!(
            visibility.trailing_overflow_items,
            vec![
                RepoTabVisibilityItem::Repo(4),
                RepoTabVisibilityItem::Repo(5),
                RepoTabVisibilityItem::Repo(6),
            ]
        );
    }

    #[test]
    fn repo_tab_overflow_can_appear_on_both_sides_around_active_tab() {
        let widths = vec![96.0; 7];
        let visibility = repo_tab_visibility(&widths, None, Some(3), false, 590.0);

        assert!(visibility.visible_repo_indices.contains(&3));
        assert!(!visibility.leading_overflow_items.is_empty());
        assert!(!visibility.trailing_overflow_items.is_empty());
        assert_eq!(
            visibility.visible_items,
            vec![
                RepoTabVisibilityItem::Repo(2),
                RepoTabVisibilityItem::Repo(3),
                RepoTabVisibilityItem::Repo(4),
            ]
        );
    }

    #[test]
    fn repo_tab_overflow_keeps_source_tab_visible_when_active() {
        let widths = vec![120.0; 5];
        let visibility = repo_tab_visibility(&widths, Some(120.0), None, true, 390.0);

        assert!(visibility.source_visible);
        assert!(!visibility.source_overflow);
        assert!(visibility.has_leading_overflow() || visibility.has_trailing_overflow());
    }

    #[test]
    fn repo_tab_overflow_shows_everything_when_width_fits() {
        let widths = vec![110.0; 3];
        let visibility = repo_tab_visibility(&widths, Some(110.0), Some(1), false, 620.0);

        assert_eq!(visibility.visible_repo_indices, vec![0, 1, 2]);
        assert!(visibility.source_visible);
        assert!(!visibility.has_leading_overflow());
        assert!(!visibility.has_trailing_overflow());
    }

    #[test]
    fn repo_tab_reorder_moves_tabs_and_preserves_active_repository() {
        let mut tabs = vec![
            RepoTab {
                root: PathBuf::from("D:/repo-a"),
                name: "repo-a".to_owned(),
            },
            RepoTab {
                root: PathBuf::from("D:/repo-b"),
                name: "repo-b".to_owned(),
            },
            RepoTab {
                root: PathBuf::from("D:/repo-c"),
                name: "repo-c".to_owned(),
            },
        ];

        let active = reorder_repo_tabs(&mut tabs, Some(1), 0, 2);

        assert_eq!(tabs[0].name, "repo-b");
        assert_eq!(tabs[1].name, "repo-c");
        assert_eq!(tabs[2].name, "repo-a");
        assert_eq!(active, Some(0));

        let active = reorder_repo_tabs(&mut tabs, active, 2, 1);

        assert_eq!(tabs[0].name, "repo-b");
        assert_eq!(tabs[1].name, "repo-a");
        assert_eq!(tabs[2].name, "repo-c");
        assert_eq!(active, Some(0));
    }

    #[test]
    fn repo_tabs_are_draggable_to_reorder_visible_tabs() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let top_bar_start = implementation_source.find("fn top_bar_panel(").unwrap();
        let top_bar_end = implementation_source[top_bar_start..]
            .find("fn repository_source_view(")
            .unwrap();
        let top_bar_source = &implementation_source[top_bar_start..top_bar_start + top_bar_end];
        let tab_start = implementation_source
            .find("fn repo_tab_with_close(")
            .unwrap();
        let tab_end = implementation_source[tab_start..]
            .find("fn repo_tab_overflow_button(")
            .unwrap();
        let tab_source = &implementation_source[tab_start..tab_start + tab_end];

        assert!(tab_source.contains("Sense::click_and_drag()"));
        assert!(top_bar_source.contains("repo_tab_drag.dragging_index"));
        assert!(top_bar_source.contains("drag_started()"));
        assert!(top_bar_source.contains("reorder_repo_tabs("));
        assert!(top_bar_source.contains("self.save_repo_tabs()"));
    }

    #[test]
    fn active_repo_root_prefers_current_tab_over_snapshot_root() {
        let tabs = vec![
            RepoTab {
                root: PathBuf::from("D:/repo-a"),
                name: "repo-a".to_owned(),
            },
            RepoTab {
                root: PathBuf::from("D:/repo-b"),
                name: "repo-b".to_owned(),
            },
        ];
        let snapshot_root = Some(PathBuf::from("D:/stale-snapshot"));

        let root = active_repo_root_for(&tabs, Some(1), snapshot_root.as_ref());

        assert_eq!(root, Some(PathBuf::from("D:/repo-b")));
    }

    #[test]
    fn repo_tab_switch_uses_cached_snapshot_before_async_refresh() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let fields_start = implementation_source
            .find("pub struct GitAgentApp")
            .unwrap();
        let fields_end = implementation_source[fields_start..]
            .find("struct RepoTab")
            .unwrap();
        let fields_source = &implementation_source[fields_start..fields_start + fields_end];
        assert!(fields_source.contains("snapshot_cache: HashMap<String, RepositorySnapshot>"));
        assert!(
            fields_source.contains("repo_task: Option<Receiver<RepoTaskResult>>"),
            "repo task must carry the requested root so stale async refreshes cannot repaint old repos"
        );

        let load_start = implementation_source
            .find("fn load_repository(&mut self, path: PathBuf)")
            .unwrap();
        let load_end = implementation_source[load_start..]
            .find("fn open_repository_source_tab")
            .unwrap();
        let load_source = &implementation_source[load_start..load_start + load_end];
        assert!(load_source.contains("self.apply_cached_snapshot_for(&path)"));
        assert!(load_source.contains("self.clear_repository_snapshot_view()"));
        assert!(load_source.contains("let requested_root = path.clone();"));
        assert!(load_source.contains("sender.send((requested_root, git::open_repository(path)))"));

        let clear_start = implementation_source
            .find("fn clear_repository_snapshot_view(")
            .unwrap();
        let clear_end = implementation_source[clear_start..]
            .find("fn apply_repository_snapshot(")
            .unwrap();
        let clear_source = &implementation_source[clear_start..clear_start + clear_end];
        assert!(clear_source.contains("self.snapshot = None"));

        let poll_start = implementation_source.find("fn poll_tasks(").unwrap();
        let poll_end = implementation_source[poll_start..]
            .find("fn poll_merge_tool_task(")
            .unwrap();
        let poll_source = &implementation_source[poll_start..poll_start + poll_end];
        assert!(poll_source.contains("Ok((requested_root, Ok(snapshot)))"));
        assert!(poll_source.contains("self.cache_repository_snapshot(&snapshot)"));
        assert!(poll_source.contains("if self.active_repo_root_matches(&requested_root)"));
        assert!(poll_source.contains("self.apply_repository_snapshot(snapshot)"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn file_manager_path_uses_windows_separators_for_explorer() {
        let arg = file_manager_target_arg(Path::new("D:/workspace/git-Agent"));

        assert_eq!(arg, "D:\\workspace\\git-Agent");
    }

    #[test]
    fn sidebar_tree_state_defaults_and_persists_per_repository() {
        let default_state = SidebarTreeState::default();
        assert!(default_state.branches_open);
        assert!(!default_state.tags_open);
        assert!(!default_state.remotes_open);
        assert!(!default_state.stashes_open);

        let mut state = RepoTabsState::default();
        state.sidebar_tree_states.insert(
            "D:/repo/a".to_owned(),
            SidebarTreeState {
                branches_open: false,
                tags_open: true,
                remotes_open: true,
                stashes_open: false,
            },
        );
        let raw = serde_json::to_string(&state).unwrap();
        assert!(raw.contains("sidebar_tree_states"));
        assert!(raw.contains("D:/repo/a"));
    }

    #[test]
    fn toolbar_buttons_allow_multilingual_inline_width() {
        assert_eq!(TOOLBAR_BUTTON_HEIGHT, 18.0);
        assert!(TOOLBAR_BUTTON_MAX_WIDTH > 96.0);
        let source = include_str!("app.rs");
        let button_start = source.find("impl<'a> AppButton<'a>").unwrap();
        let button_end = source[button_start..]
            .find("fn toolbar_button_normal_fill(")
            .unwrap();
        let button_source = &source[button_start..button_start + button_end];
        assert!(button_source.contains("(11.0, 0.0, 18.0, 18.0, 18.0"));
        assert!(button_source.contains("ui.spacing_mut().button_padding = Vec2::new(4.0, 0.0)"));
        assert!(button_source.contains("toolbar_button_normal_fill()"));
        assert!(button_source.contains("toolbar_button_hover_fill()"));
        assert!(button_source.contains("widgets.inactive.bg_fill = toolbar_button_normal_fill()"));
        assert!(button_source.contains("widgets.hovered.bg_fill = toolbar_button_hover_fill()"));
        assert!(!button_source.contains(".fill(toolbar_button_fill())"));
        assert_eq!(
            inline_button_width_from_text(
                90.0,
                TOOLBAR_BUTTON_MIN_WIDTH,
                TOOLBAR_BUTTON_MAX_WIDTH,
                TOOLBAR_BUTTON_X_PADDING
            ),
            126.0
        );
        assert_eq!(
            inline_button_width_from_text(
                300.0,
                TOOLBAR_BUTTON_MIN_WIDTH,
                TOOLBAR_BUTTON_MAX_WIDTH,
                TOOLBAR_BUTTON_X_PADDING
            ),
            TOOLBAR_BUTTON_MAX_WIDTH
        );
    }

    #[test]
    fn tool_row_corners_join_tabs_without_right_top_radius() {
        let corners = tool_row_corners();
        assert_eq!(corners.nw, 0);
        assert_eq!(corners.ne, 0);
        assert_eq!(corners.sw, 6);
        assert_eq!(corners.se, 6);
    }

    #[test]
    fn file_status_icons_are_plain_iconify_assets() {
        assert_eq!(file_status_icon('M'), UiIcon::Edit);
        assert_eq!(file_status_icon('A'), UiIcon::AddFile);
        assert_eq!(file_status_icon('?'), UiIcon::AddFile);
        assert_eq!(file_status_icon('D'), UiIcon::DeleteFile);
        assert_eq!(file_status_icon('R'), UiIcon::RenameFile);
        assert_eq!(file_status_icon('U'), UiIcon::Warning);
        assert_eq!(
            file_status_color('A', false),
            Color32::from_rgb(42, 166, 109)
        );
        assert_eq!(
            file_status_color('?', false),
            Color32::from_rgb(42, 166, 109)
        );
        assert_eq!(
            file_status_color('D', false),
            Color32::from_rgb(220, 76, 70)
        );
        assert_eq!(
            file_status_color('U', false),
            Color32::from_rgb(232, 174, 55)
        );
        assert_eq!(file_status_color('M', true), Color32::WHITE);
        assert_eq!(file_status_color('A', true), Color32::WHITE);
        assert_eq!(file_status_color('D', true), Color32::WHITE);
    }

    #[test]
    fn worktree_conflicts_have_resolve_entry() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let dialog_start = implementation_source
            .find("WorktreeActionDialog::ResolveConflicts { selected_path } =>")
            .unwrap();
        let dialog_end = implementation_source[dialog_start..]
            .find("if let Some((path, side)) = accept_side")
            .unwrap();
        let dialog_source = &implementation_source[dialog_start..dialog_start + dialog_end];
        assert!(implementation_source.contains("fn worktree_conflict_files("));
        assert!(implementation_source.contains("WorktreeActionDialog::ResolveConflicts"));
        assert!(dialog_source.contains("window_control_button(ui, \"\\u{00d7}\", true)"));
        assert!(dialog_source.contains("egui::Area::new"));
        assert!(dialog_source.contains("conflict_resolution_modal_rect(ctx)"));
        assert!(dialog_source.contains("Layout::top_down(Align::Min)"));
        assert!(dialog_source.contains("let content_height = ui.available_height()"));
        assert!(dialog_source.contains("let panel_size"));
        assert!(dialog_source.contains("Vec2::new(CONFLICT_LIST_PANEL_SIZE.x, content_height)"));
        assert!(!dialog_source.contains(".anchor(Align2::CENTER_CENTER, Vec2::ZERO)"));
        assert!(implementation_source.contains("conflict_resolution_list_panel("));
        assert!(implementation_source.contains("conflict_resolution_actions_panel("));
        assert!(implementation_source.contains("CONFLICT_ACTION_BUTTON_SIZE"));
        assert!(implementation_source.contains("CONFLICT_MODAL_SIZE"));
        assert!(
            implementation_source
                .contains("CONFLICT_MODAL_SIZE: Vec2 = Vec2 { x: 760.0, y: 360.0 }")
        );
        assert!(implementation_source.contains("conflict_resolution_dialog_background()"));
        assert!(
            implementation_source
                .contains("soft_panel_frame(conflict_resolution_dialog_background(), 12, 12)")
        );
        assert!(dialog_source.contains(".fixed_pos(modal_rect.min)"));
        assert!(dialog_source.contains("safe_set_min_size(ui, CONFLICT_MODAL_SIZE)"));
        assert!(dialog_source.contains("ui.set_max_size(CONFLICT_MODAL_SIZE)"));
        assert!(implementation_source.contains("CONFLICT_LIST_PANEL_SIZE"));
        assert!(implementation_source.contains("CONFLICT_ACTION_PANEL_SIZE"));
        assert!(implementation_source.contains("worktree.conflicts.empty"));
        assert!(!dialog_source.contains("ui.set_min_size(ui.available_size())"));
        assert!(!implementation_source.contains("ui.available_width() - action_width"));
        assert!(!implementation_source.contains("ui.available_height() - 4.0"));
        assert!(implementation_source.contains("worktree.accept_yours"));
        assert!(implementation_source.contains("worktree.accept_theirs"));
        assert!(implementation_source.contains("worktree.resolve_conflicts"));
        assert!(implementation_source.contains("worktree_header_action_button("));
        assert!(implementation_source.contains("WorktreeMenuAction::ResolveConflict"));
        assert!(implementation_source.contains("fn open_conflict_merge_tool("));
        assert!(
            implementation_source
                .contains("merge_tool_task: Option<Receiver<MergeToolTaskResult>>")
        );
        assert!(implementation_source.contains("fn poll_merge_tool_task("));
        assert!(implementation_source.contains("fn merge_tool_busy(&self) -> bool"));
        assert!(implementation_source.contains("self.merge_tool_busy()"));
        assert!(implementation_source.contains(".wait()"));
        assert!(implementation_source.contains("self.load_repository_uncached(root)"));
        assert!(implementation_source.contains("self.merge_tool_task = Some(receiver)"));
        assert!(implementation_source.contains(".arg(\"--repo-root\")"));
        assert!(implementation_source.contains(".arg(\"--stage\")"));
        assert!(implementation_source.contains(".arg(\"--theme\")"));
        assert!(implementation_source.contains(".arg(merge_theme_arg(self.theme_mode))"));
        assert!(implementation_source.contains(".arg(\"--language\")"));
        assert!(implementation_source.contains(".arg(merge_language_arg(self.language))"));

        let row_start = implementation_source.find("fn worktree_file_row(").unwrap();
        let row_end = implementation_source[row_start..]
            .find("fn commit_context_menu(")
            .unwrap();
        let row_source = &implementation_source[row_start..row_start + row_end];
        assert!(row_source.contains("file.is_conflicted()"));
        assert!(row_source.contains("\"U\""));
        assert!(row_source.contains("worktree.resolve_conflict"));
    }

    #[test]
    fn merge_tool_binary_uses_windows_gui_subsystem() {
        let source = include_str!("bin/git-agent-merge.rs");
        assert!(source.contains("windows_subsystem = \"windows\""));
    }

    #[test]
    fn plus_icon_uses_crisp_16px_asset() {
        let plus = include_str!("../assets/icons/plus.svg");
        let add_file = include_str!("../assets/icons/add-file.svg");
        let edit = include_str!("../assets/icons/edit.svg");
        let delete_file = include_str!("../assets/icons/delete-file.svg");
        let rename_file = include_str!("../assets/icons/rename-file.svg");
        assert!(plus.contains("viewBox=\"0 0 16 16\""));
        assert!(add_file.contains("viewBox=\"0 0 16 16\""));
        assert!(!add_file.contains("V7l-5-5"));
        for icon in [plus, add_file, edit, delete_file, rename_file] {
            assert!(icon.contains("#ffffff"));
            assert!(!icon.contains("#c7d0df"));
        }
    }

    #[test]
    fn search_dimension_labels_are_localized() {
        assert_eq!(
            search_dimension_label(Language::Chinese, SearchDimension::Message),
            "\u{63d0}\u{4ea4}\u{4fe1}\u{606f}"
        );
        assert_eq!(
            search_dimension_label(Language::Chinese, SearchDimension::Files),
            "\u{6587}\u{4ef6}\u{53d8}\u{5316}"
        );
        assert_eq!(
            search_dimension_label(Language::Chinese, SearchDimension::Author),
            "\u{4f5c}\u{8005}"
        );
    }

    #[test]
    fn resource_tables_use_dense_rows_and_localized_headers() {
        assert!(RESOURCE_ROW_HEIGHT <= 30.0);
        assert!(RESOURCE_TABLE_HEADER_HEIGHT <= 24.0);
        assert_eq!(
            resource_label(Language::Chinese, "name"),
            "\u{540d}\u{79f0}"
        );
        assert_eq!(
            resource_label(Language::Chinese, "status"),
            "\u{72b6}\u{6001}"
        );
        assert_eq!(
            resource_label(Language::Chinese, "stash"),
            "\u{8d2e}\u{85cf}"
        );
    }

    #[test]
    fn settings_and_repo_settings_are_separate_flows() {
        assert!(SETTINGS_DIALOG_WIDTH >= 760.0);
        assert!(SETTINGS_DIALOG_HEIGHT >= 560.0);
        assert!(SETTINGS_NAV_WIDTH >= 180.0);
        let source = include_str!("app.rs");
        assert!(source.contains("fn settings_modal"));
        assert!(source.contains("fn repo_settings_modal"));
        assert!(source.contains("self.tr(\"options.title\")"));
        assert!(source.contains("self.tr(\"repo.settings.title\")"));
        let global_start = source.find("fn global_settings_page").unwrap();
        let global_end = source.find("fn repo_remotes_settings_page").unwrap();
        assert!(!source[global_start..global_end].contains("Repository panels"));
        assert!(source[global_start..global_end].contains("settings_choice_button("));
        assert!(source[global_start..global_end].contains("settings_accent_button("));
        let choice_start = source.find("fn settings_choice_button(").unwrap();
        let choice_end = source[choice_start..]
            .find("fn settings_accent_button(")
            .unwrap();
        let choice_source = &source[choice_start..choice_start + choice_end];
        assert!(choice_source.contains("if selected {\n            Color32::WHITE"));
        assert!(choice_source.contains("theme::text()"));
        assert_eq!(
            settings_tab_label(Language::Chinese, SettingsTab::General),
            "\u{901a}\u{7528}"
        );
        assert_eq!(
            settings_tab_label(Language::Chinese, SettingsTab::RepoRemotes),
            "\u{8fdc}\u{7aef}\u{4ed3}\u{5e93}"
        );
    }

    #[test]
    fn repo_settings_dialog_uses_compact_title_and_real_tabs() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let modal_start = implementation_source
            .find("fn repo_settings_modal(&mut self")
            .unwrap();
        let modal_end = implementation_source[modal_start..]
            .find("fn global_settings_page")
            .unwrap();
        let modal_source = &implementation_source[modal_start..modal_start + modal_end];
        let tabs_start = implementation_source
            .find("fn repo_settings_tab_strip(")
            .unwrap();
        let tabs_end = implementation_source[tabs_start..]
            .find("fn repo_settings_tab_button(")
            .unwrap();
        let tabs_source = &implementation_source[tabs_start..tabs_start + tabs_end];
        let tab_button_start = implementation_source
            .find("fn repo_settings_tab_button(")
            .unwrap();
        let tab_button_end = implementation_source[tab_button_start..]
            .find("fn settings_section_title(")
            .unwrap();
        let tab_button_source =
            &implementation_source[tab_button_start..tab_button_start + tab_button_end];

        assert!(implementation_source.contains("const SETTINGS_DIALOG_TITLE_HEIGHT: f32 = 32.0"));
        assert!(implementation_source.contains("const SETTINGS_DIALOG_TITLE_SIZE: f32 = 18.0"));
        assert!(modal_source.contains("settings_dialog_title_row("));
        assert!(modal_source.contains("repo_settings_tab_strip("));
        assert!(modal_source.contains("settings_dialog_body_frame().show"));
        assert!(
            modal_source
                .contains("settings_dialog_title_row(ui, self.tr(\"repo.settings.title\"), size.x")
        );
        assert!(modal_source.contains("window_control_button(ui, \"\\u{00d7}\", true)"));
        assert!(
            !modal_source.contains("settings_dialog_header(ui, self.tr(\"repo.settings.title\"))")
        );
        assert!(REPO_SETTINGS_TABS_HEIGHT <= 34.0);
        assert!(REPO_SETTINGS_TAB_HEIGHT <= 30.0);
        assert!(REPO_SETTINGS_TAB_WIDTH >= 104.0);
        assert!(tabs_source.contains("soft_panel_frame("));
        assert!(tabs_source.contains(".shadow(panel_shadow())"));
        assert!(tabs_source.contains("Layout::left_to_right(Align::Center)"));
        assert!(!tabs_source.contains("ui.add_space(12.0);"));
        assert!(tab_button_source.contains("Align2::LEFT_CENTER"));
        assert!(!tab_button_source.contains("rect.bottom() - 13.0"));
    }

    #[test]
    fn repo_settings_tabs_do_not_overlap_from_nested_icon_ui() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let tab_button_start = implementation_source
            .find("fn repo_settings_tab_button(")
            .unwrap();
        let tab_button_end = implementation_source[tab_button_start..]
            .find("fn settings_section_title(")
            .unwrap();
        let tab_button_source =
            &implementation_source[tab_button_start..tab_button_start + tab_button_end];

        assert!(!tab_button_source.contains("draw_ui_icon("));
        assert!(!tab_button_source.contains("allocate_new_ui("));
        assert!(tab_button_source.contains("paint_ui_icon("));
        assert!(tab_button_source.contains("ui.painter().with_clip_rect("));
        assert!(tab_button_source.contains("text_clip"));
    }

    #[test]
    fn repo_settings_remote_add_and_edit_open_action_dialog() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let remotes_start = implementation_source
            .find("fn repo_remotes_settings_page")
            .unwrap();
        let remotes_end = implementation_source[remotes_start..]
            .find("fn repo_advanced_settings_page")
            .unwrap();
        let remotes_source = &implementation_source[remotes_start..remotes_start + remotes_end];
        let modal_start = implementation_source
            .find("fn repo_remote_action_modal")
            .unwrap();
        let modal_end = implementation_source[modal_start..]
            .find("fn remote_settings_table(")
            .unwrap();
        let modal_source = &implementation_source[modal_start..modal_start + modal_end];

        assert!(implementation_source.contains("enum RepoRemoteActionDialog"));
        assert!(
            implementation_source
                .contains("pending_repo_remote_action: Option<RepoRemoteActionDialog>")
        );
        assert!(implementation_source.contains("self.repo_remote_action_modal(ctx);"));
        assert!(implementation_source.contains("RepoRemoteActionDialog::Add"));
        assert!(implementation_source.contains("RepoRemoteActionDialog::Edit"));
        assert!(remotes_source.contains("self.begin_add_remote_settings()"));
        assert!(remotes_source.contains("self.begin_edit_remote_settings("));
        assert!(!remotes_source
            .contains("false,\n                        egui::Button::new(i18n::t(language, \"repo.settings.add\"))"));
        assert!(!remotes_source
            .contains("false,\n                        egui::Button::new(i18n::t(language, \"repo.settings.edit\"))"));
        assert!(!remotes_source.contains("repo_remote_details_card("));
        assert!(modal_source.contains("compact_action_dialog("));
        assert!(modal_source.contains("repo_settings_account_dropdown("));
        assert!(modal_source.contains("validate_repo_remote_action_dialog("));
        assert!(!modal_source.contains("legacy_account_settings"));
        assert!(!modal_source.contains("host_url"));
        assert!(!modal_source.contains("username"));
        assert!(implementation_source.contains("fn repo_settings_content_width("));
    }

    #[test]
    fn global_options_configure_validated_remote_accounts_for_dropdown() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let app_settings_start = implementation_source.find("struct AppSettings").unwrap();
        let app_settings_end = implementation_source[app_settings_start..]
            .find("impl Default for AppSettings")
            .unwrap();
        let app_settings_source =
            &implementation_source[app_settings_start..app_settings_start + app_settings_end];
        let global_start = implementation_source
            .find("fn global_settings_page")
            .unwrap();
        let global_end = implementation_source[global_start..]
            .find("fn repo_remotes_settings_page")
            .unwrap();
        let global_source = &implementation_source[global_start..global_start + global_end];

        assert!(implementation_source.contains("struct RemoteAccountSettings"));
        assert!(app_settings_source.contains("remote_accounts: Vec<RemoteAccountSettings>"));
        assert!(implementation_source.contains(
            "remote_accounts: normalized_remote_accounts(&app_settings.remote_accounts)"
        ));
        assert!(implementation_source.contains("remote_accounts: self.remote_accounts.clone()"));
        assert!(global_source.contains("self.global_remote_accounts_settings(ui);"));
        assert!(implementation_source.contains("fn validate_remote_account_settings("));
        assert!(implementation_source.contains("fn remote_account_host_is_valid("));
        assert!(implementation_source.contains("repo.settings.account_validation_failed"));
        assert!(implementation_source.contains("repo_settings_account_dropdown("));
        assert_eq!(
            validate_remote_account_settings("Generic Account", "https://github.com"),
            Ok(())
        );
        assert!(validate_remote_account_settings("", "https://github.com").is_err());
        assert!(validate_remote_account_settings("Work", "not host with spaces").is_err());
    }

    #[test]
    fn repo_settings_uses_shadow_gap_cards_for_remote_and_advanced_pages() {
        let source = include_str!("app.rs");
        let implementation_source = &source[..source.find("#[cfg(test)]").unwrap()];
        let modal_start = implementation_source
            .find("fn repo_settings_modal(&mut self")
            .unwrap();
        let modal_end = implementation_source[modal_start..]
            .find("fn global_settings_page")
            .unwrap();
        let modal_source = &implementation_source[modal_start..modal_start + modal_end];
        let remotes_start = implementation_source
            .find("fn repo_remotes_settings_page")
            .unwrap();
        let remotes_end = implementation_source[remotes_start..]
            .find("fn repo_advanced_settings_page")
            .unwrap();
        let remotes_source = &implementation_source[remotes_start..remotes_start + remotes_end];
        let advanced_start = implementation_source
            .find("fn repo_advanced_settings_page")
            .unwrap();
        let advanced_end = implementation_source[advanced_start..]
            .find("fn remote_settings_table(")
            .unwrap();
        let advanced_source = &implementation_source[advanced_start..advanced_start + advanced_end];
        let card_start = implementation_source
            .find("fn repo_settings_card(")
            .unwrap();
        let card_end = implementation_source[card_start..]
            .find("fn repo_settings_tab_strip(")
            .unwrap();
        let card_source = &implementation_source[card_start..card_start + card_end];
        let tabs_start = implementation_source
            .find("fn repo_settings_tab_strip(")
            .unwrap();
        let tabs_end = implementation_source[tabs_start..]
            .find("fn repo_settings_tab_button(")
            .unwrap();
        let tabs_source = &implementation_source[tabs_start..tabs_start + tabs_end];

        assert!(REPO_SETTINGS_DIALOG_HEIGHT <= 460.0);
        assert!(REPO_SETTINGS_TABS_HEIGHT <= 42.0);
        assert!(REPO_SETTINGS_TAB_HEIGHT <= 38.0);
        assert!(modal_source.contains("repo_settings_dialog_height(self.repo_settings_tab)"));
        assert!(modal_source.contains("repo_settings_tab_strip("));
        assert!(!modal_source.contains("nav_width"));
        assert!(!modal_source.contains("content_height = safe_ui_length"));
        assert!(tabs_source.contains("allocate_ui_with_layout"));
        assert!(tabs_source.contains("REPO_SETTINGS_TABS_HEIGHT"));
        assert!(!tabs_source.contains("horizontal_centered"));
        assert!(card_source.contains(".shadow(panel_shadow())"));
        assert!(card_source.contains(".stroke(Stroke::NONE)"));
        assert!(remotes_source.contains("repo_settings_card("));
        assert!(remotes_source.contains("remote_settings_table("));
        assert!(!remotes_source.contains("repo_remote_details_card("));
        assert!(!remotes_source.contains("egui::Grid::new(\"repo_remotes_grid\")"));
        assert!(advanced_source.contains("snapshot.config"));
        assert!(advanced_source.contains("repo_settings_readonly_text("));
        assert!(advanced_source.contains("repo_settings_commit_links_panel("));
        assert!(advanced_source.contains("settings_checkbox_row("));
        assert!(modal_source.contains("open_repo_config_file()"));
        assert_eq!(
            settings_tab_label(Language::Chinese, SettingsTab::RepoAdvanced),
            "\u{9ad8}\u{7ea7}"
        );
    }
}
