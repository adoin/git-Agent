use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, anyhow};
use eframe::{
    App,
    egui::{
        self, Align, Align2, Color32, FontId, Layout, Pos2, Rect, RichText, ScrollArea, Sense,
        TextEdit, Ui, Vec2,
    },
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeSide {
    Local,
    Remote,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MergeLineAction {
    Take,
    Drop,
}

#[derive(Clone, Copy)]
struct MergePalette {
    bg: Color32,
    panel: Color32,
    panel_soft: Color32,
    text: Color32,
    muted: Color32,
    accent: Color32,
    conflict_fill: Color32,
    conflict_text: Color32,
    result_fill: Color32,
    shadow: eframe::epaint::Shadow,
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
    })
}

pub fn three_way_merge(base: &str, local: &str, remote: &str) -> MergeDocument {
    let base_lines = split_lines(base);
    let local_lines = split_lines(local);
    let remote_lines = split_lines(remote);
    let max_len = base_lines
        .len()
        .max(local_lines.len())
        .max(remote_lines.len());
    let mut lines = Vec::new();
    let mut conflicts = Vec::new();

    for index in 0..max_len {
        let base_line = base_lines.get(index).cloned();
        let local_line = local_lines.get(index).cloned();
        let remote_line = remote_lines.get(index).cloned();
        let resolved = resolve_line(
            base_line.as_deref(),
            local_line.as_deref(),
            remote_line.as_deref(),
        );
        match resolved {
            Some(result) => {
                let include_in_result = result.is_some();
                lines.push(MergeLine {
                    base: base_line,
                    local: local_line,
                    remote: remote_line,
                    result: result.unwrap_or_default(),
                    include_in_result,
                    kind: MergeLineKind::Resolved,
                    conflict_index: None,
                    local_resolved: true,
                    remote_resolved: true,
                    local_taken: false,
                    remote_taken: false,
                });
            }
            None => {
                let conflict_index = conflicts.len();
                let line_index = lines.len();
                conflicts.push(ConflictBlock {
                    index: conflict_index,
                    base: base_line.iter().cloned().collect(),
                    local: local_line.iter().cloned().collect(),
                    remote: remote_line.iter().cloned().collect(),
                    line_indices: vec![line_index],
                });
                lines.push(MergeLine {
                    result: base_line.clone().unwrap_or_default(),
                    include_in_result: base_line.is_some(),
                    base: base_line,
                    local: local_line,
                    remote: remote_line,
                    kind: MergeLineKind::Conflict,
                    conflict_index: Some(conflict_index),
                    local_resolved: false,
                    remote_resolved: false,
                    local_taken: false,
                    remote_taken: false,
                });
            }
        }
    }

    MergeDocument { lines, conflicts }
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

    fn apply_conflict(&mut self, index: usize, side: MergeSide) {
        let Some(conflict) = self.conflicts.get(index).cloned() else {
            return;
        };
        let replacement = match side {
            MergeSide::Local => conflict.local,
            MergeSide::Remote => conflict.remote,
        };
        for (offset, line_index) in conflict.line_indices.iter().copied().enumerate() {
            if let Some(line) = self.lines.get_mut(line_index) {
                line.result = replacement.get(offset).cloned().unwrap_or_default();
                line.include_in_result = replacement.get(offset).is_some();
                line.set_side(side, true);
            }
        }
    }

    pub fn take_conflict_side(&mut self, index: usize, side: MergeSide) {
        self.set_conflict_side(index, side, MergeLineAction::Take);
    }

    pub fn drop_conflict_side(&mut self, index: usize, side: MergeSide) {
        self.set_conflict_side(index, side, MergeLineAction::Drop);
    }

    pub fn unresolved_conflict_count(&self, side: MergeSide) -> usize {
        self.conflicts
            .iter()
            .filter(|conflict| self.conflict_side_unresolved(conflict.index, side))
            .count()
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
        if lines.is_empty() && self.include_in_result {
            lines.push(self.result.as_str());
        }
        if lines.is_empty() && self.kind != MergeLineKind::Conflict {
            lines.push(self.result.as_str());
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
        self.include_in_result = self.kind != MergeLineKind::Conflict
            || self.local_taken
            || self.remote_taken
            || !self.result.is_empty();
    }
}

pub struct MergeToolApp {
    args: MergeArgs,
    document: MergeDocument,
    result_text: String,
    local_conflict_cursor: usize,
    remote_conflict_cursor: usize,
    theme: MergeTheme,
    language: MergeLanguage,
    status: Option<String>,
}

impl MergeToolApp {
    pub fn from_args(args: MergeArgs) -> anyhow::Result<Self> {
        let base = read_text(&args.base)?;
        let local = read_text(&args.local)?;
        let remote = read_text(&args.remote)?;
        let document = three_way_merge(&base, &local, &remote);
        Ok(Self::new(args, document))
    }

    pub fn new(args: MergeArgs, document: MergeDocument) -> Self {
        let result_text = document.result_text();
        Self {
            theme: args.theme,
            language: args.language,
            args,
            document,
            result_text,
            local_conflict_cursor: 0,
            remote_conflict_cursor: 0,
            status: None,
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
                .with_inner_size([1180.0, 760.0])
                .with_min_inner_size([980.0, 620.0]),
            ..Default::default()
        };
        eframe::run_native(
            &title,
            options,
            Box::new(move |cc| {
                crate::theme::install(&cc.egui_ctx);
                apply_merge_theme(&cc.egui_ctx, args.theme);
                let app = Self::from_args(args).unwrap_or_else(|error| {
                    let fallback = MergeArgs {
                        base: PathBuf::new(),
                        local: PathBuf::new(),
                        remote: PathBuf::new(),
                        output: PathBuf::new(),
                        repo_root: None,
                        stage: false,
                        theme: MergeTheme::Dark,
                        language: MergeLanguage::English,
                    };
                    let mut app = Self::new(fallback, three_way_merge("", "", ""));
                    app.status = Some(error.to_string());
                    app
                });
                Ok(Box::new(app))
            }),
        )
    }

    fn accept_conflict(&mut self, side: MergeSide) {
        let index = match side {
            MergeSide::Local => self.local_conflict_cursor,
            MergeSide::Remote => self.remote_conflict_cursor,
        };
        self.document.apply_conflict(index, side);
        self.result_text = self.document.result_text();
        let conflict_count = self.document.conflicts().len();
        if conflict_count > 0 {
            self.local_conflict_cursor = (self.local_conflict_cursor + 1).min(conflict_count - 1);
            self.remote_conflict_cursor = (self.remote_conflict_cursor + 1).min(conflict_count - 1);
        }
    }

    fn apply_line_action(&mut self, index: usize, side: MergeSide, action: MergeLineAction) {
        match action {
            MergeLineAction::Take => self.document.take_conflict_side(index, side),
            MergeLineAction::Drop => self.document.drop_conflict_side(index, side),
        }
        self.result_text = self.document.result_text();
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
        match write_merge_output(&self.args, &self.result_text) {
            Ok(()) => std::process::exit(0),
            Err(error) => {
                self.status = Some(format!(
                    "{} {}: {error}",
                    mt(self.language, "write_failed"),
                    self.args.output.display()
                ))
            }
        }
    }
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
    let status = Command::new("git")
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

impl App for MergeToolApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let palette = merge_palette(self.theme);
        apply_merge_theme(ctx, self.theme);

        egui::TopBottomPanel::top("merge_toolbar")
            .exact_height(42.0)
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ctx, |ui| merge_toolbar(ui, self, palette));

        egui::TopBottomPanel::bottom("merge_footer")
            .exact_height(56.0)
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ctx, |ui| merge_footer(ui, self, palette));

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.bg))
            .show(ctx, |ui| merge_editor_columns(ui, self, palette));
    }
}

fn split_lines(text: &str) -> Vec<String> {
    text.lines().map(ToOwned::to_owned).collect()
}

fn resolve_line(
    base: Option<&str>,
    local: Option<&str>,
    remote: Option<&str>,
) -> Option<Option<String>> {
    match (base, local, remote) {
        (_, Some(local), Some(remote)) if local == remote => Some(Some(local.to_owned())),
        (Some(base), Some(local), Some(remote)) if local == base => Some(Some(remote.to_owned())),
        (Some(base), Some(local), Some(remote)) if remote == base => Some(Some(local.to_owned())),
        (Some(base), None, Some(remote)) if remote == base => Some(None),
        (Some(base), Some(local), None) if local == base => Some(None),
        (None, Some(local), None) => Some(Some(local.to_owned())),
        (None, None, Some(remote)) => Some(Some(remote.to_owned())),
        (Some(_), None, None) => None,
        (None, None, None) => None,
        _ => None,
    }
}

fn read_text(path: &Path) -> anyhow::Result<String> {
    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
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

fn merge_toolbar(ui: &mut Ui, app: &mut MergeToolApp, palette: MergePalette) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(
            RichText::new(mt(app.language, "title"))
                .strong()
                .color(palette.text),
        );
        ui.add_space(10.0);
        ui.label(
            RichText::new(format!(
                "{} {}",
                app.document.conflicts().len(),
                mt(app.language, "conflicts")
            ))
            .monospace()
            .color(palette.conflict_text),
        );
        ui.add_space(10.0);
        ui.label(RichText::new(mt(app.language, "auto_applied")).color(palette.muted));

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button(merge_language_label(app.language)).clicked() {
                app.toggle_language();
            }
            if ui
                .button(merge_theme_label(app.language, app.theme))
                .clicked()
            {
                app.toggle_theme(ui.ctx());
            }
            ui.label(
                RichText::new(format!(
                    "{} {} {}",
                    mt(app.language, "no_changes"),
                    app.document.conflicts().len(),
                    mt(app.language, "conflict_count")
                ))
                .color(palette.muted),
            );
            if let Some(status) = &app.status {
                ui.label(RichText::new(status).color(palette.conflict_text));
            }
        });
    });
}

fn merge_footer(ui: &mut Ui, app: &mut MergeToolApp, palette: MergePalette) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        if ui.button(mt(app.language, "accept_left")).clicked() {
            app.accept_conflict(MergeSide::Local);
        }
        if ui.button(mt(app.language, "accept_right")).clicked() {
            app.accept_conflict(MergeSide::Remote);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(10.0);
            if ui
                .add_sized([88.0, 30.0], egui::Button::new(mt(app.language, "cancel")))
                .clicked()
            {
                std::process::exit(1);
            }
            if ui
                .add_sized(
                    [88.0, 30.0],
                    egui::Button::new(
                        RichText::new(mt(app.language, "apply"))
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(palette.accent),
                )
                .clicked()
            {
                app.write_output();
            }
        });
    });
}

fn merge_editor_columns(ui: &mut Ui, app: &mut MergeToolApp, palette: MergePalette) {
    let rect = ui
        .available_rect_before_wrap()
        .shrink2(Vec2::new(10.0, 8.0));
    let gap = MERGE_COLUMN_GAP;
    let left_w = (rect.width() * 0.32).max(250.0);
    let result_w = (rect.width() * 0.34).max(280.0);
    let right_w = (rect.width() - left_w - result_w - gap * 2.0).max(250.0);
    let left = Rect::from_min_size(rect.min, Vec2::new(left_w, rect.height()));
    let result = Rect::from_min_size(
        Pos2::new(left.right() + gap, rect.top()),
        Vec2::new(result_w, rect.height()),
    );
    let right = Rect::from_min_size(
        Pos2::new(result.right() + gap, rect.top()),
        Vec2::new(right_w, rect.height()),
    );

    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(left), |ui| {
        merge_side_panel(ui, app, MergeSide::Local, "merge_local_scroll", palette);
    });
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(result), |ui| {
        merge_result_panel(ui, app, "merge_result_scroll", palette);
    });
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(right), |ui| {
        merge_side_panel(ui, app, MergeSide::Remote, "merge_remote_scroll", palette);
    });
}

fn merge_side_panel(
    ui: &mut Ui,
    app: &mut MergeToolApp,
    side: MergeSide,
    scroll_id: &'static str,
    palette: MergePalette,
) {
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
        side_conflict_nav(ui, app, side, palette);
        ui.add_space(8.0);
        ScrollArea::vertical()
            .id_salt(scroll_id)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let cursor = match side {
                    MergeSide::Local => app.local_conflict_cursor,
                    MergeSide::Remote => app.remote_conflict_cursor,
                };
                let mut pending_line_action = None;
                for (index, line) in app.document.lines.iter().enumerate() {
                    let text = match side {
                        MergeSide::Local => line.local.as_deref().unwrap_or(""),
                        MergeSide::Remote => line.remote.as_deref().unwrap_or(""),
                    };
                    merge_code_row(
                        ui,
                        index,
                        side,
                        text,
                        line.conflict_index,
                        line.side_resolved(side),
                        cursor,
                        palette,
                        &mut pending_line_action,
                    );
                }
                if let Some((index, action)) = pending_line_action {
                    app.apply_line_action(index, side, action);
                }
            });
    });
}

fn merge_result_panel(
    ui: &mut Ui,
    app: &mut MergeToolApp,
    scroll_id: &'static str,
    palette: MergePalette,
) {
    merge_panel_frame(ui, palette, |ui| {
        side_header(ui, mt(app.language, "result"), &app.args.output, palette);
        ui.add_space(30.0);
        ui.add_space(8.0);
        ScrollArea::vertical()
            .id_salt(scroll_id)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(
                    TextEdit::multiline(&mut app.result_text)
                        .font(FontId::monospace(13.0))
                        .desired_width(ui.available_width())
                        .desired_rows(32)
                        .text_color(palette.text)
                        .background_color(palette.result_fill)
                        .frame(false),
                );
            });
    });
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

fn side_conflict_nav(ui: &mut Ui, app: &mut MergeToolApp, side: MergeSide, palette: MergePalette) {
    ui.horizontal(|ui| {
        let conflict_count = app.document.unresolved_conflict_count(side);
        let mut cursor = match side {
            MergeSide::Local => app.local_conflict_cursor,
            MergeSide::Remote => app.remote_conflict_cursor,
        };
        let enabled = conflict_count > 0;
        if ui.add_enabled(enabled, egui::Button::new("^")).clicked() {
            previous_unresolved_conflict(&app.document, side, &mut cursor);
        }
        if ui.add_enabled(enabled, egui::Button::new("v")).clicked() {
            next_unresolved_conflict(&app.document, side, &mut cursor);
        }
        match side {
            MergeSide::Local => app.local_conflict_cursor = cursor,
            MergeSide::Remote => app.remote_conflict_cursor = cursor,
        }
        ui.label(
            RichText::new(format!(
                "{} / {}",
                unresolved_position(&app.document, side, cursor),
                conflict_count
            ))
            .color(palette.muted),
        );
    });
}

fn merge_code_row(
    ui: &mut Ui,
    index: usize,
    side: MergeSide,
    text: &str,
    conflict_index: Option<usize>,
    side_resolved: bool,
    cursor: usize,
    palette: MergePalette,
    pending_action: &mut Option<(usize, MergeLineAction)>,
) {
    let row_h = 22.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::hover());
    let active_conflict = conflict_index == Some(cursor) && !side_resolved;
    if conflict_index.is_some() && !side_resolved {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::ZERO,
            if active_conflict {
                palette.conflict_fill
            } else {
                palette.panel_soft
            },
        );
    }
    if let Some(conflict_index) = conflict_index.filter(|_| !side_resolved) {
        let drop_rect = Rect::from_min_size(
            Pos2::new(rect.left() + 4.0, rect.top() + 2.0),
            Vec2::splat(18.0),
        );
        let take_rect = Rect::from_min_size(
            Pos2::new(rect.left() + 24.0, rect.top() + 2.0),
            Vec2::new(28.0, 18.0),
        );
        let drop_response = ui.put(drop_rect, egui::Button::new("X"));
        if drop_response.clicked() {
            *pending_action = Some((conflict_index, MergeLineAction::Drop));
        }
        let arrow = match side {
            MergeSide::Local => ">>",
            MergeSide::Remote => "<<",
        };
        let take_response = ui.put(take_rect, egui::Button::new(arrow));
        if take_response.clicked() {
            *pending_action = Some((conflict_index, MergeLineAction::Take));
        }
    }
    ui.painter().text(
        Pos2::new(rect.left() + 58.0, rect.center().y),
        Align2::LEFT_CENTER,
        format!("{:>4}", index + 1),
        FontId::monospace(12.0),
        palette.muted,
    );
    let text_rect = Rect::from_min_max(
        Pos2::new(rect.left() + 100.0, rect.top()),
        rect.right_bottom(),
    );
    ui.painter().with_clip_rect(text_rect).text(
        text_rect.left_center(),
        Align2::LEFT_CENTER,
        text,
        FontId::monospace(13.0),
        if conflict_index.is_some() && !side_resolved {
            palette.conflict_text
        } else {
            palette.text
        },
    );
}

fn previous_unresolved_conflict(document: &MergeDocument, side: MergeSide, cursor: &mut usize) {
    if let Some(index) = document
        .conflicts()
        .iter()
        .map(|conflict| conflict.index)
        .rev()
        .find(|index| *index < *cursor && !document.conflict_side_resolved(*index, side))
    {
        *cursor = index;
    }
}

fn next_unresolved_conflict(document: &MergeDocument, side: MergeSide, cursor: &mut usize) {
    if let Some(index) = document
        .conflicts()
        .iter()
        .map(|conflict| conflict.index)
        .find(|index| *index > *cursor && !document.conflict_side_resolved(*index, side))
    {
        *cursor = index;
    }
}

fn unresolved_position(document: &MergeDocument, side: MergeSide, cursor: usize) -> usize {
    document
        .conflicts()
        .iter()
        .filter(|conflict| !document.conflict_side_resolved(conflict.index, side))
        .position(|conflict| conflict.index == cursor)
        .map(|index| index + 1)
        .unwrap_or(0)
}

fn merge_panel_frame(ui: &mut Ui, palette: MergePalette, body: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(palette.panel)
        .shadow(palette.shadow)
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
    visuals.extreme_bg_color = palette.panel_soft;
    visuals.faint_bg_color = palette.panel_soft;
    visuals.override_text_color = Some(palette.text);
    visuals.selection.bg_fill = palette.accent;
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
            conflict_fill: Color32::from_rgb(76, 51, 45),
            conflict_text: Color32::from_rgb(255, 190, 170),
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
            conflict_fill: Color32::from_rgb(255, 219, 209),
            conflict_text: Color32::from_rgb(154, 52, 42),
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

fn mt(language: MergeLanguage, key: &str) -> &'static str {
    match (language, key) {
        (MergeLanguage::Chinese, "title") => "合并修订",
        (MergeLanguage::Chinese, "conflicts") => "个冲突",
        (MergeLanguage::Chinese, "auto_applied") => "非冲突内容已自动合并",
        (MergeLanguage::Chinese, "no_changes") => "无其他变更。",
        (MergeLanguage::Chinese, "conflict_count") => "个冲突。",
        (MergeLanguage::Chinese, "local") => "本地变更",
        (MergeLanguage::Chinese, "remote") => "远端变更",
        (MergeLanguage::Chinese, "result") => "合并结果",
        (MergeLanguage::Chinese, "accept_left") => "使用我的版本",
        (MergeLanguage::Chinese, "accept_right") => "使用他的版本",
        (MergeLanguage::Chinese, "apply") => "应用",
        (MergeLanguage::Chinese, "cancel") => "取消",
        (MergeLanguage::Chinese, "light") => "白天",
        (MergeLanguage::Chinese, "dark") => "黑夜",
        (MergeLanguage::Chinese, "write_failed") => "写入失败",
        (_, "title") => "Merge Revisions",
        (_, "conflicts") => "conflict(s)",
        (_, "auto_applied") => "Non-conflicting changes auto-applied",
        (_, "no_changes") => "No changes.",
        (_, "conflict_count") => "conflict(s).",
        (_, "local") => "Local Changes",
        (_, "remote") => "Remote Changes",
        (_, "result") => "Result",
        (_, "accept_left") => "Use Mine",
        (_, "accept_right") => "Use Theirs",
        (_, "apply") => "Apply",
        (_, "cancel") => "Cancel",
        (_, "light") => "Light",
        (_, "dark") => "Dark",
        (_, "write_failed") => "Failed to write",
        _ => "",
    }
}
