use std::{
    collections::HashMap,
    env, fs,
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    thread,
};

use eframe::{
    App, CreationContext,
    egui::{
        self, Align, Align2, Color32, CornerRadius, FontId, Layout, Pos2, Rect, RichText,
        ScrollArea, Sense, Shape, Stroke, StrokeKind, TextEdit, Ui, Vec2, epaint::CubicBezierShape,
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
const TOP_BAR_GLOBAL_WIDTH: f32 = 260.0;
const TOP_BAR_MIN_TABS_WIDTH: f32 = 320.0;
const TOOLBAR_BUTTON_HEIGHT: f32 = 28.0;
const TOOLBAR_BUTTON_ICON: f32 = 13.0;
const TOOLBAR_BUTTON_TEXT: f32 = 11.0;
const TOOLBAR_BUTTON_X_PADDING: f32 = 36.0;
const TOOLBAR_BUTTON_MIN_WIDTH: f32 = 48.0;
const TOOLBAR_BUTTON_MAX_WIDTH: f32 = 160.0;
const FILE_ROW_HEIGHT: f32 = 24.0;
const FILE_ROW_ICON_SLOT: f32 = 24.0;
const FILE_ROW_LEFT_INSET: f32 = 10.0;
const HISTORY_DETAILS_MIN_HEIGHT: f32 = 260.0;
const HISTORY_LIST_MIN_HEIGHT: f32 = 260.0;
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
    snapshot: Option<RepositorySnapshot>,
    layout: GraphLayout,
    selected_commit: Option<usize>,
    error: Option<String>,
    search: String,
    search_dimension: SearchDimension,
    repo_task: Option<Receiver<anyhow::Result<RepositorySnapshot>>>,
    details_task: Option<Receiver<anyhow::Result<CommitDetails>>>,
    diff_task: Option<Receiver<anyhow::Result<FileDiff>>>,
    details_cache: HashMap<String, CommitDetails>,
    diff_cache: HashMap<String, FileDiff>,
    selected_file_path: Option<String>,
    selected_worktree_file: Option<SelectedWorktreeFile>,
    loading_repo: bool,
    loading_details_hash: Option<String>,
    loading_diff_key: Option<String>,
    pending_commit_action: Option<CommitActionDialog>,
    last_notice: Option<String>,
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
    settings_open: bool,
    settings_tab: SettingsTab,
    repo_settings_open: bool,
    repo_settings_tab: SettingsTab,
    theme_mode: theme::ThemeMode,
    layout_prefs: LayoutPrefs,
}

#[derive(Clone, Debug)]
struct RepoTab {
    root: PathBuf,
    name: String,
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
enum SearchDimension {
    Message,
    Files,
    Author,
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

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct LayoutPrefs {
    sidebar_pct: f32,
    details_pct: f32,
    workspace_list_pct: f32,
    workspace_staged_pct: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct AppSettings {
    theme: SettingsThemeMode,
    language: SettingsLanguage,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum SettingsThemeMode {
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum SettingsLanguage {
    English,
    Chinese,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: SettingsThemeMode::Dark,
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

impl Default for LayoutPrefs {
    fn default() -> Self {
        Self {
            sidebar_pct: 0.19,
            details_pct: 0.32,
            workspace_list_pct: 0.58,
            workspace_staged_pct: 0.5,
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

fn layout_prefs_path() -> Option<PathBuf> {
    app_data_dir().map(|base| base.join("layout.json"))
}

impl GitAgentApp {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let app_settings = AppSettings::load();

        let mut app = Self {
            repo_tabs: Vec::new(),
            active_repo_tab: None,
            snapshot: None,
            layout: GraphLayout::default(),
            selected_commit: None,
            error: None,
            search: String::new(),
            search_dimension: SearchDimension::Message,
            repo_task: None,
            details_task: None,
            diff_task: None,
            details_cache: HashMap::new(),
            diff_cache: HashMap::new(),
            selected_file_path: None,
            selected_worktree_file: None,
            loading_repo: false,
            loading_details_hash: None,
            loading_diff_key: None,
            pending_commit_action: None,
            last_notice: None,
            pending_worktree_action: None,
            commit_message: String::new(),
            language: app_settings.language.into(),
            pending_stash_action: None,
            pending_branch_action: None,
            pending_tag_action: None,
            active_view: MainView::Workspace,
            branches_open: true,
            tags_open: true,
            remotes_open: true,
            stashes_open: true,
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
            layout_prefs: LayoutPrefs::load(),
        };

        if let Ok(cwd) = env::current_dir() {
            app.load_repository(cwd);
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
            return;
        }

        self.repo_tabs.push(RepoTab { root: path, name });
        self.active_repo_tab = Some(self.repo_tabs.len() - 1);
    }

    fn switch_repo_tab(&mut self, index: usize) {
        if self.active_repo_tab == Some(index) {
            return;
        }
        if let Some(tab) = self.repo_tabs.get(index).cloned() {
            self.active_repo_tab = Some(index);
            self.load_repository(tab.root);
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
    }

    fn refresh(&mut self) {
        if let Some(root) = self.snapshot.as_ref().map(|snapshot| snapshot.root.clone()) {
            self.load_repository(root);
        }
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
                    self.layout = graph::layout(&snapshot.commits);
                    self.selected_commit = (!snapshot.commits.is_empty()).then_some(0);
                    self.sync_active_tab_with_snapshot(&snapshot);
                    self.snapshot = Some(snapshot);
                    self.details_cache.clear();
                    self.diff_cache.clear();
                    self.selected_file_path = None;
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

        if let Some(receiver) = self.details_task.take() {
            match receiver.try_recv() {
                Ok(Ok(details)) => {
                    let should_autoselect = self.selected_commit_hash()
                        == Some(details.hash.as_str())
                        && self.selected_file_path.is_none();
                    let first_file = details.files.first().map(|file| file.diff_path.clone());
                    self.loading_details_hash = None;
                    self.details_cache.insert(details.hash.clone(), details);
                    if should_autoselect {
                        self.selected_file_path = first_file;
                        self.request_selected_file_diff();
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
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let Some(commit) = self
            .selected_commit
            .and_then(|index| snapshot.commits.get(index))
        else {
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

    fn request_selected_file_diff(&mut self) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let Some(hash) = self.selected_commit_hash().map(str::to_owned) else {
            return;
        };
        let Some(path) = self.selected_file_path.clone() else {
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

    fn filtered_commit_indices(&self) -> Vec<usize> {
        let Some(snapshot) = &self.snapshot else {
            return Vec::new();
        };
        let query = self.search.trim().to_lowercase();
        if query.is_empty() {
            return (0..snapshot.commits.len()).collect();
        }

        snapshot
            .commits
            .iter()
            .enumerate()
            .filter_map(|(index, commit)| {
                let matches = match self.search_dimension {
                    SearchDimension::Message => {
                        commit.subject.to_lowercase().contains(&query)
                            || commit.hash.starts_with(&query)
                            || commit.short_hash.starts_with(&query)
                    }
                    SearchDimension::Files => {
                        self.details_cache.get(&commit.hash).is_some_and(|details| {
                            details
                                .files
                                .iter()
                                .any(|file| file.path.to_lowercase().contains(&query))
                        }) || commit.subject.to_lowercase().contains(&query)
                    }
                    SearchDimension::Author => commit.author.to_lowercase().contains(&query),
                };
                matches.then_some(index)
            })
            .collect()
    }

    fn tr(&self, key: &'static str) -> &'static str {
        i18n::t(self.language, key)
    }

    fn set_theme_mode(&mut self, mode: theme::ThemeMode) {
        if self.theme_mode != mode {
            self.theme_mode = mode;
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
            language: self.language.into(),
        }
        .save();
    }
}

impl App for GitAgentApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::apply(ctx, self.theme_mode);
        self.poll_tasks(ctx);
        if ctx.input(|input| input.modifiers.ctrl && input.key_pressed(egui::Key::Comma)) {
            self.settings_tab = SettingsTab::General;
            self.settings_open = true;
        }

        egui::TopBottomPanel::top("top_bar")
            .exact_height(TOP_BAR_HEIGHT)
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
    }
}

impl GitAgentApp {
    fn main_layout(&mut self, ui: &mut Ui) {
        let full = ui.available_rect_before_wrap();
        let height = full.height();
        let full_width = full.width();
        let gap = LAYOUT_GAP as f32;
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
        let clip_pad = gap / 2.0;

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
        let has_repo = self.snapshot.is_some();
        let has_remote = self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| !snapshot.remotes.is_empty());
        let mut switch_to = None;

        let full = ui.max_rect();
        let top_y = full.top();
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
            Pos2::new(full.right(), top_y + TOP_BAR_HEIGHT),
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
                            if repo_tab_button(ui, self.active_repo_tab == Some(index), &tab.name)
                                .clicked()
                            {
                                switch_to = Some(index);
                            }
                        }
                        if icon_button(ui, UiIcon::Plus, self.tr("action.open"), !self.loading_repo)
                            .clicked()
                        {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                self.load_repository(path);
                            }
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
                if toolbar_button(ui, "open", self.tr("action.open"), !self.loading_repo).clicked()
                {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.load_repository(path);
                    }
                }
                if toolbar_button(
                    ui,
                    "refresh",
                    self.tr("action.refresh"),
                    !self.loading_repo && has_repo,
                )
                .clicked()
                {
                    self.refresh();
                }
            });
        });

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(tool_row), |ui| {
            ScrollArea::horizontal()
                .id_salt("repo_toolbar_strip")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.add_space(16.0);
                        if toolbar_button(ui, "commit", self.tr("commit.panel"), true).clicked() {
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
                        if toolbar_button(ui, "branch", self.tr("branch.local"), has_repo).clicked()
                        {
                            self.active_view = MainView::Branches;
                        }
                        if toolbar_button(ui, "tag", self.tr("tag.title"), has_repo).clicked() {
                            self.active_view = MainView::Tags;
                        }
                        if toolbar_button(ui, "stash", self.tr("stash.title"), has_repo).clicked() {
                            self.active_view = MainView::Stashes;
                        }
                        if self.loading_repo {
                            ui.spinner();
                            ui.label(
                                RichText::new(self.tr("status.loading_repo")).color(theme::muted()),
                            );
                        }
                        if let Some(notice) = &self.last_notice {
                            ui.label(RichText::new(notice).color(theme::accent()));
                        }
                    });
                });
        });

        if let Some(index) = switch_to {
            self.switch_repo_tab(index);
        }
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

            let branch_create_label = self.tr("branch.create");
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
            if tags_visible {
                if snapshot.tags.is_empty() {
                    tree_empty(ui, self.tr("tag.none"));
                } else {
                    for tag in snapshot.tags.iter().take(12) {
                        tag_row(ui, tag, self.language, &mut tag_action);
                    }
                }
            }

            if tree_header(
                ui,
                &mut self.remotes_open,
                UiIcon::Folder,
                i18n::t(self.language, "remote.title"),
            ) {
                if snapshot.remotes.is_empty() {
                    tree_empty(ui, self.tr("remote.none"));
                } else {
                    for remote in snapshot.remotes.iter().take(8) {
                        remote_row(ui, &remote.name, &remote.fetch_url);
                    }
                }
            }

            if tree_header(
                ui,
                &mut self.stashes_open,
                UiIcon::Stash,
                i18n::t(self.language, "stash.title"),
            ) {
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
            self.request_selected_worktree_diff();
        }
    }

    fn search_view(&mut self, ui: &mut Ui) {
        if self.snapshot.is_none() {
            empty_state(ui, self.loading_repo, self.language);
            return;
        }

        let filtered_indices = self.filtered_commit_indices();
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
            content_panel_frame(theme::bg()).show(ui, |ui| {
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
                ui.horizontal(|ui| {
                    let search_hint = self.tr("commit.search");
                    let response = ui.add_sized(
                        [(ui.available_width() - 132.0).max(180.0), 30.0],
                        TextEdit::singleline(&mut self.search).hint_text(search_hint),
                    );
                    let before_dimension = self.search_dimension;
                    egui::ComboBox::from_id_salt("search_dimension")
                        .selected_text(search_dimension_label(self.language, self.search_dimension))
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.search_dimension,
                                SearchDimension::Message,
                                search_dimension_label(self.language, SearchDimension::Message),
                            );
                            ui.selectable_value(
                                &mut self.search_dimension,
                                SearchDimension::Files,
                                search_dimension_label(self.language, SearchDimension::Files),
                            );
                            ui.selectable_value(
                                &mut self.search_dimension,
                                SearchDimension::Author,
                                search_dimension_label(self.language, SearchDimension::Author),
                            );
                        });
                    if response.changed() || self.search_dimension != before_dimension {
                        self.selected_commit = rows.first().map(|(index, _)| *index);
                        should_request_details = true;
                    }
                });

                ui.add_space(10.0);
                if rows.is_empty() {
                    empty_list_panel(ui, self.tr("commit.no_matches"));
                    return;
                }

                search_table_header(ui, self.language);
                let mut clicked_commit = None;
                ScrollArea::vertical()
                    .id_salt("search_results_scroll")
                    .auto_shrink([false, false])
                    .show_rows(ui, 30.0, rows.len(), |ui, range| {
                        for row_index in range {
                            let (commit_index, commit) = &rows[row_index];
                            if search_commit_row(
                                ui,
                                commit,
                                self.selected_commit == Some(*commit_index),
                            )
                            .clicked()
                            {
                                clicked_commit = Some(*commit_index);
                            }
                        }
                    });
                if let Some(index) = clicked_commit {
                    self.selected_commit = Some(index);
                    should_request_details = true;
                }
            });
        });
        ui.add_space(LAYOUT_GAP as f32);
        content_panel_frame(theme::panel()).show(ui, |ui| {
            ScrollArea::vertical()
                .id_salt("search_details_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| self.commit_details_only(ui));
        });

        if should_request_details {
            self.request_selected_details();
        }
    }

    fn commit_graph(&mut self, ui: &mut Ui, search_mode: bool) {
        if self.snapshot.is_none() {
            empty_state(ui, self.loading_repo, self.language);
            return;
        };

        let row_height = 58.0;
        let graph_width = 28.0 + self.layout.lanes.max(1) as f32 * 22.0;
        let filtered_indices = self.filtered_commit_indices();
        let commit_count = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.commits.len())
            .unwrap_or_default();
        let visible_rows = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                filtered_indices
                    .iter()
                    .filter_map(|index| {
                        snapshot.commits.get(*index).map(|commit| {
                            (
                                *index,
                                commit.clone(),
                                self.layout.rows.get(*index).cloned(),
                            )
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut should_request_details = false;

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new(if search_mode {
                    self.tr("commit.search")
                } else {
                    self.tr("nav.history")
                })
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
                    self.layout.lanes.max(1),
                    self.tr("commit.stats_lanes")
                ))
                .color(theme::muted()),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!(
                    "{} {}",
                    visible_rows.len(),
                    self.tr("commit.stats_visible")
                ))
                .color(theme::muted()),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(14.0);
                let search_hint = self.tr("commit.search");
                let response = ui.add_sized(
                    [if search_mode { 360.0 } else { 260.0 }, 30.0],
                    TextEdit::singleline(&mut self.search).hint_text(search_hint),
                );
                if response.changed() {
                    self.selected_commit = visible_rows.first().map(|(index, _, _)| *index);
                    should_request_details = true;
                }
            });
        });

        if should_request_details {
            self.request_selected_details();
        }

        if search_mode {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                let mut changed = false;
                egui::Frame::new()
                    .fill(theme::panel_soft())
                    .stroke(Stroke::new(1.0, Color32::from_rgb(48, 56, 72)))
                    .corner_radius(CornerRadius::same(5))
                    .inner_margin(egui::Margin::symmetric(10, 4))
                    .show(ui, |ui| {
                        let search_hint = self.tr("commit.search");
                        let response = ui.add_sized(
                            [ui.available_width().min(500.0), 26.0],
                            TextEdit::singleline(&mut self.search).hint_text(search_hint),
                        );
                        changed = response.changed();
                    });
                if changed {
                    self.selected_commit = visible_rows.first().map(|(index, _, _)| *index);
                    self.request_selected_details();
                }
            });
        }

        ui.add_space(8.0);

        if commit_count == 0 {
            no_commits_state(ui, self.language);
            return;
        }

        if visible_rows.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new(self.tr("commit.no_matches")).color(theme::muted()));
            });
            return;
        }

        let mut clicked_commit = None;
        let mut menu_action = None;
        ScrollArea::vertical()
            .id_salt(if search_mode {
                "search_commit_graph_scroll"
            } else {
                "history_commit_graph_scroll"
            })
            .auto_shrink([false, false])
            .show_rows(ui, row_height, visible_rows.len(), |ui, row_range| {
                for row_index in row_range {
                    let (commit_index, commit, row) = &visible_rows[row_index];
                    let is_selected = self.selected_commit == Some(*commit_index);
                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), row_height),
                        Sense::click(),
                    );

                    if response.clicked() {
                        clicked_commit = Some(*commit_index);
                    }
                    response.context_menu(|ui| {
                        menu_action = commit_context_menu(ui, commit, self.language);
                    });

                    draw_commit_row(
                        ui,
                        rect,
                        commit,
                        row.as_ref(),
                        row_height,
                        graph_width,
                        is_selected,
                    );
                }
            });

        if let Some(commit_index) = clicked_commit {
            self.selected_commit = Some(commit_index);
            self.selected_file_path = None;
            self.request_selected_details();
        }

        if let Some(action) = menu_action {
            self.handle_commit_menu_action(action);
        }
    }

    fn history_view(&mut self, ui: &mut Ui) {
        if self.snapshot.is_none() {
            empty_state(ui, self.loading_repo, self.language);
            return;
        }

        let available = ui.available_size();
        let (list_height, _details_height) = master_detail_split_heights(available.y);

        ui.allocate_ui(Vec2::new(available.x, list_height), |ui| {
            self.commit_graph(ui, false);
        });
        ui.add_space(LAYOUT_GAP as f32);
        content_panel_frame(theme::panel()).show(ui, |ui| {
            ScrollArea::vertical()
                .id_salt("history_details_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| self.commit_details_only(ui));
        });
    }

    fn commit_details_only(&mut self, ui: &mut Ui) {
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
                    .size(18.0)
                    .strong()
                    .color(theme::text()),
            );
            ui.add_space(10.0);
            detail_line(ui, self.tr("commit.hash"), &commit.hash);
            detail_line(ui, self.tr("commit.author"), &commit.author);
            detail_line(ui, self.tr("commit.when"), &commit.relative_time);
            detail_line(
                ui,
                self.tr("commit.parents"),
                &commit.parents.len().to_string(),
            );
            ui.add_space(14.0);
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
                        .id_salt("commit_details_files_scroll")
                        .max_height(132.0)
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
                        self.selected_file_path = Some(path);
                        self.selected_worktree_file = None;
                        self.request_selected_file_diff();
                    }
                }
            } else {
                ui.label(
                    RichText::new(self.tr("commit.select_to_load_files")).color(theme::muted()),
                );
            }

            ui.add_space(12.0);
            panel_heading_inline(ui, self.tr("commit.diff"));
            ui.add_space(6.0);
            self.diff_viewer(ui, &commit.hash);
        } else {
            ui.label(RichText::new(self.tr("commit.none")).color(theme::muted()));
        }
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
                        self.selected_file_path = Some(path);
                        self.selected_worktree_file = None;
                        self.request_selected_file_diff();
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
            self.diff_viewer(ui, &commit.hash);
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

    fn diff_viewer(&self, ui: &mut Ui, hash: &str) {
        let Some(path) = &self.selected_file_path else {
            ui.label(RichText::new(self.tr("commit.select_file")).color(theme::muted()));
            return;
        };
        let key = git::diff_key(hash, path);

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

        soft_panel_frame(theme::accent_soft(), 8, 8).show(ui, |ui| {
            ScrollArea::both()
                .id_salt(("commit_diff_scroll", hash, path))
                .max_height(360.0)
                .show(ui, |ui| {
                    render_unified_diff(ui, &diff.text);
                    if diff.text.lines().count() > 1_200 {
                        ui.label(RichText::new(self.tr("diff.truncated")).color(theme::muted()));
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

        soft_panel_frame(theme::accent_soft(), 8, 8).show(ui, |ui| {
            ScrollArea::both()
                .id_salt(("worktree_diff_scroll", key))
                .max_height(360.0)
                .show(ui, |ui| {
                    render_unified_diff(ui, &diff.text);
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
                egui::Window::new(self.tr("branch.checkout_remote"))
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.set_min_width(380.0);
                        ui.label(RichText::new(remote_branch.as_str()).color(theme::text()));
                        ui.label(
                            RichText::new(self.tr("branch.name"))
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
            if ui
                .selectable_label(
                    self.theme_mode == theme::ThemeMode::Light,
                    menu_label(self.language, "light_theme"),
                )
                .clicked()
            {
                self.set_theme_mode(theme::ThemeMode::Light);
            }
            if ui
                .selectable_label(
                    self.theme_mode == theme::ThemeMode::Dark,
                    menu_label(self.language, "dark_theme"),
                )
                .clicked()
            {
                self.set_theme_mode(theme::ThemeMode::Dark);
            }
        });
        ui.add_space(8.0);
        settings_field(ui, "Language", |ui| {
            if ui
                .selectable_label(self.language == Language::Chinese, "\u{4e2d}\u{6587}")
                .clicked()
            {
                self.set_language(Language::Chinese);
            }
            if ui
                .selectable_label(self.language == Language::English, "English")
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
}

#[derive(Clone, Debug)]
enum TagMenuAction {
    Create,
    Checkout { name: String },
    Delete { name: String },
}

fn draw_commit_row(
    ui: &mut Ui,
    rect: Rect,
    commit: &Commit,
    row: Option<&graph::GraphRow>,
    row_height: f32,
    graph_width: f32,
    is_selected: bool,
) {
    let painter = ui.painter();
    let bg = if is_selected {
        theme::accent_deep()
    } else if ui.is_rect_visible(rect) {
        theme::bg()
    } else {
        Color32::TRANSPARENT
    };
    painter.rect_filled(
        rect.shrink2(Vec2::new(10.0, 3.0)),
        CornerRadius::same(6),
        bg,
    );

    if let Some(row) = row {
        let top = rect.top();
        let center_y = rect.center().y;
        let bottom = rect.bottom();
        let lane_x = |lane: usize| rect.left() + 24.0 + lane as f32 * 22.0;

        for edge in &row.edges {
            let color = theme::LANES[edge.to_lane % theme::LANES.len()];
            let from = Pos2::new(lane_x(edge.from_lane), center_y);
            let to = Pos2::new(lane_x(edge.to_lane), bottom + row_height * 0.5);
            let mid_y = center_y + row_height * 0.45;
            let control_a = Pos2::new(from.x, mid_y);
            let control_b = Pos2::new(to.x, mid_y);
            let stroke = Stroke::new(
                if edge.kind == EdgeKind::Continue {
                    2.0
                } else {
                    1.6
                },
                color,
            );
            painter.add(Shape::CubicBezier(CubicBezierShape::from_points_stroke(
                [from, control_a, control_b, to],
                false,
                Color32::TRANSPARENT,
                stroke,
            )));
        }

        let node = Pos2::new(lane_x(row.lane), center_y);
        let color = theme::LANES[row.lane % theme::LANES.len()];
        painter.circle_filled(node, 6.0, color);
        painter.circle_stroke(node, 8.0, Stroke::new(1.0, Color32::from_rgb(11, 13, 18)));

        for lane in 0..row.lane {
            let x = lane_x(lane);
            painter.line_segment(
                [Pos2::new(x, top), Pos2::new(x, bottom)],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(120, 130, 148, 65)),
            );
        }
    }

    let text_left = rect.left() + graph_width + 10.0;
    let title_pos = Pos2::new(text_left, rect.top() + 10.0);
    let meta_pos = Pos2::new(text_left, rect.top() + 34.0);
    painter.text(
        title_pos,
        Align2::LEFT_TOP,
        &commit.subject,
        FontId::proportional(15.0),
        if is_selected {
            Color32::WHITE
        } else {
            theme::text()
        },
    );
    painter.text(
        meta_pos,
        Align2::LEFT_TOP,
        format!(
            "{}  {}  {}",
            commit.short_hash, commit.author, commit.relative_time
        ),
        FontId::monospace(12.0),
        if is_selected {
            Color32::from_rgb(222, 247, 244)
        } else {
            theme::muted()
        },
    );
    painter.rect_stroke(
        Rect::from_min_max(
            Pos2::new(rect.left() + 10.0, rect.bottom() - 1.0),
            rect.right_top(),
        ),
        CornerRadius::ZERO,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 14)),
        StrokeKind::Inside,
    );
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
        offset: [0, 3],
        blur: 14,
        spread: 0,
        color: theme::accent_shadow(),
    }
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
                AppButtonStyle::RepoTab { .. } => theme::accent(),
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
    let width = ui.available_width();
    let date_w = 132.0;
    let author_w = 230.0;
    let hash_w = 92.0;
    let desc_w = (width - date_w - author_w - hash_w).max(180.0);
    ui.horizontal(|ui| {
        ui.add_sized(
            [desc_w, 24.0],
            egui::Label::new(RichText::new(labels.0).color(theme::muted())),
        );
        ui.add_sized(
            [date_w, 24.0],
            egui::Label::new(RichText::new(labels.1).color(theme::muted())),
        );
        ui.add_sized(
            [author_w, 24.0],
            egui::Label::new(RichText::new(labels.2).color(theme::muted())),
        );
        ui.add_sized(
            [hash_w, 24.0],
            egui::Label::new(RichText::new(labels.3).color(theme::muted())),
        );
    });
    ui.add_space(6.0);
}

fn search_commit_row(ui: &mut Ui, commit: &Commit, selected: bool) -> egui::Response {
    let response = ui.allocate_response(Vec2::new(ui.available_width(), 30.0), Sense::click());
    let rect = response.rect;
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(1.0, 1.0)),
            CornerRadius::same(3),
            if selected {
                theme::accent_deep()
            } else {
                theme::accent_soft()
            },
        );
    }

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        let width = ui.available_width();
        let date_w = 132.0;
        let author_w = 230.0;
        let hash_w = 92.0;
        let desc_w = (width - date_w - author_w - hash_w).max(180.0);
        ui.horizontal(|ui| {
            ui.add_sized(
                [desc_w, 26.0],
                egui::Label::new(
                    RichText::new(&commit.subject)
                        .color(if selected { Color32::WHITE } else { theme::text() }),
                )
                .truncate(),
            );
            ui.add_sized(
                [date_w, 26.0],
                egui::Label::new(
                    RichText::new(&commit.relative_time)
                        .color(if selected { Color32::WHITE } else { theme::text() }),
                )
                .truncate(),
            );
            ui.add_sized(
                [author_w, 26.0],
                egui::Label::new(
                    RichText::new(&commit.author)
                        .color(if selected { Color32::WHITE } else { theme::text() }),
                )
                .truncate(),
            );
            ui.add_sized(
                [hash_w, 26.0],
                egui::Label::new(
                    RichText::new(&commit.short_hash)
                        .monospace()
                        .color(if selected {
                            Color32::from_rgb(222, 247, 244)
                        } else {
                            theme::muted()
                        }),
                )
                .truncate(),
            );
        });
    });
    response
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
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        let clipped = rect.shrink2(Vec2::new(4.0, 4.0));
        ui.set_clip_rect(clipped);
        ui.vertical_centered(|ui| {
            ui.add_space(7.0);
            let image = egui::Image::new(icon_source(icon))
                .fit_to_exact_size(Vec2::splat(18.0))
                .tint(if selected {
                    Color32::WHITE
                } else {
                    theme::accent()
                });
            ui.add(image);
            ui.add_space(3.0);
            ui.add_sized(
                [width - 8.0, 18.0],
                egui::Label::new(RichText::new(label).size(11.0).color(if selected {
                    Color32::WHITE
                } else {
                    theme::text()
                }))
                .truncate(),
            );
        });
    });
    response.on_hover_text(label)
}

fn repo_tab_button(ui: &mut Ui, selected: bool, label: &str) -> egui::Response {
    AppButton::repo_tab(UiIcon::Folder, label, selected).show(ui)
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

fn remote_row(ui: &mut Ui, name: &str, url: &str) {
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.label(RichText::new(name).strong().color(theme::text()));
    });
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.label(RichText::new(url).small().color(theme::muted()));
    });
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

    draw_file_row_content(ui, rect, FILE_ROW_LEFT_INSET, &status, &file.display_path, false);

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
    draw_file_status_icon(ui, icon_rect, status);

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
                egui::Label::new(
                    RichText::new(path)
                        .monospace()
                        .color(if selected { Color32::WHITE } else { theme::text() }),
                )
                .truncate(),
            );
        });
    });
}

fn draw_file_status_icon(ui: &mut Ui, rect: Rect, status: &str) {
    let kind = status.chars().next().unwrap_or('M');
    let color = match kind {
        'A' | '?' => theme::info(),
        'D' => Color32::from_rgb(244, 113, 116),
        'R' => theme::info(),
        _ => theme::warning(),
    };
    let icon = file_status_icon(kind);
    draw_ui_icon(
        ui,
        Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
        icon,
        color,
    );
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

fn render_unified_diff(ui: &mut Ui, text: &str) {
    let mut old_line: Option<usize> = None;
    let mut new_line: Option<usize> = None;

    for line in text.lines().take(1_200) {
        if line.starts_with("diff --git")
            || line.starts_with("index ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
        {
            if line.starts_with("diff --git") {
                ui.label(
                    RichText::new(clean_diff_header(line))
                        .monospace()
                        .color(theme::info()),
                );
            }
            continue;
        }

        if line.starts_with("@@") {
            let (old_start, new_start) = parse_hunk_header(line);
            old_line = old_start;
            new_line = new_start;
            ui.add_space(6.0);
            ui.label(
                RichText::new(line)
                    .monospace()
                    .color(theme::info()),
            );
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

        diff_row(ui, &left_no, &right_no, body, kind);

        match kind {
            DiffKind::Added => new_line = new_line.map(|line| line + 1),
            DiffKind::Removed => old_line = old_line.map(|line| line + 1),
            DiffKind::Context => {
                old_line = old_line.map(|line| line + 1);
                new_line = new_line.map(|line| line + 1);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum DiffKind {
    Added,
    Removed,
    Context,
}

fn diff_row(ui: &mut Ui, left_no: &str, right_no: &str, body: &str, kind: DiffKind) {
    let fill = match kind {
        DiffKind::Added => Color32::from_rgba_unmultiplied(55, 135, 75, 70),
        DiffKind::Removed => Color32::from_rgba_unmultiplied(150, 60, 65, 70),
        DiffKind::Context => Color32::TRANSPARENT,
    };
    let text_color = match kind {
        DiffKind::Added => Color32::from_rgb(170, 235, 180),
        DiffKind::Removed => Color32::from_rgb(255, 170, 175),
        DiffKind::Context => theme::muted(),
    };
    let width = ui.available_width().max(560.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 21.0), Sense::hover());
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
    }
    let gutter = Color32::from_rgb(85, 95, 112);
    ui.painter().text(
        Pos2::new(rect.left() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        left_no,
        FontId::monospace(12.0),
        gutter,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 48.0, rect.center().y),
        Align2::LEFT_CENTER,
        right_no,
        FontId::monospace(12.0),
        gutter,
    );
    let sign = match kind {
        DiffKind::Added => "+",
        DiffKind::Removed => "-",
        DiffKind::Context => " ",
    };
    ui.painter().text(
        Pos2::new(rect.left() + 88.0, rect.center().y),
        Align2::LEFT_CENTER,
        sign,
        FontId::monospace(12.0),
        text_color,
    );
    let text_rect = Rect::from_min_max(
        Pos2::new(rect.left() + 108.0, rect.top()),
        Pos2::new(rect.right() - 6.0, rect.bottom()),
    );
    ui.painter().with_clip_rect(text_rect).text(
        text_rect.left_center(),
        Align2::LEFT_CENTER,
        body,
        FontId::monospace(12.0),
        text_color,
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

#[cfg(test)]
mod ui_tests {
    use super::*;

    #[test]
    fn top_bar_has_two_fixed_rows() {
        assert_eq!(
            TOP_BAR_HEIGHT,
            TITLE_BAR_HEIGHT + MENU_BAR_HEIGHT + TOP_BAR_ROW_HEIGHT * 2.0
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
        assert!(shadow.blur >= 12);
        assert!(shadow.color.a() > 0);
        assert!(shadow.color.g() > shadow.color.r());
        assert!(shadow.color.b() > shadow.color.r());
        let source = include_str!("app.rs");
        assert!(source.contains("theme::accent_deep()"));
        assert!(source.contains("theme::accent_soft()"));
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
            language: SettingsLanguage::English,
        };
        let raw = serde_json::to_string(&settings).unwrap();
        assert!(raw.contains("\"theme\":\"Light\""));
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
        let end = source.find("fn repo_tab_button(").unwrap();
        let sidebar_nav_card_source = &source[start..end];
        assert!(!sidebar_nav_card_source.contains("painter().with_clip_rect"));
        assert!(!sidebar_nav_card_source.contains(".text("));
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
    fn history_scroll_areas_have_distinct_ids() {
        let source = include_str!("app.rs");
        assert!(source.contains("history_commit_graph_scroll"));
        assert!(source.contains("history_details_scroll"));
        assert!(source.contains("search_results_scroll"));
        assert!(source.contains("search_details_scroll"));
    }

    #[test]
    fn toolbar_icons_use_raw_actions() {
        assert_eq!(toolbar_icon("commit", ""), UiIcon::Commit);
        assert_eq!(toolbar_icon("pull", ""), UiIcon::Pull);
        assert_eq!(toolbar_icon("branch", ""), UiIcon::Branch);
        assert_eq!(toolbar_icon("tag", ""), UiIcon::Tag);
        assert_eq!(toolbar_icon("stash", ""), UiIcon::Stash);
        assert_eq!(toolbar_icon("+", ""), UiIcon::Plus);
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
    }

    #[test]
    fn plus_icon_uses_crisp_16px_asset() {
        let plus = include_str!("../assets/icons/plus.svg");
        let add_file = include_str!("../assets/icons/add-file.svg");
        assert!(plus.contains("viewBox=\"0 0 16 16\""));
        assert!(add_file.contains("viewBox=\"0 0 16 16\""));
        assert!(!add_file.contains("V7l-5-5"));
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
