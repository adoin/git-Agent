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
    git::{self, Commit, CommitDetails, FileDiff, RepositorySnapshot},
    graph::{self, EdgeKind, GraphLayout},
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
    loading_repo: bool,
    loading_details_hash: Option<String>,
    loading_diff_key: Option<String>,
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
            loading_repo: false,
            loading_details_hash: None,
            loading_diff_key: None,
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
}

impl App for GitAgentApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_tasks(ctx);

        egui::TopBottomPanel::top("top_bar")
            .exact_height(64.0)
            .frame(egui::Frame::new().fill(theme::BG))
            .show(ctx, |ui| self.top_bar(ui));

        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(270.0)
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
            .show(ctx, |ui| self.commit_graph(ui));
    }
}

impl GitAgentApp {
    fn top_bar(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            ui.add_space(14.0);
            ui.label(RichText::new("Git Agent").heading().color(theme::TEXT));
            ui.label(RichText::new("fast visual Git client").color(theme::MUTED));

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(14.0);
                if ui
                    .add_enabled(!self.loading_repo, egui::Button::new("Open"))
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.load_repository(path);
                    }
                }
                if ui
                    .add_enabled(
                        !self.loading_repo && self.snapshot.is_some(),
                        egui::Button::new("Refresh"),
                    )
                    .clicked()
                {
                    self.refresh();
                }
                if self.loading_repo {
                    ui.spinner();
                    ui.label(RichText::new("Loading repository").color(theme::MUTED));
                }
            });
        });
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("Repository").strong().color(theme::TEXT));
                if let Some(snapshot) = &self.snapshot {
                    ui.label(
                        RichText::new(snapshot.root.display().to_string())
                            .small()
                            .color(theme::MUTED),
                    );
                } else {
                    ui.label(RichText::new("No repository loaded").color(theme::MUTED));
                }
            });
        });

        ui.add_space(24.0);
        panel_heading(ui, "Branch");
        if let Some(snapshot) = &self.snapshot {
            sidebar_pill(ui, &snapshot.branch, theme::ACCENT);
            ui.add_space(18.0);
            panel_heading(ui, "Branches");
            ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                for branch in snapshot.branches.iter().take(24) {
                    branch_row(ui, branch.current, branch.remote, &branch.name);
                }
                if snapshot.branches.len() > 24 {
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(format!("+{} more", snapshot.branches.len() - 24))
                                .color(theme::MUTED),
                        );
                    });
                }
            });
            ui.add_space(18.0);
            panel_heading(ui, "Working Tree");
            if snapshot.status.is_empty() {
                ui.label(RichText::new("Clean").color(theme::MUTED));
            } else {
                for item in snapshot.status.iter().take(12) {
                    ui.label(RichText::new(item).monospace().color(theme::TEXT));
                }
                if snapshot.status.len() > 12 {
                    ui.label(
                        RichText::new(format!("+{} more", snapshot.status.len() - 12))
                            .color(theme::MUTED),
                    );
                }
            }
        }

        if let Some(error) = &self.error {
            ui.add_space(18.0);
            ui.colored_label(theme::WARNING, error);
        }
    }

    fn commit_graph(&mut self, ui: &mut Ui) {
        if self.snapshot.is_none() {
            empty_state(ui, self.loading_repo);
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
            ui.label(RichText::new(format!("{} commits loaded", commit_count)).color(theme::MUTED));
            ui.separator();
            ui.label(
                RichText::new(format!("{} graph lanes", self.layout.lanes.max(1)))
                    .color(theme::MUTED),
            );
            ui.separator();
            ui.label(RichText::new(format!("{} visible", visible_rows.len())).color(theme::MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(16.0);
                let response = ui.add_sized(
                    [260.0, 30.0],
                    TextEdit::singleline(&mut self.search).hint_text("Search commits"),
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
            no_commits_state(ui);
            return;
        }

        if visible_rows.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("No matching commits").color(theme::MUTED));
            });
            return;
        }

        let mut clicked_commit = None;
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
    }

    fn details(&mut self, ui: &mut Ui) {
        ui.add_space(18.0);
        panel_heading(ui, "Commit Details");
        ui.add_space(8.0);

        let Some(snapshot) = &self.snapshot else {
            ui.label(RichText::new("Open a repository to inspect commits.").color(theme::MUTED));
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
            detail_line(ui, "Hash", &commit.hash);
            detail_line(ui, "Author", &commit.author);
            detail_line(ui, "When", &commit.relative_time);
            detail_line(ui, "Parents", &commit.parents.len().to_string());
            ui.add_space(18.0);
            panel_heading_inline(ui, "Changed Files");
            ui.add_space(6.0);

            if self.loading_details_hash.as_deref() == Some(commit.hash.as_str()) {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Loading files").color(theme::MUTED));
                });
            } else if let Some(details) = self.details_cache.get(&commit.hash) {
                if details.files.is_empty() {
                    ui.label(RichText::new("No file changes recorded.").color(theme::MUTED));
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
                        self.request_selected_file_diff();
                    }
                }
            } else {
                ui.label(RichText::new("Select the commit to load files.").color(theme::MUTED));
            }

            ui.add_space(14.0);
            panel_heading_inline(ui, "Diff");
            ui.add_space(6.0);
            self.diff_viewer(ui, &commit.hash);
        } else {
            ui.label(RichText::new("No commits found.").color(theme::MUTED));
        }
    }

    fn diff_viewer(&self, ui: &mut Ui, hash: &str) {
        let Some(path) = &self.selected_file_path else {
            ui.label(RichText::new("Select a changed file.").color(theme::MUTED));
            return;
        };
        let key = git::diff_key(hash, path);

        if self.loading_diff_key.as_deref() == Some(key.as_str()) {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("Loading diff").color(theme::MUTED));
            });
            return;
        }

        let Some(diff) = self.diff_cache.get(&key) else {
            ui.label(RichText::new("Diff is queued for loading.").color(theme::MUTED));
            return;
        };

        if diff.text.trim().is_empty() {
            ui.label(RichText::new("No textual diff for this file.").color(theme::MUTED));
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
                        ui.label(RichText::new("Diff truncated at 1200 lines").color(theme::MUTED));
                    }
                });
            });
    }
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

fn branch_row(ui: &mut Ui, current: bool, remote: bool, name: &str) {
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        let color = if current {
            theme::ACCENT
        } else if remote {
            Color32::from_rgb(120, 164, 255)
        } else {
            theme::MUTED
        };
        let label = if remote { "remote" } else { "local" };
        ui.label(RichText::new(if current { "*" } else { " " }).color(color));
        ui.label(RichText::new(name).color(if current { theme::TEXT } else { theme::MUTED }));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(12.0);
            ui.label(RichText::new(label).small().color(color));
        });
    });
}

fn sidebar_pill(ui: &mut Ui, text: &str, color: Color32) {
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        egui::Frame::new()
            .fill(Color32::from_rgb(31, 42, 45))
            .stroke(Stroke::new(1.0, color))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.label(RichText::new(text).color(theme::TEXT));
            });
    });
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
            let color = match status.chars().next().unwrap_or('M') {
                'A' => Color32::from_rgb(104, 210, 121),
                'D' => Color32::from_rgb(244, 113, 116),
                'R' => Color32::from_rgb(120, 164, 255),
                _ => theme::WARNING,
            };
            ui.add_sized(
                [34.0, 22.0],
                egui::Label::new(RichText::new(status).monospace().color(color)),
            );
            ui.label(RichText::new(path).monospace().color(theme::TEXT));
        });
    });
    response
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

fn empty_state(ui: &mut Ui, loading: bool) {
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            if loading {
                ui.spinner();
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Loading repository")
                        .heading()
                        .color(theme::TEXT),
                );
                return;
            }
            ui.label(
                RichText::new("Open a Git repository")
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

fn no_commits_state(ui: &mut Ui) {
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("No commits yet")
                    .heading()
                    .color(theme::TEXT),
            );
            ui.label(
                RichText::new("Create the first commit, then the graph will render here.")
                    .color(theme::MUTED),
            );
        });
    });
}
