use std::{
    collections::HashMap,
    env,
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

use crate::{
    git::{
        self, Commit, CommitDetails, FileDiff, RepositorySnapshot, ResetMode, StashEntry, Tag,
        WorktreeFile,
    },
    graph::{self, EdgeKind, GraphLayout},
    i18n::{self, Language},
    theme,
};

pub struct GitAgentApp {
    snapshot: Option<RepositorySnapshot>,
    layout: GraphLayout,
    selected_commit: Option<usize>,
    error: Option<String>,
    search: String,
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MainView {
    Workspace,
    History,
    Search,
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

impl GitAgentApp {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);

        let mut app = Self {
            snapshot: None,
            layout: GraphLayout::default(),
            selected_commit: None,
            error: None,
            search: String::new(),
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
            language: Language::Chinese,
            pending_stash_action: None,
            pending_branch_action: None,
            pending_tag_action: None,
            active_view: MainView::Workspace,
            branches_open: true,
            tags_open: true,
            remotes_open: true,
            stashes_open: true,
        };

        if let Ok(cwd) = env::current_dir() {
            app.load_repository(cwd);
        }

        app
    }

    fn load_repository(&mut self, path: PathBuf) {
        let (sender, receiver) = mpsc::channel();
        self.repo_task = Some(receiver);
        self.loading_repo = true;
        self.error = None;

        thread::spawn(move || {
            let _ = sender.send(git::open_repository(path));
        });
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
                let matches = commit.subject.to_lowercase().contains(&query)
                    || commit.author.to_lowercase().contains(&query)
                    || commit.hash.starts_with(&query)
                    || commit.short_hash.starts_with(&query);
                matches.then_some(index)
            })
            .collect()
    }

    fn tr(&self, key: &'static str) -> &'static str {
        i18n::t(self.language, key)
    }
}

impl App for GitAgentApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::apply(ctx);
        self.poll_tasks(ctx);

        egui::TopBottomPanel::top("top_bar")
            .exact_height(82.0)
            .frame(egui::Frame::new().fill(theme::BG))
            .show(ctx, |ui| self.top_bar(ui));

        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(210.0)
            .frame(egui::Frame::new().fill(theme::PANEL))
            .show(ctx, |ui| self.sidebar(ui));

        egui::SidePanel::right("details")
            .resizable(true)
            .default_width(340.0)
            .width_range(280.0..=460.0)
            .frame(egui::Frame::new().fill(theme::PANEL))
            .show(ctx, |ui| self.details(ui));

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BG))
            .show(ctx, |ui| match self.active_view {
                MainView::Workspace => self.workspace_view(ui),
                MainView::History | MainView::Search => self.commit_graph(ui),
            });

        self.commit_action_modal(ctx);
        self.worktree_action_modal(ctx);
        self.stash_action_modal(ctx);
        self.branch_action_modal(ctx);
        self.tag_action_modal(ctx);
    }
}

impl GitAgentApp {
    fn top_bar(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            ui.add_space(10.0);
            if toolbar_button(ui, "+", self.tr("commit.panel"), true).clicked() {
                self.active_view = MainView::Workspace;
            }
            let has_repo = self.snapshot.is_some();
            let has_remote = self
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| !snapshot.remotes.is_empty());
            if toolbar_button(
                ui,
                "↓",
                self.tr("action.pull"),
                !self.loading_repo && has_repo && has_remote,
            )
            .clicked()
            {
                self.pull_current();
            }
            if toolbar_button(
                ui,
                "↑",
                self.tr("action.push"),
                !self.loading_repo && has_repo && has_remote,
            )
            .clicked()
            {
                self.push_current();
            }
            if toolbar_button(
                ui,
                "↻",
                self.tr("action.fetch"),
                !self.loading_repo && has_repo && has_remote,
            )
            .clicked()
            {
                self.fetch_all();
            }
            ui.separator();
            if toolbar_button(ui, "⑂", self.tr("branch.local"), has_repo).clicked() {
                self.active_view = MainView::History;
            }
            if toolbar_button(ui, "◇", self.tr("tag.title"), has_repo).clicked() {
                self.tags_open = true;
            }
            if toolbar_button(ui, "▦", self.tr("stash.title"), has_repo).clicked() {
                self.stashes_open = true;
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(12.0);
                if toolbar_button(ui, "…", self.tr("action.open"), !self.loading_repo).clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.load_repository(path);
                    }
                }
                if toolbar_button(
                    ui,
                    "⟳",
                    self.tr("action.refresh"),
                    !self.loading_repo && self.snapshot.is_some(),
                )
                .clicked()
                {
                    self.refresh();
                }
                if ui.button(self.language.code()).clicked() {
                    self.language = self.language.next();
                }
                if self.loading_repo {
                    ui.spinner();
                    ui.label(RichText::new(self.tr("status.loading_repo")).color(theme::MUTED));
                }
                if let Some(notice) = &self.last_notice {
                    ui.label(RichText::new(notice).color(theme::ACCENT));
                }
            });
        });
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        ScrollArea::vertical()
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
                    .color(theme::TEXT),
            );
        });
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            if let Some(snapshot) = &self.snapshot {
                ui.label(
                    RichText::new(snapshot.root.display().to_string())
                        .small()
                        .color(theme::MUTED),
                );
            } else {
                ui.label(RichText::new(self.tr("repo.none")).color(theme::MUTED));
            }
        });

        ui.add_space(10.0);
        if sidebar_nav_item(
            ui,
            self.active_view == MainView::Workspace,
            "▣",
            self.tr("worktree.title"),
        )
        .clicked()
        {
            self.active_view = MainView::Workspace;
        }
        if sidebar_nav_item(
            ui,
            self.active_view == MainView::History,
            "≋",
            self.tr("nav.history"),
        )
        .clicked()
        {
            self.active_view = MainView::History;
        }
        if sidebar_nav_item(
            ui,
            self.active_view == MainView::Search,
            "⌕",
            self.tr("commit.search"),
        )
        .clicked()
        {
            self.active_view = MainView::Search;
        }

        ui.add_space(8.0);
        ui.separator();

        if let Some(snapshot) = &self.snapshot {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new("*").color(theme::ACCENT));
                ui.label(RichText::new(&snapshot.branch).strong().color(theme::TEXT));
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
                        .color(theme::MUTED),
                    );
                });
            }
            ui.add_space(8.0);

            let mut branch_action = None;
            let mut tag_action = None;
            let mut stash_action = None;

            if tree_header(
                ui,
                &mut self.branches_open,
                "⑂",
                i18n::t(self.language, "branch.local"),
            ) {
                ui.horizontal(|ui| {
                    ui.add_space(26.0);
                    if ui.small_button(self.tr("branch.create")).clicked() {
                        branch_action = Some(BranchMenuAction::Create);
                    }
                });
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

            if tree_header(
                ui,
                &mut self.tags_open,
                "◇",
                i18n::t(self.language, "tag.title"),
            ) {
                ui.horizontal(|ui| {
                    ui.add_space(26.0);
                    if ui.small_button(self.tr("tag.create")).clicked() {
                        tag_action = Some(TagMenuAction::Create);
                    }
                });
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
                "☁",
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
                "▦",
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
            ui.colored_label(theme::WARNING, error);
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
                    .color(theme::TEXT),
            );
            ui.label(
                RichText::new(format!("{status_count}"))
                    .small()
                    .color(theme::MUTED),
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
        ui.separator();

        let list_height = (ui.available_height() - 130.0).max(220.0);
        ui.allocate_ui(Vec2::new(ui.available_width(), list_height), |ui| {
            if status_count == 0 {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new(self.tr("worktree.clean")).color(theme::MUTED));
                });
            } else {
                let table_height = ((ui.available_height() - 12.0) / 2.0).max(120.0);
                worktree_table(
                    ui,
                    self.tr("worktree.staged"),
                    &staged,
                    true,
                    table_height,
                    self.language,
                    &mut worktree_action,
                    &mut selected_worktree_after_draw,
                );
                ui.add_space(10.0);
                worktree_table(
                    ui,
                    self.tr("worktree.unstaged"),
                    &unstaged,
                    false,
                    table_height,
                    self.language,
                    &mut worktree_action,
                    &mut selected_worktree_after_draw,
                );
            }
        });

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

    fn commit_graph(&mut self, ui: &mut Ui) {
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

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.label(
                RichText::new(format!(
                    "{} {}",
                    commit_count,
                    self.tr("commit.stats_loaded")
                ))
                .color(theme::MUTED),
            );
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "{} {}",
                    self.layout.lanes.max(1),
                    self.tr("commit.stats_lanes")
                ))
                .color(theme::MUTED),
            );
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "{} {}",
                    visible_rows.len(),
                    self.tr("commit.stats_visible")
                ))
                .color(theme::MUTED),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(16.0);
                let search_hint = self.tr("commit.search");
                let response = ui.add_sized(
                    [260.0, 30.0],
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

        ui.add_space(8.0);

        if commit_count == 0 {
            no_commits_state(ui, self.language);
            return;
        }

        if visible_rows.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new(self.tr("commit.no_matches")).color(theme::MUTED));
            });
            return;
        }

        let mut clicked_commit = None;
        let mut menu_action = None;
        ScrollArea::vertical()
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
        ui.add_space(18.0);
        panel_heading(ui, self.tr("commit.details"));
        ui.add_space(8.0);

        let Some(snapshot) = &self.snapshot else {
            ui.label(RichText::new(self.tr("repo.none")).color(theme::MUTED));
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
                    .color(theme::TEXT),
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
                    ui.label(RichText::new(self.tr("commit.loading_files")).color(theme::MUTED));
                });
            } else if let Some(details) = self.details_cache.get(&commit.hash) {
                if details.files.is_empty() {
                    ui.label(RichText::new(self.tr("commit.no_changes")).color(theme::MUTED));
                } else {
                    let mut clicked_file = None;
                    ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                        for file in &details.files {
                            let selected =
                                self.selected_file_path.as_deref() == Some(file.diff_path.as_str());
                            if file_change_row(ui, &file.status, &file.path, selected).clicked() {
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
                ui.label(RichText::new(self.tr("commit.select_to_load_files")).color(theme::MUTED));
            }

            ui.add_space(14.0);
            panel_heading_inline(ui, self.tr("commit.diff"));
            ui.add_space(6.0);
            self.diff_viewer(ui, &commit.hash);
        } else {
            ui.label(RichText::new(self.tr("commit.none")).color(theme::MUTED));
        }

        ui.add_space(18.0);
        self.worktree_diff_viewer(ui);
    }

    fn commit_panel(&mut self, ui: &mut Ui, staged_count: usize) {
        ui.separator();
        ui.add_space(10.0);
        panel_heading_inline(ui, self.tr("commit.panel"));
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!("{staged_count} {}", self.tr("commit.staged_files")))
                .small()
                .color(theme::MUTED),
        );
        let message_hint = self.tr("commit.message");
        ui.add_sized(
            [ui.available_width(), 78.0],
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
    }

    fn diff_viewer(&self, ui: &mut Ui, hash: &str) {
        let Some(path) = &self.selected_file_path else {
            ui.label(RichText::new(self.tr("commit.select_file")).color(theme::MUTED));
            return;
        };
        let key = git::diff_key(hash, path);

        if self.loading_diff_key.as_deref() == Some(key.as_str()) {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(self.tr("diff.loading")).color(theme::MUTED));
            });
            return;
        }

        let Some(diff) = self.diff_cache.get(&key) else {
            ui.label(RichText::new(self.tr("diff.queued")).color(theme::MUTED));
            return;
        };

        if diff.text.trim().is_empty() {
            ui.label(RichText::new(self.tr("diff.empty")).color(theme::MUTED));
            return;
        }

        egui::Frame::new()
            .fill(Color32::from_rgb(14, 16, 21))
            .stroke(Stroke::new(1.0, Color32::from_rgb(45, 52, 66)))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(egui::Margin::symmetric(8, 8))
            .show(ui, |ui| {
                ScrollArea::both().max_height(360.0).show(ui, |ui| {
                    for line in diff.text.lines().take(1_200) {
                        diff_line(ui, line);
                    }
                    if diff.text.lines().count() > 1_200 {
                        ui.label(RichText::new(self.tr("diff.truncated")).color(theme::MUTED));
                    }
                });
            });
    }

    fn worktree_diff_viewer(&self, ui: &mut Ui) {
        let Some(selected) = &self.selected_worktree_file else {
            return;
        };

        ui.separator();
        ui.add_space(10.0);
        panel_heading_inline(ui, self.tr("worktree.title"));
        ui.label(
            RichText::new(&selected.display_path)
                .monospace()
                .color(theme::TEXT),
        );
        ui.add_space(6.0);

        let key = git::worktree_diff_key(&selected.path, selected.staged);
        if self.loading_diff_key.as_deref() == Some(key.as_str()) {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(self.tr("diff.loading")).color(theme::MUTED));
            });
            return;
        }

        let Some(diff) = self.diff_cache.get(&key) else {
            ui.label(RichText::new(self.tr("diff.queued")).color(theme::MUTED));
            return;
        };

        if diff.text.trim().is_empty() {
            ui.label(RichText::new(self.tr("diff.empty")).color(theme::MUTED));
            return;
        }

        egui::Frame::new()
            .fill(Color32::from_rgb(14, 16, 21))
            .stroke(Stroke::new(1.0, Color32::from_rgb(45, 52, 66)))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(egui::Margin::symmetric(8, 8))
            .show(ui, |ui| {
                ScrollArea::both().max_height(360.0).show(ui, |ui| {
                    for line in diff.text.lines().take(1_200) {
                        diff_line(ui, line);
                    }
                    if diff.text.lines().count() > 1_200 {
                        ui.label(RichText::new(self.tr("diff.truncated")).color(theme::MUTED));
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
                            .color(theme::MUTED),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(self.tr("branch.name"))
                                .small()
                                .color(theme::MUTED),
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
                                .color(theme::MUTED),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(self.tr("tag.name"))
                                .small()
                                .color(theme::MUTED),
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
                            .color(theme::TEXT),
                        );
                        ui.label(
                            RichText::new(self.tr("commit.detached_warning")).color(theme::WARNING),
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
                            RichText::new(self.tr("commit.confirm_cherry_pick")).color(theme::TEXT),
                        );
                        ui.label(
                            RichText::new(short_hash.as_str())
                                .monospace()
                                .color(theme::MUTED),
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
                            RichText::new(self.tr("commit.confirm_revert")).color(theme::TEXT),
                        );
                        ui.label(
                            RichText::new(short_hash.as_str())
                                .monospace()
                                .color(theme::MUTED),
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
                            RichText::new(self.tr("commit.confirm_reset")).color(theme::WARNING),
                        );
                        ui.label(
                            RichText::new(short_hash.as_str())
                                .monospace()
                                .color(theme::MUTED),
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
                        ui.label(RichText::new(&path).monospace().color(theme::TEXT));
                        ui.label(
                            RichText::new(if untracked {
                                "This will delete the untracked file or directory."
                            } else {
                                "This will restore the path from HEAD."
                            })
                            .color(theme::WARNING),
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
                        ui.label(RichText::new(self.tr("stash.confirm_drop")).color(theme::TEXT));
                        ui.label(RichText::new(message.as_str()).color(theme::MUTED));
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
                                .color(theme::MUTED),
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
                        ui.label(RichText::new(remote_branch.as_str()).color(theme::TEXT));
                        ui.label(
                            RichText::new(self.tr("branch.name"))
                                .small()
                                .color(theme::MUTED),
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
                            RichText::new(self.tr("branch.confirm_delete")).color(theme::TEXT),
                        );
                        ui.label(RichText::new(name.as_str()).color(theme::WARNING));
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
                                .color(theme::MUTED),
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
                        ui.label(RichText::new(self.tr("tag.confirm_delete")).color(theme::TEXT));
                        ui.label(RichText::new(name.as_str()).color(theme::WARNING));
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
        Color32::from_rgb(36, 49, 57)
    } else if ui.is_rect_visible(rect) {
        theme::BG
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
        theme::TEXT,
    );
    painter.text(
        meta_pos,
        Align2::LEFT_TOP,
        format!(
            "{}  {}  {}",
            commit.short_hash, commit.author, commit.relative_time
        ),
        FontId::monospace(12.0),
        theme::MUTED,
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
        ui.label(RichText::new(text).strong().color(theme::TEXT));
    });
}

fn panel_heading_inline(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).strong().color(theme::TEXT));
}

fn toolbar_button(ui: &mut Ui, icon: &str, label: &str, enabled: bool) -> egui::Response {
    let text = RichText::new(format!("{icon}\n{label}")).color(theme::TEXT);
    ui.add_enabled(
        enabled,
        egui::Button::new(text).min_size(Vec2::new(58.0, 58.0)),
    )
}

fn sidebar_nav_item(ui: &mut Ui, selected: bool, icon: &str, label: &str) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
    let fill = if selected {
        Color32::from_rgb(42, 112, 185)
    } else if response.hovered() {
        Color32::from_rgb(33, 38, 50)
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(6.0, 1.0)),
            CornerRadius::same(2),
            fill,
        );
    }
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(RichText::new(icon).color(if selected {
                Color32::WHITE
            } else {
                theme::ACCENT
            }));
            ui.label(RichText::new(label).color(if selected {
                Color32::WHITE
            } else {
                theme::TEXT
            }));
        });
    });
    response
}

fn tree_header(ui: &mut Ui, open: &mut bool, icon: &str, label: &str) -> bool {
    let arrow = if *open { "⌄" } else { "›" };
    if ui
        .horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::new(arrow).color(theme::MUTED));
            ui.label(RichText::new(icon).color(theme::ACCENT));
            ui.label(RichText::new(label).strong().color(theme::TEXT));
        })
        .response
        .clicked()
    {
        *open = !*open;
    }
    *open
}

fn tree_empty(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(30.0);
        ui.label(RichText::new(text).color(theme::MUTED));
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
    egui::Frame::new()
        .fill(theme::PANEL)
        .stroke(Stroke::new(1.0, Color32::from_rgb(48, 54, 68)))
        .corner_radius(CornerRadius::same(3))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_min_height(height);
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).strong().color(theme::TEXT));
                ui.label(RichText::new(format!("({})", files.len())).color(theme::MUTED));
            });
            ui.separator();
            if files.is_empty() {
                ui.add_space(20.0);
                ui.label(RichText::new("—").color(theme::MUTED));
            } else {
                ScrollArea::vertical()
                    .max_height((height - 44.0).max(60.0))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
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
            Color32::from_rgb(32, 36, 47),
        );
    }

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            let color = if current {
                theme::ACCENT
            } else if remote {
                Color32::from_rgb(120, 164, 255)
            } else {
                theme::MUTED
            };
            let label = if remote {
                i18n::t(language, "common.remote")
            } else {
                i18n::t(language, "common.local")
            };
            ui.label(RichText::new(if current { "*" } else { " " }).color(color));
            ui.label(RichText::new(name).color(if current { theme::TEXT } else { theme::MUTED }));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(12.0);
                ui.label(RichText::new(label).small().color(color));
            });
        });
    });

    response.context_menu(|ui| {
        ui.set_min_width(200.0);
        ui.label(RichText::new(name).color(theme::TEXT));
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
        ui.label(RichText::new(name).strong().color(theme::TEXT));
    });
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.label(RichText::new(url).small().color(theme::MUTED));
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
            Color32::from_rgb(32, 36, 47),
        );
    }

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new(&stash.selector)
                        .monospace()
                        .color(theme::ACCENT),
                );
                ui.label(
                    RichText::new(&stash.relative_time)
                        .small()
                        .color(theme::MUTED),
                );
            });
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(RichText::new(&stash.message).small().color(theme::TEXT));
            });
        });
    });

    response.context_menu(|ui| {
        ui.set_min_width(180.0);
        ui.label(
            RichText::new(&stash.selector)
                .monospace()
                .color(theme::TEXT),
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
            Color32::from_rgb(32, 36, 47),
        );
    }

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(RichText::new(&tag.name).color(theme::ACCENT));
                ui.label(
                    RichText::new(&tag.target)
                        .monospace()
                        .small()
                        .color(theme::MUTED),
                );
            });
            if !tag.subject.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(RichText::new(&tag.subject).small().color(theme::MUTED));
                });
            }
        });
    });

    response.context_menu(|ui| {
        ui.set_min_width(180.0);
        ui.label(RichText::new(&tag.name).color(theme::TEXT));
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
    let response = ui.allocate_response(Vec2::new(ui.available_width(), 24.0), Sense::click());
    let rect = response.rect;
    if response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(2.0, 1.0)),
            CornerRadius::same(4),
            Color32::from_rgb(32, 36, 47),
        );
    }

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(24.0, 22.0), Sense::hover());
            draw_file_status_icon(ui, icon_rect, &status);
            ui.label(
                RichText::new(&file.display_path)
                    .monospace()
                    .color(theme::TEXT),
            );
        });
    });

    response.context_menu(|ui| {
        ui.set_min_width(180.0);
        ui.label(
            RichText::new(&file.display_path)
                .monospace()
                .color(theme::TEXT),
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
    ui.label(RichText::new(&commit.subject).strong().color(theme::TEXT));
    ui.label(
        RichText::new(format!("{}  {}", commit.short_hash, commit.author))
            .small()
            .color(theme::MUTED),
    );
    ui.separator();

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

    ui.separator();
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
    ui.separator();
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
    ui.separator();
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
    ui.label(RichText::new(label).small().color(theme::MUTED));
    ui.label(RichText::new(value).monospace().color(theme::TEXT));
}

fn file_change_row(ui: &mut Ui, status: &str, path: &str, selected: bool) -> egui::Response {
    let response = ui.allocate_response(Vec2::new(ui.available_width(), 24.0), Sense::click());
    let rect = response.rect;
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(2.0, 1.0)),
            CornerRadius::same(4),
            if selected {
                Color32::from_rgb(36, 49, 57)
            } else {
                Color32::from_rgb(32, 36, 47)
            },
        );
    }

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(24.0, 22.0), Sense::hover());
            draw_file_status_icon(ui, icon_rect, status);
            ui.label(RichText::new(path).monospace().color(theme::TEXT));
        });
    });
    response
}

fn draw_file_status_icon(ui: &mut Ui, rect: Rect, status: &str) {
    let painter = ui.painter();
    let center = rect.center();
    let kind = status.chars().next().unwrap_or('M');
    let color = match kind {
        'A' => Color32::from_rgb(104, 210, 121),
        'D' => Color32::from_rgb(244, 113, 116),
        'R' => Color32::from_rgb(120, 164, 255),
        _ => theme::WARNING,
    };
    let stroke = Stroke::new(1.8, color);

    painter.rect_filled(
        rect.shrink(2.0),
        CornerRadius::same(4),
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 22),
    );

    match kind {
        'A' => {
            painter.line_segment(
                [
                    Pos2::new(center.x - 5.0, center.y),
                    Pos2::new(center.x + 5.0, center.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x, center.y - 5.0),
                    Pos2::new(center.x, center.y + 5.0),
                ],
                stroke,
            );
        }
        'D' => {
            painter.line_segment(
                [
                    Pos2::new(center.x - 5.0, center.y - 5.0),
                    Pos2::new(center.x + 5.0, center.y + 5.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x + 5.0, center.y - 5.0),
                    Pos2::new(center.x - 5.0, center.y + 5.0),
                ],
                stroke,
            );
        }
        'R' => {
            painter.line_segment(
                [
                    Pos2::new(center.x - 6.0, center.y - 3.0),
                    Pos2::new(center.x + 5.0, center.y - 3.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x + 2.0, center.y - 6.0),
                    Pos2::new(center.x + 5.0, center.y - 3.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x + 2.0, center.y),
                    Pos2::new(center.x + 5.0, center.y - 3.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x + 6.0, center.y + 4.0),
                    Pos2::new(center.x - 5.0, center.y + 4.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x - 2.0, center.y + 1.0),
                    Pos2::new(center.x - 5.0, center.y + 4.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(center.x - 2.0, center.y + 7.0),
                    Pos2::new(center.x - 5.0, center.y + 4.0),
                ],
                stroke,
            );
        }
        _ => {
            painter.add(Shape::convex_polygon(
                vec![
                    Pos2::new(center.x - 7.0, center.y + 3.5),
                    Pos2::new(center.x + 1.5, center.y - 5.0),
                    Pos2::new(center.x + 5.0, center.y - 1.5),
                    Pos2::new(center.x - 3.5, center.y + 7.0),
                ],
                color,
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 70)),
            ));
            painter.line_segment(
                [
                    Pos2::new(center.x - 5.0, center.y + 5.0),
                    Pos2::new(center.x + 3.0, center.y - 3.0),
                ],
                Stroke::new(1.2, Color32::from_rgba_unmultiplied(255, 255, 255, 140)),
            );
            painter.add(Shape::convex_polygon(
                vec![
                    Pos2::new(center.x + 1.5, center.y - 5.0),
                    Pos2::new(center.x + 7.0, center.y - 7.0),
                    Pos2::new(center.x + 5.0, center.y - 1.5),
                ],
                Color32::from_rgb(245, 210, 150),
                Stroke::new(1.0, color),
            ));
            painter.circle_filled(Pos2::new(center.x + 7.0, center.y - 7.0), 1.5, theme::TEXT);
        }
    }
}

fn diff_line(ui: &mut Ui, line: &str) {
    let color = if line.starts_with('+') && !line.starts_with("+++") {
        Color32::from_rgb(126, 222, 144)
    } else if line.starts_with('-') && !line.starts_with("---") {
        Color32::from_rgb(255, 128, 132)
    } else if line.starts_with("@@") {
        Color32::from_rgb(120, 164, 255)
    } else if line.starts_with("diff --git") {
        theme::ACCENT
    } else {
        theme::MUTED
    };

    ui.label(RichText::new(line).monospace().color(color));
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
                        .color(theme::TEXT),
                );
                return;
            }
            ui.label(
                RichText::new(i18n::t(language, "repo.none"))
                    .heading()
                    .color(theme::TEXT),
            );
            ui.label(
                RichText::new("Git Agent will render the commit graph with virtualized rows.")
                    .color(theme::MUTED),
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
                    .color(theme::TEXT),
            );
            ui.label(
                RichText::new(i18n::t(language, "commit.no_commits_hint")).color(theme::MUTED),
            );
        });
    });
}
