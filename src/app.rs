use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc::{self, Receiver},
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
const MENU_BAR_HEIGHT: f32 = 28.0;
const TOP_BAR_HEIGHT: f32 = TITLE_BAR_HEIGHT + MENU_BAR_HEIGHT + TOP_BAR_ROW_HEIGHT * 2.0;
const TOP_BAR_ROW_HEIGHT: f32 = 40.0;
const TOP_BAR_GLOBAL_WIDTH: f32 = 480.0;
const TOP_BAR_MIN_TABS_WIDTH: f32 = 420.0;
const TOOLBAR_BUTTON_HEIGHT: f32 = 28.0;
const TOOLBAR_BUTTON_ICON: f32 = 13.0;
const TOOLBAR_BUTTON_TEXT: f32 = 11.0;
const TOOLBAR_BUTTON_X_PADDING: f32 = 36.0;
const TOOLBAR_BUTTON_MIN_WIDTH: f32 = 48.0;
const TOOLBAR_BUTTON_MAX_WIDTH: f32 = 160.0;
const FILE_ROW_HEIGHT: f32 = 24.0;
const FILE_ROW_ICON_SLOT: f32 = 24.0;
const FILE_ROW_LEFT_INSET: f32 = 10.0;
const HISTORY_TABLE_HEADER_HEIGHT: f32 = 24.0;
const HISTORY_TABLE_ROW_HEIGHT: f32 = 22.0;
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
const SETTINGS_NAV_WIDTH: f32 = 190.0;
const SETTINGS_FOOTER_HEIGHT: f32 = 44.0;
const LAYOUT_GAP: i8 = 8;
const RESIZE_HANDLE_THICKNESS: f32 = 8.0;

pub struct GitAgentApp {
    repo_tabs: Vec<RepoTab>,
    active_repo_tab: Option<usize>,
    source_tab_open: bool,
    repo_source_tab: RepoSourceTab,
    snapshot: Option<RepositorySnapshot>,
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
    clone_url: String,
    clone_destination: String,
    create_repo_path: String,
    clone_url_status: CloneUrlStatus,
    clone_url_last_edited: Option<Instant>,
    clone_url_task: Option<Receiver<(String, anyhow::Result<()>)>>,
    search_dimension: SearchDimension,
    repo_task: Option<Receiver<anyhow::Result<RepositorySnapshot>>>,
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
    selected_diff_rows: Vec<DiffLineKey>,
    history_rows_cache: HistoryRowsCache,
    selected_worktree_file: Option<SelectedWorktreeFile>,
    loading_repo: bool,
    loading_details_hash: Option<String>,
    loading_diff_key: Option<String>,
    pending_commit_action: Option<CommitActionDialog>,
    last_notice: Option<String>,
    toast_notice: Option<(String, Instant)>,
    pending_worktree_action: Option<WorktreeActionDialog>,
    commit_message: String,
    language: Language,
    pending_stash_action: Option<StashActionDialog>,
    pending_branch_action: Option<BranchActionDialog>,
    pending_tag_action: Option<TagActionDialog>,
    active_view: MainView,
    branches_open: bool,
    tags_open: bool,
    remotes_open: bool,
    stashes_open: bool,
    sidebar_tree_states: HashMap<String, SidebarTreeState>,
    settings_open: bool,
    settings_tab: SettingsTab,
    repo_settings_open: bool,
    repo_settings_tab: SettingsTab,
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

#[derive(Clone, Debug)]
struct KnownRepository {
    root: PathBuf,
    name: String,
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
    },
    ConfirmCheckout {
        hash: String,
        short_hash: String,
    },
    ConfirmCherryPick {
        hash: String,
        short_hash: String,
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
}

#[derive(Clone, Debug)]
enum WorktreeActionDialog {
    ConfirmDiscard { path: String, untracked: bool },
}

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
    CheckoutRemote {
        remote_branch: String,
        local_branch: String,
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
    Create { name: String },
    ConfirmDelete { name: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedWorktreeFile {
    path: String,
    display_path: String,
    staged: bool,
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
    history_graph_pct: f32,
    history_top_pct: f32,
    history_desc_pct: f32,
    history_date_pct: f32,
    history_author_pct: f32,
    history_hash_pct: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default)]
struct AppSettings {
    theme: SettingsThemeMode,
    theme_accent: SettingsThemeAccent,
    language: SettingsLanguage,
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
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
        .map(|base| base.join("Git Agent"))
}

fn app_settings_path() -> Option<PathBuf> {
    app_data_dir().map(|base| base.join("settings.json"))
}

fn repo_tabs_path() -> Option<PathBuf> {
    app_data_dir().map(|base| base.join("tabs.json"))
}

fn repo_state_key(path: &Path) -> String {
    path.display().to_string()
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

impl Default for LayoutPrefs {
    fn default() -> Self {
        Self {
            sidebar_pct: 0.19,
            details_pct: 0.32,
            workspace_list_pct: 0.58,
            workspace_staged_pct: 0.5,
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
            source_tab_open: tabs_state.source_tab_open,
            repo_source_tab: RepoSourceTab::Local,
            snapshot: None,
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
            clone_url: String::new(),
            clone_destination: String::new(),
            create_repo_path: String::new(),
            clone_url_status: CloneUrlStatus::Empty,
            clone_url_last_edited: None,
            clone_url_task: None,
            search_dimension: SearchDimension::Message,
            repo_task: None,
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
            selected_diff_rows: Vec::new(),
            history_rows_cache: HistoryRowsCache::default(),
            selected_worktree_file: None,
            loading_repo: false,
            loading_details_hash: None,
            loading_diff_key: None,
            pending_commit_action: None,
            last_notice: None,
            toast_notice: None,
            pending_worktree_action: None,
            commit_message: String::new(),
            language: app_settings.language.into(),
            pending_stash_action: None,
            pending_branch_action: None,
            pending_tag_action: None,
            active_view: MainView::Workspace,
            branches_open: SidebarTreeState::default().branches_open,
            tags_open: SidebarTreeState::default().tags_open,
            remotes_open: SidebarTreeState::default().remotes_open,
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
        self.ensure_repo_tab(path.clone());
        let (sender, receiver) = mpsc::channel();
        self.repo_task = Some(receiver);
        self.loading_repo = true;
        self.error = None;

        thread::spawn(move || {
            let _ = sender.send(git::open_repository(path));
        });
    }

    fn open_repository_source_tab(&mut self) {
        self.source_tab_open = true;
        self.active_repo_tab = None;
        self.active_view = MainView::Workspace;
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
        self.save_repo_tabs();
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
        self.snapshot.as_ref().map(|snapshot| snapshot.root.clone())
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

    fn open_remote_panel(&mut self) {
        self.repo_settings_tab = SettingsTab::RepoRemotes;
        self.repo_settings_open = true;
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

    fn execute_git_action(&mut self, action: impl FnOnce(&std::path::Path) -> anyhow::Result<()>) {
        let Some(root) = self.snapshot.as_ref().map(|snapshot| snapshot.root.clone()) else {
            return;
        };

        match action(&root) {
            Ok(()) => {
                self.error = None;
                self.last_notice = Some(self.tr("status.action_completed").to_owned());
                self.load_repository(root);
            }
            Err(error) => {
                self.error = Some(error.to_string());
                self.last_notice = None;
            }
        }
    }

    fn fetch_all(&mut self) {
        self.execute_git_action(|root| git::fetch(root));
    }

    fn pull_current(&mut self) {
        self.execute_git_action(|root| git::pull(root));
    }

    fn push_current(&mut self) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let branch = snapshot.branch.clone();
        let remote = snapshot
            .remotes
            .first()
            .map(|remote| remote.name.clone())
            .unwrap_or_else(|| "origin".to_owned());
        let has_upstream = snapshot.upstream.is_some();

        self.execute_git_action(move |root| {
            if has_upstream {
                git::push(root)
            } else {
                git::push_set_upstream(root, &remote, &branch)
            }
        });
    }

    fn poll_tasks(&mut self, ctx: &egui::Context) {
        if let Some(receiver) = self.repo_task.take() {
            match receiver.try_recv() {
                Ok(Ok(snapshot)) => {
                    let mut snapshot = snapshot;
                    self.apply_history_sort_order_to_snapshot(&mut snapshot);
                    self.layout = graph::layout(&snapshot.commits);
                    self.selected_commit = (!snapshot.commits.is_empty()).then_some(0);
                    self.search_selected_commit = None;
                    self.sync_active_tab_with_snapshot(&snapshot);
                    self.apply_sidebar_tree_state_for_repo(&snapshot.root);
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
                    self.selected_worktree_file = None;
                    self.loading_repo = false;
                    self.error = None;
                    self.request_selected_details();
                    ctx.request_repaint();
                }
                Ok(Err(error)) => {
                    self.snapshot = None;
                    self.layout = GraphLayout::default();
                    self.selected_commit = None;
                    self.search_selected_commit = None;
                    self.loading_repo = false;
                    self.error = Some(error.to_string());
                    ctx.request_repaint();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.repo_task = Some(receiver);
                    ctx.request_repaint_after(std::time::Duration::from_millis(80));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loading_repo = false;
                    self.error = Some("Repository loader stopped unexpectedly".to_owned());
                    ctx.request_repaint();
                }
            }
        }

        if let Some(receiver) = self.repo_source_task.take() {
            match receiver.try_recv() {
                Ok(Ok(path)) => {
                    self.last_notice = Some(self.tr("status.action_completed").to_owned());
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

        thread::spawn(move || {
            let _ = sender.send(git::load_worktree_diff(
                root,
                &selected.path,
                selected.staged,
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
            TITLE_BAR_HEIGHT + MENU_BAR_HEIGHT + TOP_BAR_ROW_HEIGHT
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
        }
        .save();
    }

    fn save_repo_tabs(&self) {
        RepoTabsState::from_app(self).save();
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
                    .inner_margin(egui::Margin::symmetric(LAYOUT_GAP, LAYOUT_GAP)),
            )
            .show(ctx, |ui| self.main_layout(ui));

        self.commit_action_modal(ctx);
        self.worktree_action_modal(ctx);
        self.stash_action_modal(ctx);
        self.branch_action_modal(ctx);
        self.tag_action_modal(ctx);
        self.settings_modal(ctx);
        self.repo_settings_modal(ctx);
        self.toast_overlay(ctx);
    }
}

impl GitAgentApp {
    fn main_layout(&mut self, ui: &mut Ui) {
        let full = ui.available_rect_before_wrap();
        let height = full.height();
        let full_width = full.width();
        let gap = LAYOUT_GAP as f32;
        if self.repository_source_active() {
            content_panel_frame(theme::panel()).show(ui, |ui| {
                ui.set_min_size(frame_inner_size(
                    full_width,
                    height,
                    CONTENT_PANEL_INSET_X,
                    CONTENT_PANEL_INSET_Y,
                ));
                self.repository_source_view(ui);
            });
            ui.allocate_rect(full, Sense::hover());
            return;
        }

        let details_visible = view_uses_side_details(self.active_view);
        self.layout_prefs.clamp();

        let mut sidebar_width = (full_width * self.layout_prefs.sidebar_pct)
            .clamp(220.0, 340.0)
            .min(full_width * 0.34);
        let mut details_width = if details_visible {
            (full_width * self.layout_prefs.details_pct)
                .clamp(340.0, 640.0)
                .min(full_width * 0.46)
        } else {
            0.0
        };
        let details_gap = if details_visible { gap } else { 0.0 };
        let min_center = 360.0;
        let min_sidebar = 200.0;
        let min_details = if details_visible { 320.0 } else { 0.0 };
        let spare = full_width - gap - details_gap - min_center;
        if sidebar_width + details_width > spare {
            let overflow = sidebar_width + details_width - spare;
            if details_visible {
                details_width = (details_width - overflow).max(min_details);
            }
            if sidebar_width + details_width > spare {
                sidebar_width = (spare - details_width).max(min_sidebar);
            }
        }
        let center_width =
            (full_width - sidebar_width - gap - details_width - details_gap).max(min_center);

        let sidebar_rect = Rect::from_min_size(full.left_top(), Vec2::new(sidebar_width, height));
        let center_rect = Rect::from_min_size(
            Pos2::new(sidebar_rect.right() + gap, full.top()),
            Vec2::new(center_width, height),
        );
        let details_rect = Rect::from_min_size(
            Pos2::new(center_rect.right() + gap, full.top()),
            Vec2::new(details_width, height),
        );
        let sidebar_center_gap = Rect::from_min_max(
            Pos2::new(sidebar_rect.right(), full.top()),
            Pos2::new(center_rect.left(), full.bottom()),
        );
        let center_details_gap = Rect::from_min_max(
            Pos2::new(center_rect.right(), full.top()),
            Pos2::new(details_rect.left(), full.bottom()),
        );
        let clip_pad = gap;

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(sidebar_rect), |ui| {
            ui.set_clip_rect(sidebar_rect.expand(clip_pad).intersect(full));
            soft_panel_frame(theme::panel(), LAYOUT_GAP, LAYOUT_GAP).show(ui, |ui| {
                ui.set_min_size(frame_inner_size(
                    sidebar_width,
                    height,
                    LAYOUT_GAP,
                    LAYOUT_GAP,
                ));
                self.sidebar(ui);
            });
        });
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(center_rect), |ui| {
            ui.set_clip_rect(center_rect.expand(clip_pad).intersect(full));
            content_panel_frame(theme::panel()).show(ui, |ui| {
                ui.set_min_size(frame_inner_size(
                    center_width,
                    height,
                    CONTENT_PANEL_INSET_X,
                    CONTENT_PANEL_INSET_Y,
                ));
                match self.active_view {
                    MainView::Workspace => self.workspace_view(ui),
                    MainView::History => self.history_view(ui),
                    MainView::Search => self.search_view(ui),
                    MainView::Branches => self.branches_view(ui),
                    MainView::Tags => self.tags_view(ui),
                    MainView::Stashes => self.stashes_view(ui),
                }
            });
        });
        if details_visible {
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(details_rect), |ui| {
                ui.set_clip_rect(details_rect.expand(clip_pad).intersect(full));
                content_panel_frame(theme::panel()).show(ui, |ui| {
                    ui.set_min_size(frame_inner_size(
                        details_width,
                        height,
                        CONTENT_PANEL_INSET_X,
                        CONTENT_PANEL_INSET_Y,
                    ));
                    self.details(ui);
                });
            });
        }
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

    fn custom_title_bar(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        let rect = ui.max_rect();
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, theme::panel());

        let controls_width = 128.0;
        let drag_rect = Rect::from_min_max(
            rect.left_top(),
            Pos2::new(rect.right() - controls_width, rect.bottom()),
        );
        let drag_response = ui.interact(
            drag_rect,
            ui.id().with("custom_title_drag_region"),
            Sense::click_and_drag(),
        );
        if drag_response.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if drag_response.double_clicked() {
            let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(8.0);
                app_title_logo(ui);
                ui.label(
                    RichText::new("Git Agent")
                        .strong()
                        .size(13.0)
                        .color(theme::text()),
                );
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
                        !self.loading_repo && has_remote,
                        egui::Button::new(self.tr("action.fetch")),
                    )
                    .clicked()
                {
                    self.fetch_all();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        !self.loading_repo && has_remote,
                        egui::Button::new(self.tr("action.pull")),
                    )
                    .clicked()
                {
                    self.pull_current();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        !self.loading_repo && has_remote,
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

        let full = ui.max_rect();
        let top_y = full.top();
        let top_bar_height = self.top_bar_height();
        let title_row = Rect::from_min_max(
            full.left_top(),
            Pos2::new(full.right(), top_y + TITLE_BAR_HEIGHT),
        );
        let menu_row = Rect::from_min_max(
            Pos2::new(full.left(), title_row.bottom()),
            Pos2::new(full.right(), title_row.bottom() + MENU_BAR_HEIGHT),
        );
        let tab_row = Rect::from_min_max(
            Pos2::new(full.left(), menu_row.bottom()),
            Pos2::new(full.right(), menu_row.bottom() + TOP_BAR_ROW_HEIGHT),
        );
        let tool_row = Rect::from_min_max(
            Pos2::new(full.left(), tab_row.bottom()),
            Pos2::new(full.right(), top_y + top_bar_height),
        );
        ui.painter()
            .rect_filled(full, CornerRadius::ZERO, theme::panel());

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(title_row), |ui| {
            self.custom_title_bar(ctx, ui);
        });

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(menu_row), |ui| {
            self.desktop_menu_bar(ui, has_repo, has_remote);
        });

        let tab_left = Rect::from_min_max(
            tab_row.left_top(),
            Pos2::new(
                (tab_row.right() - TOP_BAR_GLOBAL_WIDTH)
                    .max(tab_row.left() + TOP_BAR_MIN_TABS_WIDTH),
                tab_row.bottom(),
            ),
        );
        let tab_right = Rect::from_min_max(
            Pos2::new(tab_left.right(), tab_row.top()),
            tab_row.right_bottom(),
        );

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(tab_left), |ui| {
            ScrollArea::horizontal()
                .id_salt("repo_tab_strip")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.add_space(8.0);
                        for (index, tab) in self.repo_tabs.iter().enumerate() {
                            let (tab_clicked, close_clicked) = repo_tab_with_close(
                                ui,
                                UiIcon::Folder,
                                self.active_repo_tab == Some(index),
                                &tab.name,
                                self.tr("repo.source.close_tab"),
                            );
                            if close_clicked {
                                close_repo_tab = Some(index);
                            } else if tab_clicked {
                                switch_to = Some(index);
                            }
                        }
                        if self.source_tab_open {
                            let (tab_clicked, close_clicked) = repo_tab_with_close(
                                ui,
                                UiIcon::Folder,
                                self.repository_source_active(),
                                self.tr("repo.source.new_tab"),
                                self.tr("repo.source.close_tab"),
                            );
                            if close_clicked {
                                close_source_tab = true;
                            } else if tab_clicked {
                                self.active_repo_tab = None;
                            }
                        }
                        if icon_button(
                            ui,
                            UiIcon::Plus,
                            self.tr("repo.source.new_tab"),
                            !self.loading_repo,
                        )
                        .clicked()
                        {
                            self.open_repository_source_tab();
                        }
                    });
                });
        });

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(tab_right), |ui| {
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
                    self.open_remote_panel();
                }
                if toolbar_button(ui, "git-flow", self.tr("repo.git_flow"), has_repo).clicked() {
                    self.open_git_workflow();
                }
            });
        });

        if !self.repository_source_active() {
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(tool_row), |ui| {
                ScrollArea::horizontal()
                    .id_salt("repo_toolbar_strip")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.add_space(16.0);
                            if toolbar_button(ui, "commit", self.tr("commit.panel"), true).clicked()
                            {
                                self.active_view = MainView::Workspace;
                            }
                            if toolbar_button(
                                ui,
                                "pull",
                                self.tr("action.pull"),
                                !self.loading_repo && has_repo && has_remote,
                            )
                            .clicked()
                            {
                                self.pull_current();
                            }
                            if toolbar_button(
                                ui,
                                "push",
                                self.tr("action.push"),
                                !self.loading_repo && has_repo && has_remote,
                            )
                            .clicked()
                            {
                                self.push_current();
                            }
                            if toolbar_button(
                                ui,
                                "fetch",
                                self.tr("action.fetch"),
                                !self.loading_repo && has_repo && has_remote,
                            )
                            .clicked()
                            {
                                self.fetch_all();
                            }
                            ui.add_space(LAYOUT_GAP as f32);
                            if toolbar_button(ui, "branch", self.tr("branch.local"), has_repo)
                                .clicked()
                            {
                                self.active_view = MainView::Branches;
                            }
                            if toolbar_button(ui, "tag", self.tr("tag.title"), has_repo).clicked() {
                                self.active_view = MainView::Tags;
                            }
                            if toolbar_button(ui, "stash", self.tr("stash.title"), has_repo)
                                .clicked()
                            {
                                self.active_view = MainView::Stashes;
                            }
                            if self.loading_repo {
                                ui.spinner();
                                ui.label(
                                    RichText::new(self.tr("status.loading_repo"))
                                        .color(theme::muted()),
                                );
                            }
                            if let Some(notice) = &self.last_notice {
                                ui.label(RichText::new(notice).color(theme::accent()));
                            }
                        });
                    });
            });
        }

        if let Some(index) = close_repo_tab {
            self.close_repo_tab(index);
            switch_to = None;
        }
        if close_source_tab {
            self.source_tab_open = false;
            if self.active_repo_tab.is_none() && !self.repo_tabs.is_empty() {
                switch_to = Some(self.repo_tabs.len() - 1);
            }
            self.save_repo_tabs();
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
                ui.add_space(12.0);
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
            ui.add_sized(
                [ui.available_width().min(800.0), 30.0],
                TextEdit::singleline(&mut self.repo_source_search).hint_text(search_hint),
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
        self.known_local_repositories()
            .into_iter()
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
            .collect()
    }

    fn known_local_repositories(&self) -> Vec<KnownRepository> {
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
        repositories
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

        if let Some(snapshot) = &self.snapshot {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new("*").color(theme::accent()));
                ui.label(
                    RichText::new(&snapshot.branch)
                        .strong()
                        .color(theme::text()),
                );
            });
            if let Some(upstream) = &snapshot.upstream {
                ui.horizontal(|ui| {
                    ui.add_space(30.0);
                    ui.label(
                        RichText::new(format!(
                            "{}  ahead {}  behind {}",
                            upstream.name, upstream.ahead, upstream.behind
                        ))
                        .small()
                        .color(theme::muted()),
                    );
                });
            }
            ui.add_space(8.0);

            let mut branch_action = None;
            let mut tag_action = None;
            let mut stash_action = None;
            let mut sidebar_state_changed = false;

            let branch_create_label = self.tr("branch.create");
            let branches_open_before = self.branches_open;
            let (branches_visible, create_branch_clicked) = tree_header_with_action(
                ui,
                &mut self.branches_open,
                UiIcon::Branch,
                i18n::t(self.language, "branch.local"),
                UiIcon::Plus,
                branch_create_label,
            );
            if create_branch_clicked {
                branch_action = Some(BranchMenuAction::Create);
            }
            if self.branches_open != branches_open_before {
                sidebar_state_changed = true;
            }
            if branches_visible {
                for branch in snapshot
                    .branches
                    .iter()
                    .filter(|branch| !branch.remote)
                    .take(18)
                {
                    branch_row(
                        ui,
                        branch.current,
                        branch.remote,
                        &branch.name,
                        self.language,
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
                    tree_empty(ui, self.tr("remote.none"));
                } else {
                    let remote_names = remote_group_names(snapshot);
                    for remote_name in remote_names.iter().take(8) {
                        remote_group_row(ui, remote_name);
                        for branch in remote_branches
                            .iter()
                            .filter(|branch| branch_belongs_to_remote(&branch.name, remote_name))
                            .take(18)
                        {
                            remote_branch_row(
                                ui,
                                &branch.name,
                                remote_branch_display_name(&branch.name, remote_name),
                                self.language,
                                &mut branch_action,
                            );
                        }
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
            if let Some(action) = branch_action {
                self.handle_branch_action(action);
            }
            if let Some(action) = tag_action {
                self.handle_tag_action(action);
            }
            if sidebar_state_changed {
                self.save_sidebar_tree_state_for_active_repo();
            }
        }

        if let Some(error) = &self.error {
            ui.add_space(18.0);
            ui.colored_label(theme::warning(), error);
        }
    }

    fn workspace_view(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        let Some(snapshot) = &self.snapshot else {
            empty_state(ui, self.loading_repo, self.language);
            return;
        };

        let staged = snapshot.staged.clone();
        let unstaged = snapshot.unstaged.clone();
        let status_count = snapshot.status.len();
        let mut worktree_action = None;
        let mut selected_worktree_after_draw = None;

        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new(self.tr("worktree.title"))
                    .heading()
                    .color(theme::text()),
            );
            ui.label(
                RichText::new(format!("{status_count}"))
                    .small()
                    .color(theme::muted()),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(12.0);
                if ui
                    .add_enabled(
                        !unstaged.is_empty(),
                        egui::Button::new(self.tr("worktree.stage_all")),
                    )
                    .clicked()
                {
                    worktree_action = Some(WorktreeMenuAction::StageAll);
                }
                if ui
                    .add_enabled(
                        !staged.is_empty(),
                        egui::Button::new(self.tr("worktree.unstage_all")),
                    )
                    .clicked()
                {
                    worktree_action = Some(WorktreeMenuAction::UnstageAll);
                }
            });
        });
        ui.add_space(10.0);

        if status_count == 0 {
            clean_worktree_state(
                ui,
                self.tr("worktree.clean"),
                self.tr("worktree.clean_detail"),
            );
            return;
        }

        let available_height = ui.available_height();
        let list_commit_gap = LAYOUT_GAP as f32;
        let list_height = (available_height * self.layout_prefs.workspace_list_pct)
            .clamp(220.0, (available_height - 260.0).max(220.0));
        ui.allocate_ui(Vec2::new(ui.available_width(), list_height), |ui| {
            let table_gap = LAYOUT_GAP as f32;
            let table_total = (ui.available_height() - table_gap).max(160.0);
            let staged_height = (table_total * self.layout_prefs.workspace_staged_pct)
                .clamp(86.0, (table_total - 86.0).max(86.0));
            let unstaged_height = (table_total - staged_height).max(86.0);
            worktree_table(
                ui,
                self.tr("worktree.staged"),
                &staged,
                true,
                staged_height,
                self.language,
                &mut worktree_action,
                &mut selected_worktree_after_draw,
            );
            let splitter_rect = ui
                .allocate_exact_size(Vec2::new(ui.available_width(), table_gap), Sense::hover())
                .0;
            if let Some(delta) =
                horizontal_resize_delta(ui, splitter_rect, "workspace_staged_unstaged_resize")
            {
                self.layout_prefs.workspace_staged_pct =
                    ((staged_height + delta) / table_total).clamp(0.24, 0.76);
                self.layout_prefs.save();
            }
            worktree_table(
                ui,
                self.tr("worktree.unstaged"),
                &unstaged,
                false,
                unstaged_height,
                self.language,
                &mut worktree_action,
                &mut selected_worktree_after_draw,
            );
        });

        let commit_splitter_rect = ui
            .allocate_exact_size(
                Vec2::new(ui.available_width(), list_commit_gap),
                Sense::hover(),
            )
            .0;
        if let Some(delta) =
            horizontal_resize_delta(ui, commit_splitter_rect, "workspace_list_commit_resize")
        {
            self.layout_prefs.workspace_list_pct =
                ((list_height + delta) / available_height).clamp(0.42, 0.74);
            self.layout_prefs.save();
        }
        self.commit_panel(ui, staged.len());

        if let Some(action) = worktree_action {
            self.handle_worktree_action(action);
        }
        if let Some(selected) = selected_worktree_after_draw {
            self.selected_worktree_file = Some(selected);
            self.selected_file_path = None;
            self.selected_diff_rows.clear();
            self.request_selected_worktree_diff();
        }
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
            "\u{663e}\u{793a}\u{8fdc}\u{7a0b}\u{5206}\u{652f}"
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

        let search_hint = if self.language == Language::Chinese {
            "\u{641c}\u{7d22}\u{63d0}\u{4ea4}"
        } else {
            "Search commits"
        };
        let search_width = 260.0_f32.min((toolbar_rect.width() - 380.0).max(120.0));
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
        if history_table_header(ui, self.language, graph_width, &mut self.layout_prefs) {
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
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(body_rect), |ui| {
            ui.set_min_size(body_rect.size());
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
                            let (response, copied_hash) = history_commit_table_row(
                                ui,
                                commit,
                                row,
                                graph_width,
                                lane_count,
                                &self.layout_prefs,
                                self.language,
                                is_selected,
                                self.history_show_remote_refs,
                            );
                            hash_copied |= copied_hash;
                            if response.clicked() {
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
                ui.set_min_size(frame_inner_size(left_width, available.y, 8, 8));
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    self.history_commit_summary(ui);
                });
            });
        });
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(right_rect), |ui| {
            source_tree_panel_frame().show(ui, |ui| {
                ui.set_min_size(frame_inner_size(right_width, available.y, 8, 8));
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
                ui.set_min_size(frame_inner_size(left_width, available.y, 8, 8));
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    self.search_commit_summary(ui);
                });
            });
        });
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(right_rect), |ui| {
            source_tree_panel_frame().show(ui, |ui| {
                ui.set_min_size(frame_inner_size(right_width, available.y, 8, 8));
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
        let mut action = None;

        content_panel_frame(theme::bg()).show(ui, |ui| {
            resource_header(
                ui,
                self.tr("branch.local"),
                &format!("{} local  {} remote", local.len(), remote.len()),
                |ui| {
                    if ui.button(self.tr("branch.create")).clicked() {
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
                        branch_table_row(
                            ui,
                            branch.current,
                            false,
                            &branch.name,
                            self.language,
                            &mut action,
                        );
                    }
                    for branch in &remote {
                        branch_table_row(
                            ui,
                            branch.current,
                            true,
                            &branch.name,
                            self.language,
                            &mut action,
                        );
                    }
                    if local.is_empty() && remote.is_empty() {
                        empty_list_panel(ui, self.tr("branch.none"));
                    }
                });
        });

        if let Some(action) = action {
            self.handle_branch_action(action);
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
                self.pending_commit_action = Some(CommitActionDialog::CreateTag {
                    hash,
                    short_hash: short_hash.clone(),
                    name: format!("v-{short_hash}"),
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
        }
    }

    fn handle_worktree_action(&mut self, action: WorktreeMenuAction) {
        match action {
            WorktreeMenuAction::Stage { path } => {
                self.execute_git_action(|root| git::stage_path(root, &path));
            }
            WorktreeMenuAction::StageAll => {
                self.execute_git_action(|root| git::stage_all(root));
            }
            WorktreeMenuAction::Unstage { path } => {
                self.execute_git_action(|root| git::unstage_path(root, &path));
            }
            WorktreeMenuAction::UnstageAll => {
                self.execute_git_action(|root| git::unstage_all(root));
            }
            WorktreeMenuAction::Discard { path, untracked } => {
                self.pending_worktree_action =
                    Some(WorktreeActionDialog::ConfirmDiscard { path, untracked });
            }
        }
    }

    fn handle_stash_action(&mut self, action: StashMenuAction) {
        match action {
            StashMenuAction::Create => {
                self.pending_stash_action = Some(StashActionDialog::Create {
                    message: String::new(),
                });
            }
            StashMenuAction::Apply { selector } => {
                self.execute_git_action(|root| git::stash_apply(root, &selector));
            }
            StashMenuAction::Pop { selector } => {
                self.execute_git_action(|root| git::stash_pop(root, &selector));
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
                self.execute_git_action(|root| git::checkout_branch(root, &name));
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
            BranchMenuAction::Delete { name } => {
                self.pending_branch_action =
                    Some(BranchActionDialog::ConfirmDelete { name, force: false });
            }
            BranchMenuAction::DeleteRemote { remote_branch } => {
                self.pending_branch_action =
                    Some(BranchActionDialog::ConfirmDeleteRemote { remote_branch });
            }
        }
    }

    fn handle_tag_action(&mut self, action: TagMenuAction) {
        match action {
            TagMenuAction::Create => {
                self.pending_tag_action = Some(TagActionDialog::Create {
                    name: String::new(),
                });
            }
            TagMenuAction::Checkout { name } => {
                self.execute_git_action(|root| git::checkout_tag(root, &name));
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

    fn commit_panel(&mut self, ui: &mut Ui, staged_count: usize) {
        ui.add_space(14.0);
        soft_panel_frame(theme::accent_soft(), 12, 10).show(ui, |ui| {
            panel_heading_inline(ui, self.tr("commit.panel"));
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("{staged_count} {}", self.tr("commit.staged_files")))
                    .small()
                    .color(theme::muted()),
            );
            let message_hint = self.tr("commit.message");
            ui.add_sized(
                [ui.available_width(), 58.0],
                TextEdit::multiline(&mut self.commit_message).hint_text(message_hint),
            );
            ui.add_space(8.0);
            let can_commit = staged_count > 0 && !self.commit_message.trim().is_empty();
            if ui
                .add_enabled(can_commit, egui::Button::new(self.tr("commit.button")))
                .clicked()
            {
                let message = self.commit_message.trim().to_owned();
                self.execute_git_action(|root| git::commit(root, &message));
                self.commit_message.clear();
            }
        });
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

        diff_panel_frame().show(ui, |ui| {
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

        diff_panel_frame().show(ui, |ui| {
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
    }

    fn worktree_diff_viewer(&self, ui: &mut Ui) {
        let Some(selected) = &self.selected_worktree_file else {
            return;
        };

        ui.add_space(8.0);
        ui.add_space(10.0);
        panel_heading_inline(ui, self.tr("worktree.title"));
        ui.label(
            RichText::new(&selected.display_path)
                .monospace()
                .color(theme::text()),
        );
        ui.add_space(6.0);

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

        diff_panel_frame().show(ui, |ui| {
            ScrollArea::both()
                .id_salt(("worktree_diff_scroll", key))
                .max_height(360.0)
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
    }

    fn commit_action_modal(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.pending_commit_action.take() else {
            return;
        };

        let mut keep_open = true;
        let mut close_after = false;
        let mut execute: Option<Box<dyn FnOnce(&std::path::Path) -> anyhow::Result<()>>> = None;

        let title = match &dialog {
            CommitActionDialog::CreateBranch { .. } => self.tr("branch.create"),
            CommitActionDialog::CreateTag { .. } => self.tr("menu.create_tag"),
            CommitActionDialog::ConfirmCheckout { .. } => self.tr("menu.checkout_commit"),
            CommitActionDialog::ConfirmCherryPick { .. } => self.tr("menu.cherry_pick"),
            CommitActionDialog::ConfirmRevert { .. } => self.tr("menu.revert"),
            CommitActionDialog::ConfirmReset { .. } => self.tr("menu.reset"),
        };

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_min_width(360.0);
                match &mut dialog {
                    CommitActionDialog::CreateBranch {
                        hash,
                        short_hash,
                        name,
                        checkout,
                    } => {
                        ui.label(
                            RichText::new(format!(
                                "{} {short_hash}",
                                self.tr("commit.create_from")
                            ))
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
                            if ui.button(self.tr("dialog.create")).clicked()
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
                        ui.add(TextEdit::singleline(name));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button(self.tr("dialog.create")).clicked()
                                && !name.trim().is_empty()
                            {
                                let tag_name = name.trim().to_owned();
                                let hash = hash.clone();
                                execute = Some(Box::new(move |root| {
                                    git::create_tag(root, &tag_name, &hash)
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
                        ui.label(
                            RichText::new(self.tr("commit.detached_warning"))
                                .color(theme::warning()),
                        );
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button(self.tr("dialog.checkout")).clicked() {
                                let hash = hash.clone();
                                execute =
                                    Some(Box::new(move |root| git::checkout_commit(root, &hash)));
                                close_after = true;
                            }
                            if ui.button(self.tr("dialog.cancel")).clicked() {
                                close_after = true;
                            }
                        });
                    }
                    CommitActionDialog::ConfirmCherryPick { hash, short_hash } => {
                        ui.label(
                            RichText::new(self.tr("commit.confirm_cherry_pick"))
                                .color(theme::text()),
                        );
                        ui.label(
                            RichText::new(short_hash.as_str())
                                .monospace()
                                .color(theme::muted()),
                        );
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button(self.tr("menu.cherry_pick")).clicked() {
                                let hash = hash.clone();
                                execute = Some(Box::new(move |root| {
                                    git::cherry_pick_commit(root, &hash)
                                }));
                                close_after = true;
                            }
                            if ui.button(self.tr("dialog.cancel")).clicked() {
                                close_after = true;
                            }
                        });
                    }
                    CommitActionDialog::ConfirmRevert { hash, short_hash } => {
                        ui.label(
                            RichText::new(self.tr("commit.confirm_revert")).color(theme::text()),
                        );
                        ui.label(
                            RichText::new(short_hash.as_str())
                                .monospace()
                                .color(theme::muted()),
                        );
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button(self.tr("menu.revert")).clicked() {
                                let hash = hash.clone();
                                execute =
                                    Some(Box::new(move |root| git::revert_commit(root, &hash)));
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
                        ui.label(
                            RichText::new(self.tr("commit.confirm_reset")).color(theme::warning()),
                        );
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
                            if ui.button(self.tr("menu.reset")).clicked() {
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
                }
            });

        if let Some(action) = execute {
            self.execute_git_action(action);
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
        let mut execute: Option<Box<dyn FnOnce(&std::path::Path) -> anyhow::Result<()>>> = None;

        match dialog {
            WorktreeActionDialog::ConfirmDiscard { path, untracked } => {
                egui::Window::new("Discard changes")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.set_min_width(380.0);
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
                            if ui.button("Discard").clicked() {
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
        }
    }

    fn stash_action_modal(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.pending_stash_action.take() else {
            return;
        };

        let mut keep_open = true;
        let mut close_after = false;
        let mut execute: Option<Box<dyn FnOnce(&std::path::Path) -> anyhow::Result<()>>> = None;

        match &mut dialog {
            StashActionDialog::Create { message } => {
                egui::Window::new(self.tr("stash.create"))
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.set_min_width(380.0);
                        let hint = self.tr("stash.message");
                        ui.add_sized(
                            [ui.available_width(), 34.0],
                            TextEdit::singleline(message).hint_text(hint),
                        );
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button(self.tr("stash.create")).clicked() {
                                let message = message.trim().to_owned();
                                execute =
                                    Some(Box::new(move |root| git::stash_push(root, &message)));
                                close_after = true;
                            }
                            if ui.button(self.tr("dialog.cancel")).clicked() {
                                close_after = true;
                            }
                        });
                    });
            }
            StashActionDialog::ConfirmDrop { selector, message } => {
                egui::Window::new(self.tr("stash.drop"))
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.set_min_width(380.0);
                        ui.label(RichText::new(self.tr("stash.confirm_drop")).color(theme::text()));
                        ui.label(RichText::new(message.as_str()).color(theme::muted()));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button(self.tr("stash.drop")).clicked() {
                                let selector = selector.clone();
                                execute =
                                    Some(Box::new(move |root| git::stash_drop(root, &selector)));
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
        let mut execute: Option<Box<dyn FnOnce(&std::path::Path) -> anyhow::Result<()>>> = None;

        match &mut dialog {
            BranchActionDialog::Create { name, checkout } => {
                egui::Window::new(self.tr("branch.create"))
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.set_min_width(380.0);
                        ui.label(
                            RichText::new(self.tr("branch.name"))
                                .small()
                                .color(theme::muted()),
                        );
                        ui.add(TextEdit::singleline(name));
                        ui.checkbox(checkout, self.tr("branch.checkout"));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button(self.tr("dialog.create")).clicked()
                                && !name.trim().is_empty()
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
            BranchActionDialog::CheckoutRemote {
                remote_branch,
                local_branch,
            } => {
                egui::Window::new(self.tr("branch.sync_remote"))
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.set_min_width(380.0);
                        ui.label(RichText::new(remote_branch.as_str()).color(theme::text()));
                        ui.label(
                            RichText::new(self.tr("branch.local_alias"))
                                .small()
                                .color(theme::muted()),
                        );
                        ui.add(TextEdit::singleline(local_branch));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button(self.tr("dialog.checkout")).clicked()
                                && !local_branch.trim().is_empty()
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
                    });
            }
            BranchActionDialog::ConfirmDelete { name, force } => {
                egui::Window::new(self.tr("branch.delete"))
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.set_min_width(380.0);
                        ui.label(
                            RichText::new(self.tr("branch.confirm_delete")).color(theme::text()),
                        );
                        ui.label(RichText::new(name.as_str()).color(theme::warning()));
                        ui.checkbox(force, self.tr("branch.force_delete"));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button(self.tr("branch.delete")).clicked() {
                                let name = name.clone();
                                let force = *force;
                                execute = Some(Box::new(move |root| {
                                    git::delete_branch(root, &name, force)
                                }));
                                close_after = true;
                            }
                            if ui.button(self.tr("dialog.cancel")).clicked() {
                                close_after = true;
                            }
                        });
                    });
            }
            BranchActionDialog::ConfirmDeleteRemote { remote_branch } => {
                egui::Window::new(self.tr("branch.delete_remote"))
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.set_min_width(380.0);
                        ui.label(
                            RichText::new(self.tr("branch.confirm_delete_remote"))
                                .color(theme::text()),
                        );
                        ui.label(RichText::new(remote_branch.as_str()).color(theme::warning()));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button(self.tr("branch.delete_remote")).clicked() {
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
            self.pending_branch_action = Some(dialog);
        }
    }

    fn tag_action_modal(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.pending_tag_action.take() else {
            return;
        };

        let mut keep_open = true;
        let mut close_after = false;
        let mut execute: Option<Box<dyn FnOnce(&std::path::Path) -> anyhow::Result<()>>> = None;

        match &mut dialog {
            TagActionDialog::Create { name } => {
                egui::Window::new(self.tr("tag.create"))
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.set_min_width(380.0);
                        ui.label(
                            RichText::new(self.tr("tag.name"))
                                .small()
                                .color(theme::muted()),
                        );
                        ui.add(TextEdit::singleline(name));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button(self.tr("dialog.create")).clicked()
                                && !name.trim().is_empty()
                            {
                                let name = name.trim().to_owned();
                                execute = Some(Box::new(move |root| {
                                    git::create_tag_at_head(root, &name)
                                }));
                                close_after = true;
                            }
                            if ui.button(self.tr("dialog.cancel")).clicked() {
                                close_after = true;
                            }
                        });
                    });
            }
            TagActionDialog::ConfirmDelete { name } => {
                egui::Window::new(self.tr("tag.delete"))
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.set_min_width(380.0);
                        ui.label(RichText::new(self.tr("tag.confirm_delete")).color(theme::text()));
                        ui.label(RichText::new(name.as_str()).color(theme::warning()));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if ui.button(self.tr("tag.delete")).clicked() {
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
                egui::Frame::new()
                    .fill(theme::panel())
                    .corner_radius(CornerRadius::same(7))
                    .shadow(panel_shadow())
                    .inner_margin(egui::Margin::symmetric(14, 12))
                    .show(ui, |ui| {
                        ui.set_width(size.x - 28.0);
                        ui.allocate_ui_with_layout(
                            Vec2::new(size.x - 28.0, 36.0),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                settings_dialog_header(ui, self.tr("options.title"));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if window_control_button(ui, "\u{00d7}", true).clicked() {
                                        close_requested = true;
                                    }
                                });
                            },
                        );
                        ui.add_space(10.0);
                        let content_height = size.y - SETTINGS_FOOTER_HEIGHT - 76.0;
                        soft_panel_frame(theme::panel(), 16, 12).show(ui, |ui| {
                            ui.set_min_size(frame_inner_size(
                                size.x - 28.0,
                                content_height,
                                16,
                                12,
                            ));
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
            (screen.width() * 0.58).clamp(520.0, SETTINGS_DIALOG_WIDTH),
            (screen.height() * 0.68).clamp(380.0, SETTINGS_DIALOG_HEIGHT),
        );
        let nav_width = (size.x * 0.28).clamp(150.0, SETTINGS_NAV_WIDTH);
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
                egui::Frame::new()
                    .fill(theme::panel())
                    .corner_radius(CornerRadius::same(7))
                    .shadow(panel_shadow())
                    .inner_margin(egui::Margin::symmetric(14, 12))
                    .show(ui, |ui| {
                        ui.set_width(size.x - 28.0);
                        ui.allocate_ui_with_layout(
                            Vec2::new(size.x - 28.0, 36.0),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                settings_dialog_header(ui, self.tr("repo.settings.title"));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if window_control_button(ui, "\u{00d7}", true).clicked() {
                                        close_requested = true;
                                    }
                                });
                            },
                        );
                        ui.add_space(10.0);
                        let content_height = size.y - SETTINGS_FOOTER_HEIGHT - 76.0;
                        ui.allocate_ui_with_layout(
                            Vec2::new(size.x - 28.0, content_height),
                            Layout::left_to_right(Align::TOP),
                            |ui| {
                                ui.allocate_ui(Vec2::new(nav_width, content_height), |ui| {
                                    soft_panel_frame(theme::panel(), 8, 8).show(ui, |ui| {
                                        ui.set_min_size(frame_inner_size(
                                            nav_width,
                                            content_height,
                                            8,
                                            8,
                                        ));
                                        ui.vertical(|ui| {
                                            settings_nav_item(
                                                ui,
                                                &mut self.repo_settings_tab,
                                                SettingsTab::RepoRemotes,
                                                UiIcon::Folder,
                                                settings_tab_label(
                                                    self.language,
                                                    SettingsTab::RepoRemotes,
                                                ),
                                            );
                                            settings_nav_item(
                                                ui,
                                                &mut self.repo_settings_tab,
                                                SettingsTab::RepoAdvanced,
                                                UiIcon::Branch,
                                                settings_tab_label(
                                                    self.language,
                                                    SettingsTab::RepoAdvanced,
                                                ),
                                            );
                                        });
                                    });
                                });
                                ui.add_space(10.0);
                                let page_width = size.x - nav_width - 66.0;
                                ui.allocate_ui(Vec2::new(page_width, content_height), |ui| {
                                    soft_panel_frame(theme::panel(), 16, 12).show(ui, |ui| {
                                        ui.set_min_size(frame_inner_size(
                                            page_width,
                                            content_height,
                                            16,
                                            12,
                                        ));
                                        ScrollArea::vertical()
                                            .id_salt("repo_settings_content_scroll")
                                            .auto_shrink([false, false])
                                            .show(ui, |ui| {
                                                ui.vertical(|ui| match self.repo_settings_tab {
                                                    SettingsTab::RepoRemotes => {
                                                        self.repo_remotes_settings_page(ui)
                                                    }
                                                    SettingsTab::RepoAdvanced => {
                                                        self.repo_advanced_settings_page(ui)
                                                    }
                                                    SettingsTab::General => {}
                                                });
                                            });
                                    });
                                });
                            },
                        );
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
    }

    fn repo_remotes_settings_page(&mut self, ui: &mut Ui) {
        settings_section_title(ui, "Remote repositories");
        egui::Grid::new("repo_remotes_grid")
            .striped(true)
            .min_col_width(120.0)
            .show(ui, |ui| {
                ui.label(RichText::new("Name").strong());
                ui.label(RichText::new("URL").strong());
                ui.end_row();
                if let Some(snapshot) = &self.snapshot {
                    for remote in &snapshot.remotes {
                        ui.label(&remote.name);
                        ui.add(
                            egui::Label::new(
                                RichText::new(if remote.fetch_url.is_empty() {
                                    &remote.push_url
                                } else {
                                    &remote.fetch_url
                                })
                                .monospace(),
                            )
                            .wrap(),
                        );
                        ui.end_row();
                    }
                }
            });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_enabled(false, egui::Button::new("Add"));
            ui.add_enabled(false, egui::Button::new("Edit"));
            ui.add_enabled(false, egui::Button::new("Remove"));
        });
    }

    fn repo_advanced_settings_page(&mut self, ui: &mut Ui) {
        settings_section_title(ui, "Repository");
        if let Some(snapshot) = &self.snapshot {
            field_row(ui, "Repository", &snapshot.root.display().to_string());
            field_row(ui, "Current branch", &snapshot.branch);
        }
        ui.add_space(12.0);
        settings_section_title(ui, "User");
        field_row(ui, "Name", "Emssion");
        field_row(ui, "Email", "configured by git");
        ui.add_space(12.0);
        settings_section_title(ui, "Options");
        let mut auto_refresh = true;
        let mut refresh_remote = true;
        settings_checkbox_row(
            ui,
            &mut auto_refresh,
            "Automatically refresh this repository",
        );
        settings_checkbox_row(
            ui,
            &mut refresh_remote,
            "Refresh remote status in background",
        );
    }
}

#[derive(Clone, Debug)]
enum WorktreeMenuAction {
    Stage { path: String },
    StageAll,
    Unstage { path: String },
    UnstageAll,
    Discard { path: String, untracked: bool },
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
    Checkout { name: String },
    CheckoutRemote { remote_branch: String },
    Delete { name: String },
    DeleteRemote { remote_branch: String },
}

#[derive(Clone, Debug)]
enum TagMenuAction {
    Create,
    Checkout { name: String },
    Delete { name: String },
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

fn panel_shadow() -> egui::epaint::Shadow {
    egui::epaint::Shadow {
        offset: [3, 4],
        blur: 9,
        spread: 0,
        color: theme::accent_shadow(),
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

fn frame_inner_size(width: f32, height: f32, x_margin: i8, y_margin: i8) -> Vec2 {
    Vec2::new(
        (width - f32::from(x_margin) * 2.0).max(0.0),
        (height - f32::from(y_margin) * 2.0).max(0.0),
    )
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
            AppButtonStyle::IconOnly => (14.0, 0.0, 30.0, 30.0, 30.0, theme::panel_soft(), 4),
            AppButtonStyle::Toolbar => (
                TOOLBAR_BUTTON_ICON,
                TOOLBAR_BUTTON_TEXT,
                TOOLBAR_BUTTON_MIN_WIDTH,
                TOOLBAR_BUTTON_MAX_WIDTH,
                TOOLBAR_BUTTON_HEIGHT,
                theme::panel_soft(),
                4,
            ),
            AppButtonStyle::RepoTab { selected } => (
                14.0,
                12.0,
                110.0,
                220.0,
                28.0,
                if selected {
                    theme::accent_deep()
                } else {
                    theme::panel()
                },
                3,
            ),
        };

        let tint = if self.enabled {
            theme::text()
        } else {
            theme::muted()
        };
        let image = egui::Image::new(icon_source(self.icon))
            .fit_to_exact_size(Vec2::splat(icon_size))
            .tint(match self.style {
                AppButtonStyle::RepoTab { selected: true } => Color32::WHITE,
                AppButtonStyle::RepoTab { selected: false } => theme::accent(),
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
                        AppButtonStyle::RepoTab { selected: true } => Color32::WHITE,
                        _ => theme::text(),
                    }),
            )
            .min_size(Vec2::new(
                inline_button_width(ui, self.label, text_size, min_width, max_width, 38.0),
                height,
            )),
        }
        .fill(fill)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(radius));

        ui.add_enabled(self.enabled, button)
            .on_hover_text(self.label)
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
        if ui.visuals().dark_mode {
            Color32::from_rgb(46, 53, 68)
        } else {
            Color32::from_rgb(214, 224, 235)
        }
    } else if ui.visuals().dark_mode {
        Color32::from_rgb(34, 39, 51)
    } else {
        Color32::from_rgb(225, 232, 240)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(14.0),
        if close { Color32::WHITE } else { theme::text() },
    );
    response
}

fn menu_button(ui: &mut Ui, label: &'static str, add_contents: impl FnOnce(&mut Ui)) {
    ui.menu_button(
        RichText::new(label).color(theme::text()).size(13.0),
        add_contents,
    );
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
        (_, "name") => "Name",
        (_, "type") => "Type",
        (_, "status") => "Status",
        (_, "target") => "Target",
        (_, "message") => "Message",
        (_, "when") => "When",
        (_, "stash") => "Stash",
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
    let width = ui.available_width();
    let target_w = 150.0;
    let name_w = 220.0;
    let subject_w = (width - name_w - target_w).max(220.0);
    ui.horizontal(|ui| {
        table_header_cell(ui, resource_label(language, "name"), name_w);
        table_header_cell(ui, resource_label(language, "target"), target_w);
        table_header_cell(ui, resource_label(language, "message"), subject_w);
    });
    ui.add_space(6.0);
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
        .fill(Color32::from_rgb(255, 255, 255))
        .shadow(panel_shadow())
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
    response
}

fn header_action_button_at(ui: &mut Ui, rect: Rect, label: &str) -> egui::Response {
    let response = ui.interact(rect, ui.id().with(("header_action", label)), Sense::click());
    let fill = if response.hovered() {
        Color32::from_rgb(238, 244, 248)
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
    let response = ui.interact(rect, button_id, Sense::click());
    if response.clicked() {
        ui.memory_mut(|memory| memory.toggle_popup(popup_id));
    }

    let fill = if response.hovered() {
        Color32::from_rgb(224, 235, 241)
    } else {
        Color32::from_rgb(232, 240, 245)
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
    ui.selectable_label(selected, text)
}

fn history_toolbar_checkbox_at(
    ui: &mut Ui,
    rect: Rect,
    value: &mut bool,
    label: &str,
) -> egui::Response {
    let mut response = ui.interact(
        rect,
        ui.make_persistent_id("history_remote_refs_checkbox"),
        Sense::click(),
    );
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
        Color32::from_rgb(232, 241, 244)
    } else {
        Color32::from_rgb(246, 249, 251)
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
) -> bool {
    let width = ui.available_width();
    let cols = history_column_widths(width, graph_width, prefs);
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

    let content_left = rect.left() + graph_width;
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
                    adjust_history_graph_width(prefs, width, graph_width, cols.desc, delta);
                } else {
                    adjust_history_column_widths(
                        prefs,
                        &cols,
                        width - graph_width - 8.0,
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
) -> (egui::Response, bool) {
    let response = ui.allocate_response(
        Vec2::new(ui.available_width(), HISTORY_TABLE_ROW_HEIGHT),
        Sense::click(),
    );
    let rect = response.rect;
    let painter = ui.painter();
    let row_bg = if selected {
        Color32::from_rgb(42, 137, 232)
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

    draw_history_graph_cell(ui, rect, row, graph_width, lane_count);

    let width = rect.width();
    let cols = history_column_widths(width, graph_width, prefs);
    let mut x = rect.left() + graph_width;
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
    (response, hash_copied)
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
    for name in refs.iter().take(3) {
        let label = truncate_middle(name, 22);
        let width = (label.chars().count() as f32 * 6.6 + 18.0).clamp(34.0, 142.0);
        if x + width + 6.0 > rect.right() {
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
        painter.text(
            badge_rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::monospace(11.0),
            Color32::from_rgb(26, 65, 112),
        );
        x += width + 6.0;
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
    let response = ui.allocate_response(Vec2::new(ui.available_width(), 24.0), Sense::click());
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
    let response = ui.allocate_response(
        Vec2::new(ui.available_width(), RESOURCE_ROW_HEIGHT),
        Sense::click(),
    );
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

    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(label)
}

fn repo_tab_with_close(
    ui: &mut Ui,
    icon: UiIcon,
    selected: bool,
    label: &str,
    close_label: &str,
) -> (bool, bool) {
    let text_width = ui.fonts(|fonts| {
        fonts
            .layout_no_wrap(label.to_owned(), FontId::proportional(12.0), theme::text())
            .rect
            .width()
    });
    let width = (text_width + 70.0).clamp(108.0, 204.0);
    let (_, response) = ui.allocate_exact_size(Vec2::new(width, 28.0), Sense::click());
    let rect = response.rect;
    let show_close = selected || response.hovered();
    let fill = if selected {
        theme::accent_deep()
    } else if response.hovered() {
        theme::accent_soft()
    } else {
        theme::panel()
    };
    let text_color = if selected {
        Color32::WHITE
    } else {
        theme::text()
    };
    let icon_color = if selected {
        Color32::WHITE
    } else {
        theme::accent()
    };
    let close_rect = Rect::from_center_size(
        Pos2::new(rect.right() - 12.0, rect.center().y),
        Vec2::splat(18.0),
    );
    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.left() + 17.0, rect.center().y),
        Vec2::splat(14.0),
    );
    let text_rect = Rect::from_min_max(
        Pos2::new(rect.left() + 34.0, rect.top()),
        Pos2::new(close_rect.left() - 2.0, rect.bottom()),
    );

    ui.painter().rect_filled(rect, CornerRadius::same(3), fill);
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
        let close_color = if selected {
            Color32::WHITE
        } else {
            theme::muted()
        };
        if close_response.hovered() {
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

    (response.clicked() && !close_clicked, close_clicked)
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

fn settings_dialog_header(ui: &mut Ui, title: &str) {
    ui.horizontal(|ui| {
        let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
        draw_ui_icon(ui, icon_rect, UiIcon::Settings, theme::accent());
        ui.label(
            RichText::new(title)
                .size(22.0)
                .strong()
                .color(theme::text()),
        );
    });
}

fn settings_tab_label(language: Language, tab: SettingsTab) -> &'static str {
    match (language, tab) {
        (Language::Chinese, SettingsTab::General) => "\u{901a}\u{7528}",
        (Language::Chinese, SettingsTab::RepoRemotes) => "\u{4ed3}\u{5e93}\u{8fdc}\u{7a0b}",
        (Language::Chinese, SettingsTab::RepoAdvanced) => "\u{4ed3}\u{5e93}\u{9ad8}\u{7ea7}",
        (_, SettingsTab::General) => "General",
        (_, SettingsTab::RepoRemotes) => "Repository Remotes",
        (_, SettingsTab::RepoAdvanced) => "Repository Advanced",
    }
}

fn settings_nav_item(
    ui: &mut Ui,
    current: &mut SettingsTab,
    tab: SettingsTab,
    icon: UiIcon,
    label: &str,
) {
    let selected = *current == tab;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(1.0, 2.0)),
            CornerRadius::same(4),
            if selected {
                theme::accent_deep()
            } else {
                theme::panel_soft()
            },
        );
    }
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.allocate_ui_with_layout(rect.size(), Layout::left_to_right(Align::Center), |ui| {
            ui.add_space(8.0);
            let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
            draw_ui_icon(ui, icon_rect, icon, theme::accent());
            ui.add_sized(
                [(ui.available_width() - 8.0).max(40.0), 24.0],
                egui::Label::new(RichText::new(label).strong().color(if selected {
                    Color32::WHITE
                } else {
                    theme::muted()
                }))
                .truncate(),
            );
        });
    });
    if response.clicked() {
        *current = tab;
    }
}

fn field_row(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.set_min_height(28.0);
        ui.add_sized([120.0, 22.0], egui::Label::new(label));
        ui.add_sized(
            [ui.available_width().max(120.0), 22.0],
            egui::Label::new(RichText::new(value).monospace()).wrap(),
        );
    });
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
    tree_header_inner(ui, open, icon, label, Some((action_icon, action_label)))
}

fn tree_header_inner(
    ui: &mut Ui,
    open: &mut bool,
    icon: UiIcon,
    label: &str,
    action: Option<(UiIcon, &str)>,
) -> (bool, bool) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
    let mut action_clicked = false;
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.set_clip_rect(rect);
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
            if let Some((action_icon, action_label)) = action {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let response = icon_button(ui, action_icon, action_label, true);
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

fn worktree_table(
    ui: &mut Ui,
    title: &str,
    files: &[WorktreeFile],
    staged: bool,
    height: f32,
    language: Language,
    action: &mut Option<WorktreeMenuAction>,
    selected: &mut Option<SelectedWorktreeFile>,
) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let panel_rect = rect.shrink(2.0);
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(panel_rect), |ui| {
        ui.set_clip_rect(rect);
        soft_panel_frame(theme::panel(), 10, 8).show(ui, |ui| {
            ui.set_min_size(frame_inner_size(
                panel_rect.width(),
                panel_rect.height(),
                10,
                8,
            ));
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
                        for file in files {
                            if worktree_file_row(ui, file, staged, language, action).clicked() {
                                *selected = Some(SelectedWorktreeFile {
                                    path: file.path.clone(),
                                    display_path: file.display_path.clone(),
                                    staged,
                                });
                            }
                        }
                    });
            }
        });
    });
}

fn clean_worktree_state(ui: &mut Ui, text: &str, detail: &str) {
    ui.add_space(24.0);
    soft_panel_frame(theme::panel(), 18, 16).show(ui, |ui| {
        ui.set_min_height((ui.available_height() - 36.0).max(260.0));
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(text).size(22.0).strong().color(theme::text()));
                ui.add_space(6.0);
                ui.label(RichText::new(detail).color(theme::muted()));
            });
        });
    });
}

fn branch_table_row(
    ui: &mut Ui,
    current: bool,
    remote: bool,
    name: &str,
    language: Language,
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
    branch_context_menu(response, current, remote, name, language, action)
}

fn branch_context_menu(
    response: egui::Response,
    current: bool,
    remote: bool,
    name: &str,
    language: Language,
    action: &mut Option<BranchMenuAction>,
) -> egui::Response {
    response.context_menu(|ui| {
        ui.set_min_width(200.0);
        ui.label(RichText::new(name).color(theme::text()));
        ui.separator();
        if remote {
            if ui
                .button(i18n::t(language, "branch.checkout_remote"))
                .clicked()
            {
                *action = Some(BranchMenuAction::CheckoutRemote {
                    remote_branch: name.to_owned(),
                });
                ui.close_menu();
            }
            if ui
                .button(i18n::t(language, "branch.delete_remote"))
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
                    !current,
                    egui::Button::new(i18n::t(language, "branch.checkout")),
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
                    !current,
                    egui::Button::new(i18n::t(language, "branch.delete")),
                )
                .clicked()
            {
                *action = Some(BranchMenuAction::Delete {
                    name: name.to_owned(),
                });
                ui.close_menu();
            }
        }
    });
    response
}

fn tag_table_row(
    ui: &mut Ui,
    tag: &Tag,
    language: Language,
    action: &mut Option<TagMenuAction>,
) -> egui::Response {
    let (rect, response) = resource_row_response(ui);
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        let width = ui.available_width();
        let target_w = 150.0;
        let name_w = 220.0;
        let subject_w = (width - name_w - target_w).max(220.0);
        ui.horizontal(|ui| {
            ui.add_sized(
                [name_w, RESOURCE_ROW_HEIGHT],
                egui::Label::new(RichText::new(&tag.name).color(theme::accent())).truncate(),
            );
            ui.add_sized(
                [target_w, RESOURCE_ROW_HEIGHT],
                egui::Label::new(
                    RichText::new(&tag.target)
                        .monospace()
                        .small()
                        .color(theme::muted()),
                )
                .truncate(),
            );
            ui.add_sized(
                [subject_w, RESOURCE_ROW_HEIGHT],
                egui::Label::new(RichText::new(&tag.subject).small().color(theme::text()))
                    .truncate(),
            );
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
    language: Language,
    action: &mut Option<BranchMenuAction>,
) -> egui::Response {
    let response = ui.allocate_response(Vec2::new(ui.available_width(), 24.0), Sense::click());
    let rect = response.rect;
    if response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(2.0, 1.0)),
            CornerRadius::same(4),
            theme::accent_soft(),
        );
    }

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            let color = if current {
                theme::accent()
            } else if remote {
                theme::info()
            } else {
                theme::muted()
            };
            let label = if remote {
                i18n::t(language, "common.remote")
            } else {
                i18n::t(language, "common.local")
            };
            ui.label(RichText::new(if current { "*" } else { " " }).color(color));
            ui.label(RichText::new(name).color(if current {
                theme::text()
            } else {
                theme::muted()
            }));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(12.0);
                ui.label(RichText::new(label).small().color(color));
            });
        });
    });

    if remote && response.double_clicked() {
        *action = Some(BranchMenuAction::CheckoutRemote {
            remote_branch: name.to_owned(),
        });
    }

    response.context_menu(|ui| {
        ui.set_min_width(200.0);
        ui.label(RichText::new(name).color(theme::text()));
        ui.separator();
        if remote {
            if ui
                .button(i18n::t(language, "branch.checkout_remote"))
                .clicked()
            {
                *action = Some(BranchMenuAction::CheckoutRemote {
                    remote_branch: name.to_owned(),
                });
                ui.close_menu();
            }
        } else {
            if ui
                .add_enabled(
                    !current,
                    egui::Button::new(i18n::t(language, "branch.checkout")),
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
                    !current,
                    egui::Button::new(i18n::t(language, "branch.delete")),
                )
                .clicked()
            {
                *action = Some(BranchMenuAction::Delete {
                    name: name.to_owned(),
                });
                ui.close_menu();
            }
        }
    });

    response
}

fn remote_group_row(ui: &mut Ui, name: &str) {
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.label(RichText::new(name).strong().color(theme::text()));
    });
}

fn remote_branch_row(
    ui: &mut Ui,
    full_name: &str,
    display_name: &str,
    language: Language,
    action: &mut Option<BranchMenuAction>,
) -> egui::Response {
    let response = ui.allocate_response(Vec2::new(ui.available_width(), 24.0), Sense::click());
    let rect = response.rect;
    if response.hovered() || response.double_clicked() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(2.0, 1.0)),
            CornerRadius::same(4),
            theme::accent_soft(),
        );
    }

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.horizontal(|ui| {
            ui.add_space(28.0);
            ui.label(RichText::new(display_name).color(theme::muted()));
        });
    });

    if response.double_clicked() {
        *action = Some(BranchMenuAction::CheckoutRemote {
            remote_branch: full_name.to_owned(),
        });
    }

    response.context_menu(|ui| {
        ui.set_min_width(220.0);
        ui.label(RichText::new(full_name).color(theme::text()));
        ui.separator();
        if ui
            .button(i18n::t(language, "branch.checkout_remote"))
            .clicked()
        {
            *action = Some(BranchMenuAction::CheckoutRemote {
                remote_branch: full_name.to_owned(),
            });
            ui.close_menu();
        }
        if ui
            .button(i18n::t(language, "branch.delete_remote"))
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

fn remote_group_names(snapshot: &RepositorySnapshot) -> Vec<String> {
    let mut names = snapshot
        .remotes
        .iter()
        .map(|remote| remote.name.clone())
        .collect::<Vec<_>>();
    for branch in snapshot.branches.iter().filter(|branch| branch.remote) {
        if let Some((remote, _)) = branch.name.split_once('/') {
            if !names.iter().any(|name| name == remote) {
                names.push(remote.to_owned());
            }
        }
    }
    names
}

fn branch_belongs_to_remote(branch_name: &str, remote_name: &str) -> bool {
    branch_name
        .split_once('/')
        .is_some_and(|(remote, _)| remote == remote_name)
}

fn remote_branch_display_name<'a>(branch_name: &'a str, remote_name: &str) -> &'a str {
    branch_name
        .strip_prefix(remote_name)
        .and_then(|name| name.strip_prefix('/'))
        .unwrap_or(branch_name)
}

fn stash_row(
    ui: &mut Ui,
    stash: &StashEntry,
    language: Language,
    action: &mut Option<StashMenuAction>,
) -> egui::Response {
    let response = ui.allocate_response(Vec2::new(ui.available_width(), 42.0), Sense::click());
    let rect = response.rect;
    if response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(2.0, 2.0)),
            CornerRadius::same(4),
            theme::accent_soft(),
        );
    }

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
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
    let response = ui.allocate_response(Vec2::new(ui.available_width(), 38.0), Sense::click());
    let rect = response.rect;
    if response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(2.0, 2.0)),
            CornerRadius::same(4),
            theme::accent_soft(),
        );
    }

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
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

fn worktree_file_row(
    ui: &mut Ui,
    file: &WorktreeFile,
    staged: bool,
    language: Language,
    action: &mut Option<WorktreeMenuAction>,
) -> egui::Response {
    let status = if staged {
        file.index_status.to_string()
    } else if file.index_status == '?' {
        "A".to_owned()
    } else {
        file.worktree_status.to_string()
    };
    let response = ui.allocate_response(
        Vec2::new(ui.available_width(), FILE_ROW_HEIGHT),
        Sense::click(),
    );
    let rect = response.rect;
    if response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(2.0, 1.0)),
            CornerRadius::same(4),
            theme::accent_soft(),
        );
    }

    draw_file_row_content(
        ui,
        rect,
        FILE_ROW_LEFT_INSET,
        &status,
        &file.display_path,
        false,
    );

    response.context_menu(|ui| {
        ui.set_min_width(180.0);
        ui.label(
            RichText::new(&file.display_path)
                .monospace()
                .color(theme::text()),
        );
        ui.separator();
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
    });

    response
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
    ui.add_enabled(
        false,
        egui::Button::new(i18n::t(language, "menu.compare_worktree")),
    );
    ui.add_enabled(
        false,
        egui::Button::new(i18n::t(language, "menu.open_remote")),
    );
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
        'M' | 'R' => theme::info(),
        _ => theme::muted(),
    }
}

fn file_status_icon(kind: char) -> UiIcon {
    match kind {
        'A' | '?' => UiIcon::AddFile,
        'D' => UiIcon::DeleteFile,
        'R' => UiIcon::RenameFile,
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

fn view_uses_side_details(view: MainView) -> bool {
    matches!(view, MainView::Workspace)
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
        Command::new("cmd")
            .args(["/C", "start", "", "cmd", "/K"])
            .current_dir(root)
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

fn open_file_manager(root: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer").arg(root).spawn().map(|_| ())
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

#[cfg(test)]
mod ui_tests {
    use super::*;

    #[test]
    fn top_bar_has_two_fixed_rows() {
        assert_eq!(
            TOP_BAR_HEIGHT,
            TITLE_BAR_HEIGHT + MENU_BAR_HEIGHT + TOP_BAR_ROW_HEIGHT * 2.0
        );
        let source = include_str!("app.rs");
        assert!(source.contains("exact_height(self.top_bar_height())"));
        assert!(source.contains("if !self.repository_source_active()"));
        let tab_right_start = source
            .find("ui.allocate_new_ui(egui::UiBuilder::new().max_rect(tab_right)")
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
        assert_eq!(
            green.accent_shadow,
            Color32::from_rgba_unmultiplied(44, 56, 72, 54)
        );
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
            "sidebar_pct=0.20\ndetails_pct=0.31\nworkspace_list_pct=0.70\nworkspace_staged_pct=0.60\n",
        )
        .unwrap();
        assert!((prefs.sidebar_pct - 0.20).abs() < f32::EPSILON);
        assert!((prefs.details_pct - 0.31).abs() < f32::EPSILON);
        assert!((prefs.workspace_list_pct - 0.70).abs() < f32::EPSILON);
        assert!((prefs.workspace_staged_pct - 0.60).abs() < f32::EPSILON);
        assert_eq!(prefs.history_top_pct, 0.0);

        let inner = frame_inner_size(260.0, 300.0, LAYOUT_GAP, LAYOUT_GAP);
        assert!(inner.x < 260.0);
        assert!(inner.y < 300.0);

        let gap = Rect::from_min_max(Pos2::new(260.0, 0.0), Pos2::new(268.0, 600.0));
        let handle = resize_handle_rect(gap, true);
        assert_eq!(handle.width(), RESIZE_HANDLE_THICKNESS);
        assert_eq!(handle.height(), gap.height());
    }

    #[test]
    fn app_settings_are_json_and_dialog_window_has_no_outer_frame() {
        let settings = AppSettings {
            theme: SettingsThemeMode::Light,
            theme_accent: SettingsThemeAccent::Purple,
            language: SettingsLanguage::English,
        };
        let raw = serde_json::to_string(&settings).unwrap();
        assert!(raw.contains("\"theme\":\"Light\""));
        assert!(raw.contains("\"theme_accent\":\"Purple\""));
        assert!(raw.contains("\"language\":\"English\""));

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
    fn only_workspace_keeps_side_details_panel() {
        assert!(view_uses_side_details(MainView::Workspace));
        assert!(!view_uses_side_details(MainView::History));
        assert!(!view_uses_side_details(MainView::Search));
        assert!(!view_uses_side_details(MainView::Branches));
        assert!(!view_uses_side_details(MainView::Tags));
        assert!(!view_uses_side_details(MainView::Stashes));
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
        assert!(table_source.contains("ui.set_min_size(body_rect.size())"));
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
    fn unified_diff_uses_single_layer_code_review_colors() {
        let source = include_str!("app.rs");
        let diff_panel_start = source.find("fn diff_panel_frame(").unwrap();
        let graph_start = source[diff_panel_start..]
            .find("fn history_graph_width(")
            .unwrap();
        let diff_panel_source = &source[diff_panel_start..diff_panel_start + graph_start];
        assert!(diff_panel_source.contains("Color32::from_rgb(255, 255, 255)"));
        assert!(!diff_panel_source.contains("theme::accent_soft()"));
        let diff_frame_end = source[diff_panel_start..]
            .find("fn diff_display_mode_salt(")
            .unwrap();
        let diff_frame_source = &source[diff_panel_start..diff_panel_start + diff_frame_end];
        assert!(diff_frame_source.contains(".shadow(panel_shadow())"));
        assert!(!diff_frame_source.contains(".stroke("));

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
    fn plus_tab_opens_repository_source_page() {
        let source = include_str!("app.rs");
        assert!(source.contains("fn open_repository_source_tab("));
        assert!(source.contains("fn repository_source_view("));
        assert!(source.contains("self.open_repository_source_tab();"));
        assert!(source.contains("AppButtonStyle::RepoTab { selected: true } => Color32::WHITE"));
        assert!(source.contains("fn repo_tab_with_close("));
        assert!(source.contains("let mut close_source_tab = false;"));
        assert!(source.contains("Pos2::new(close_rect.left() - 2.0, rect.bottom())"));
        assert!(source.contains("fn close_repo_tab("));
        assert!(source.contains("fn save_repo_tabs("));

        let plus_start = source
            .find("UiIcon::Plus,\n                            self.tr(\"repo.source.new_tab\")")
            .unwrap();
        let plus_end = source[plus_start..]
            .find("ui.allocate_new_ui(egui::UiBuilder::new().max_rect(tab_right)")
            .unwrap();
        let plus_block = &source[plus_start..plus_start + plus_end];
        assert!(!plus_block.contains("pick_folder"));
        assert!(!plus_block.contains("RichText::new(\"\\u{00d7}\")"));
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
        assert!(TOOLBAR_BUTTON_MAX_WIDTH > 96.0);
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
    fn file_status_icons_are_plain_iconify_assets() {
        assert_eq!(file_status_icon('M'), UiIcon::Edit);
        assert_eq!(file_status_icon('A'), UiIcon::AddFile);
        assert_eq!(file_status_icon('?'), UiIcon::AddFile);
        assert_eq!(file_status_icon('D'), UiIcon::DeleteFile);
        assert_eq!(file_status_icon('R'), UiIcon::RenameFile);
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
        assert_eq!(file_status_color('M', true), Color32::WHITE);
        assert_eq!(file_status_color('A', true), Color32::WHITE);
        assert_eq!(file_status_color('D', true), Color32::WHITE);
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
            "\u{4ed3}\u{5e93}\u{8fdc}\u{7a0b}"
        );
    }
}
