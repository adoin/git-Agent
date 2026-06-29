use std::{env, fs, path::PathBuf};

use anyhow::{Context, anyhow};
use eframe::{
    App,
    egui::{
        self, Align, Align2, Color32, FontId, Layout, Pos2, Rect, RichText, ScrollArea, Sense,
        Stroke, Vec2,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiffTheme {
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiffLanguage {
    English,
    Chinese,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiffCellKind {
    Context,
    Added,
    Removed,
    Empty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffLine {
    pub left_line: Option<usize>,
    pub right_line: Option<usize>,
    pub left_text: String,
    pub right_text: String,
    pub left_kind: DiffCellKind,
    pub right_kind: DiffCellKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiffRow {
    Meta(String),
    Hunk(String),
    Line(DiffLine),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffFile {
    pub left_path: String,
    pub right_path: String,
    pub rows: Vec<DiffRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffArgs {
    pub title: String,
    pub left_label: String,
    pub right_label: String,
    pub diff: PathBuf,
    pub theme: DiffTheme,
    pub language: DiffLanguage,
}

pub struct DiffToolApp {
    args: DiffArgs,
    diff_text: String,
    files: Vec<DiffFile>,
}

impl DiffToolApp {
    pub fn from_args(args: DiffArgs) -> anyhow::Result<Self> {
        let diff_text = fs::read_to_string(&args.diff)
            .with_context(|| format!("failed to read {}", args.diff.display()))?;
        let files = parse_side_by_side_diff(&diff_text);
        Ok(Self {
            args,
            diff_text,
            files,
        })
    }

    pub fn run_from_env() -> eframe::Result<()> {
        let args = match parse_diff_args(env::args()) {
            Ok(args) => args,
            Err(error) => {
                eprintln!(
                    "Usage: git-agent-diff --title <title> --left <label> --right <label> --diff <patch> [--theme dark|light] [--language en|zh]\n{error}"
                );
                std::process::exit(2);
            }
        };
        let title = format!("Git Agent Diff - {}", args.title);
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title(title.clone())
                .with_inner_size([1120.0, 760.0])
                .with_min_inner_size([820.0, 540.0]),
            ..Default::default()
        };
        eframe::run_native(
            &title,
            options,
            Box::new(move |cc| {
                crate::theme::install(&cc.egui_ctx);
                apply_diff_theme(&cc.egui_ctx, args.theme);
                let app = Self::from_args(args).unwrap_or_else(|error| Self {
                    args: DiffArgs {
                        title: "Diff".to_owned(),
                        left_label: String::new(),
                        right_label: String::new(),
                        diff: PathBuf::new(),
                        theme: DiffTheme::Dark,
                        language: DiffLanguage::English,
                    },
                    diff_text: error.to_string(),
                    files: Vec::new(),
                });
                Ok(Box::new(app))
            }),
        )
    }
}

impl App for DiffToolApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_diff_theme(ctx, self.args.theme);
        let palette = diff_palette(self.args.theme);

        egui::TopBottomPanel::top("diff_toolbar")
            .exact_height(54.0)
            .frame(egui::Frame::new().fill(palette.bg))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(RichText::new(&self.args.title).strong().color(palette.text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(12.0);
                        ui.label(RichText::new(&self.args.right_label).color(palette.added));
                        ui.label(RichText::new("vs").color(palette.muted));
                        ui.label(RichText::new(&self.args.left_label).color(palette.removed));
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ctx, |ui| {
                ScrollArea::both()
                    .id_salt("diff_tool_side_by_side_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(10.0);
                        if self.diff_text.trim().is_empty() {
                            ui.label(
                                RichText::new(dt(self.args.language, "empty")).color(palette.muted),
                            );
                            return;
                        }
                        if self.files.is_empty() {
                            show_raw_diff(ui, &self.diff_text, palette);
                        } else {
                            show_side_by_side_diff(
                                ui,
                                &self.files,
                                &self.args.left_label,
                                &self.args.right_label,
                                palette,
                            );
                        }
                    });
            });
    }
}

pub fn parse_diff_args<I, S>(args: I) -> anyhow::Result<DiffArgs>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut items = args.into_iter().map(Into::into).collect::<Vec<_>>();
    if !items.is_empty() {
        items.remove(0);
    }

    let mut title = None;
    let mut left_label = None;
    let mut right_label = None;
    let mut diff = None;
    let mut theme = DiffTheme::Dark;
    let mut language = DiffLanguage::English;
    let mut iter = items.into_iter();

    while let Some(item) = iter.next() {
        match item.as_str() {
            "--title" => title = iter.next(),
            "--left" => left_label = iter.next(),
            "--right" => right_label = iter.next(),
            "--diff" => diff = iter.next().map(PathBuf::from),
            "--theme" => {
                theme = match iter.next().as_deref() {
                    Some("light") => DiffTheme::Light,
                    Some("dark") => DiffTheme::Dark,
                    Some(value) => return Err(anyhow!("unsupported theme {value}")),
                    None => return Err(anyhow!("missing value for --theme")),
                };
            }
            "--language" => {
                language = match iter.next().as_deref() {
                    Some("zh") => DiffLanguage::Chinese,
                    Some("en") => DiffLanguage::English,
                    Some(value) => return Err(anyhow!("unsupported language {value}")),
                    None => return Err(anyhow!("missing value for --language")),
                };
            }
            other => return Err(anyhow!("unexpected argument {other}")),
        }
    }

    Ok(DiffArgs {
        title: title.ok_or_else(|| anyhow!("missing --title"))?,
        left_label: left_label.ok_or_else(|| anyhow!("missing --left"))?,
        right_label: right_label.ok_or_else(|| anyhow!("missing --right"))?,
        diff: diff.ok_or_else(|| anyhow!("missing --diff"))?,
        theme,
        language,
    })
}

pub fn parse_side_by_side_diff(diff_text: &str) -> Vec<DiffFile> {
    let mut files = Vec::new();
    let mut current: Option<DiffFile> = None;
    let mut left_line = 0usize;
    let mut right_line = 0usize;
    let mut in_hunk = false;
    let mut removed = Vec::<String>::new();
    let mut added = Vec::<String>::new();

    for raw in diff_text.lines() {
        if raw.starts_with("diff --git ") {
            if let Some(file) = current.as_mut() {
                flush_change_block(
                    file,
                    &mut removed,
                    &mut added,
                    &mut left_line,
                    &mut right_line,
                );
            }
            push_current_file(&mut files, &mut current);
            let (left_path, right_path) = parse_diff_git_paths(raw);
            current = Some(DiffFile {
                left_path,
                right_path,
                rows: Vec::new(),
            });
            in_hunk = false;
            left_line = 0;
            right_line = 0;
            continue;
        }

        let file = current_file_mut(&mut current);
        if raw.starts_with("--- ") {
            file.left_path = raw.trim_start_matches("--- ").to_owned();
            continue;
        }
        if raw.starts_with("+++ ") {
            file.right_path = raw.trim_start_matches("+++ ").to_owned();
            continue;
        }
        if raw.starts_with("@@") {
            flush_change_block(
                file,
                &mut removed,
                &mut added,
                &mut left_line,
                &mut right_line,
            );
            if let Some((left_start, right_start)) = parse_hunk_start(raw) {
                left_line = left_start;
                right_line = right_start;
            }
            file.rows.push(DiffRow::Hunk(raw.to_owned()));
            in_hunk = true;
            continue;
        }

        if !in_hunk {
            if !raw.is_empty() {
                file.rows.push(DiffRow::Meta(raw.to_owned()));
            }
            continue;
        }

        if raw.starts_with('-') && !raw.starts_with("---") {
            removed.push(raw[1..].to_owned());
        } else if raw.starts_with('+') && !raw.starts_with("+++") {
            added.push(raw[1..].to_owned());
        } else if let Some(text) = raw.strip_prefix(' ') {
            flush_change_block(
                file,
                &mut removed,
                &mut added,
                &mut left_line,
                &mut right_line,
            );
            file.rows.push(DiffRow::Line(DiffLine {
                left_line: Some(left_line),
                right_line: Some(right_line),
                left_text: text.to_owned(),
                right_text: text.to_owned(),
                left_kind: DiffCellKind::Context,
                right_kind: DiffCellKind::Context,
            }));
            left_line += 1;
            right_line += 1;
        } else {
            flush_change_block(
                file,
                &mut removed,
                &mut added,
                &mut left_line,
                &mut right_line,
            );
            file.rows.push(DiffRow::Meta(raw.to_owned()));
        }
    }

    if let Some(file) = current.as_mut() {
        flush_change_block(
            file,
            &mut removed,
            &mut added,
            &mut left_line,
            &mut right_line,
        );
    }
    push_current_file(&mut files, &mut current);
    files
}

pub fn diff_file_display_label(side_label: &str, path: &str) -> String {
    let label = side_label.trim().trim_end_matches('/');
    let path = path.trim();
    let path = path
        .strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
        .trim_start_matches('/');

    match (label.is_empty(), path.is_empty()) {
        (true, true) => String::new(),
        (true, false) => path.to_owned(),
        (false, true) => format!("[{label}]"),
        (false, false) => format!("[{label}]/{path}"),
    }
}

fn current_file_mut(current: &mut Option<DiffFile>) -> &mut DiffFile {
    if current.is_none() {
        *current = Some(DiffFile {
            left_path: "left".to_owned(),
            right_path: "right".to_owned(),
            rows: Vec::new(),
        });
    }
    current.as_mut().expect("current diff file exists")
}

fn push_current_file(files: &mut Vec<DiffFile>, current: &mut Option<DiffFile>) {
    if let Some(file) = current.take()
        && (!file.rows.is_empty() || !file.left_path.is_empty() || !file.right_path.is_empty())
    {
        files.push(file);
    }
}

fn flush_change_block(
    file: &mut DiffFile,
    removed: &mut Vec<String>,
    added: &mut Vec<String>,
    left_line: &mut usize,
    right_line: &mut usize,
) {
    let max_rows = removed.len().max(added.len());
    for index in 0..max_rows {
        let has_left = index < removed.len();
        let has_right = index < added.len();
        let row = DiffLine {
            left_line: has_left.then_some(*left_line),
            right_line: has_right.then_some(*right_line),
            left_text: removed.get(index).cloned().unwrap_or_default(),
            right_text: added.get(index).cloned().unwrap_or_default(),
            left_kind: if has_left {
                DiffCellKind::Removed
            } else {
                DiffCellKind::Empty
            },
            right_kind: if has_right {
                DiffCellKind::Added
            } else {
                DiffCellKind::Empty
            },
        };
        if has_left {
            *left_line += 1;
        }
        if has_right {
            *right_line += 1;
        }
        file.rows.push(DiffRow::Line(row));
    }
    removed.clear();
    added.clear();
}

fn parse_diff_git_paths(line: &str) -> (String, String) {
    let mut parts = line
        .trim_start_matches("diff --git ")
        .split_whitespace()
        .map(str::to_owned);
    let left = parts.next().unwrap_or_else(|| "left".to_owned());
    let right = parts.next().unwrap_or_else(|| "right".to_owned());
    (left, right)
}

fn parse_hunk_start(line: &str) -> Option<(usize, usize)> {
    let mut left = None;
    let mut right = None;
    for part in line.split_whitespace() {
        if part.starts_with('-') {
            left = parse_hunk_part(part);
        } else if part.starts_with('+') {
            right = parse_hunk_part(part);
        }
    }
    Some((left?, right?))
}

fn parse_hunk_part(part: &str) -> Option<usize> {
    part.get(1..)?
        .split(',')
        .next()
        .and_then(|value| value.parse::<usize>().ok())
}

#[derive(Clone, Copy)]
struct DiffPalette {
    bg: Color32,
    panel: Color32,
    text: Color32,
    muted: Color32,
    added: Color32,
    removed: Color32,
    meta: Color32,
    file_bg: Color32,
    hunk_bg: Color32,
    gutter_bg: Color32,
    added_bg: Color32,
    removed_bg: Color32,
    empty_bg: Color32,
    row_border: Color32,
}

fn diff_palette(theme: DiffTheme) -> DiffPalette {
    match theme {
        DiffTheme::Dark => DiffPalette {
            bg: Color32::from_rgb(24, 27, 31),
            panel: Color32::from_rgb(29, 32, 36),
            text: Color32::from_rgb(222, 229, 238),
            muted: Color32::from_rgb(130, 143, 160),
            added: Color32::from_rgb(154, 220, 170),
            removed: Color32::from_rgb(245, 155, 155),
            meta: Color32::from_rgb(120, 170, 235),
            file_bg: Color32::from_rgb(39, 45, 52),
            hunk_bg: Color32::from_rgb(34, 46, 63),
            gutter_bg: Color32::from_rgb(34, 38, 44),
            added_bg: Color32::from_rgb(24, 58, 40),
            removed_bg: Color32::from_rgb(70, 34, 34),
            empty_bg: Color32::from_rgb(26, 29, 33),
            row_border: Color32::from_rgb(47, 53, 61),
        },
        DiffTheme::Light => DiffPalette {
            bg: Color32::from_rgb(239, 242, 246),
            panel: Color32::from_rgb(253, 254, 255),
            text: Color32::from_rgb(32, 39, 50),
            muted: Color32::from_rgb(105, 116, 132),
            added: Color32::from_rgb(32, 132, 72),
            removed: Color32::from_rgb(180, 54, 48),
            meta: Color32::from_rgb(49, 105, 190),
            file_bg: Color32::from_rgb(226, 232, 240),
            hunk_bg: Color32::from_rgb(229, 239, 255),
            gutter_bg: Color32::from_rgb(241, 244, 248),
            added_bg: Color32::from_rgb(226, 246, 234),
            removed_bg: Color32::from_rgb(255, 235, 232),
            empty_bg: Color32::from_rgb(248, 250, 252),
            row_border: Color32::from_rgb(225, 230, 236),
        },
    }
}

fn show_raw_diff(ui: &mut egui::Ui, diff_text: &str, palette: DiffPalette) {
    for line in diff_text.lines() {
        ui.label(
            RichText::new(line)
                .monospace()
                .font(FontId::monospace(13.0))
                .color(diff_line_color(line, palette)),
        );
    }
}

fn show_side_by_side_diff(
    ui: &mut egui::Ui,
    files: &[DiffFile],
    left_label: &str,
    right_label: &str,
    palette: DiffPalette,
) {
    let gap = 8.0;
    let column_width = ((ui.available_width().max(980.0) - gap) / 2.0).max(460.0);
    let total_width = column_width * 2.0 + gap;

    for (index, file) in files.iter().enumerate() {
        if index > 0 {
            ui.add_space(12.0);
        }
        draw_file_header(
            ui,
            file,
            left_label,
            right_label,
            total_width,
            column_width,
            gap,
            palette,
        );
        for row in &file.rows {
            match row {
                DiffRow::Meta(text) => draw_meta_row(ui, text, total_width, palette),
                DiffRow::Hunk(text) => draw_hunk_row(ui, text, total_width, palette),
                DiffRow::Line(line) => {
                    draw_line_row(ui, line, total_width, column_width, gap, palette)
                }
            }
        }
    }
}

fn draw_file_header(
    ui: &mut egui::Ui,
    file: &DiffFile,
    left_label: &str,
    right_label: &str,
    total_width: f32,
    column_width: f32,
    gap: f32,
    palette: DiffPalette,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total_width, 30.0), Sense::hover());
    ui.painter().rect_filled(rect, 4.0, palette.file_bg);
    let (left_rect, right_rect) = split_columns(rect, column_width, gap);
    draw_header_text(
        ui,
        left_rect,
        &diff_file_display_label(left_label, &file.left_path),
        palette.removed,
        Align2::LEFT_CENTER,
    );
    draw_header_text(
        ui,
        right_rect,
        &diff_file_display_label(right_label, &file.right_path),
        palette.added,
        Align2::LEFT_CENTER,
    );
}

fn draw_meta_row(ui: &mut egui::Ui, text: &str, total_width: f32, palette: DiffPalette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total_width, 22.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, palette.panel);
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .text(
            rect.left_center() + Vec2::new(10.0, 0.0),
            Align2::LEFT_CENTER,
            text,
            FontId::monospace(12.0),
            palette.meta,
        );
}

fn draw_hunk_row(ui: &mut egui::Ui, text: &str, total_width: f32, palette: DiffPalette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total_width, 24.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, palette.hunk_bg);
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .text(
            rect.left_center() + Vec2::new(10.0, 0.0),
            Align2::LEFT_CENTER,
            text,
            FontId::monospace(12.0),
            palette.meta,
        );
}

fn draw_line_row(
    ui: &mut egui::Ui,
    line: &DiffLine,
    total_width: f32,
    column_width: f32,
    gap: f32,
    palette: DiffPalette,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total_width, 24.0), Sense::hover());
    let (left_rect, right_rect) = split_columns(rect, column_width, gap);
    draw_cell(
        ui,
        left_rect,
        line.left_line,
        &line.left_text,
        line.left_kind,
        palette,
    );
    draw_cell(
        ui,
        right_rect,
        line.right_line,
        &line.right_text,
        line.right_kind,
        palette,
    );
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, palette.row_border),
    );
}

fn split_columns(rect: Rect, column_width: f32, gap: f32) -> (Rect, Rect) {
    let left_rect = Rect::from_min_size(rect.left_top(), Vec2::new(column_width, rect.height()));
    let right_rect = Rect::from_min_size(
        Pos2::new(left_rect.right() + gap, rect.top()),
        Vec2::new(column_width, rect.height()),
    );
    (left_rect, right_rect)
}

fn draw_cell(
    ui: &egui::Ui,
    rect: Rect,
    line_number: Option<usize>,
    text: &str,
    kind: DiffCellKind,
    palette: DiffPalette,
) {
    ui.painter().rect_filled(rect, 0.0, cell_bg(kind, palette));
    let gutter_width = 50.0;
    let gutter_rect = Rect::from_min_max(
        rect.left_top(),
        Pos2::new(rect.left() + gutter_width, rect.bottom()),
    );
    ui.painter()
        .rect_filled(gutter_rect, 0.0, palette.gutter_bg);
    if let Some(line_number) = line_number {
        ui.painter().text(
            gutter_rect.right_center() - Vec2::new(8.0, 0.0),
            Align2::RIGHT_CENTER,
            line_number.to_string(),
            FontId::monospace(12.0),
            palette.muted,
        );
    }
    let text_rect = Rect::from_min_max(
        Pos2::new(gutter_rect.right() + 8.0, rect.top()),
        rect.right_bottom(),
    );
    ui.painter()
        .with_clip_rect(text_rect.intersect(ui.clip_rect()))
        .text(
            text_rect.left_center(),
            Align2::LEFT_CENTER,
            text,
            FontId::monospace(13.0),
            cell_text(kind, palette),
        );
}

fn draw_header_text(ui: &egui::Ui, rect: Rect, text: &str, color: Color32, align: Align2) {
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .text(
            rect.left_center() + Vec2::new(10.0, 0.0),
            align,
            text,
            FontId::proportional(13.0),
            color,
        );
}

fn cell_bg(kind: DiffCellKind, palette: DiffPalette) -> Color32 {
    match kind {
        DiffCellKind::Context => palette.panel,
        DiffCellKind::Added => palette.added_bg,
        DiffCellKind::Removed => palette.removed_bg,
        DiffCellKind::Empty => palette.empty_bg,
    }
}

fn cell_text(kind: DiffCellKind, palette: DiffPalette) -> Color32 {
    match kind {
        DiffCellKind::Added => palette.added,
        DiffCellKind::Removed => palette.removed,
        DiffCellKind::Empty => palette.muted,
        DiffCellKind::Context => palette.text,
    }
}

fn diff_line_color(line: &str, palette: DiffPalette) -> Color32 {
    if line.starts_with("@@") || line.starts_with("diff --git") || line.starts_with("index ") {
        palette.meta
    } else if line.starts_with('+') && !line.starts_with("+++") {
        palette.added
    } else if line.starts_with('-') && !line.starts_with("---") {
        palette.removed
    } else {
        palette.text
    }
}

fn apply_diff_theme(ctx: &egui::Context, theme: DiffTheme) {
    let palette = diff_palette(theme);
    let mut visuals = match theme {
        DiffTheme::Dark => egui::Visuals::dark(),
        DiffTheme::Light => egui::Visuals::light(),
    };
    visuals.panel_fill = palette.bg;
    visuals.window_fill = palette.panel;
    visuals.override_text_color = Some(palette.text);
    ctx.set_visuals(visuals);
}

fn dt(language: DiffLanguage, key: &str) -> &'static str {
    match (language, key) {
        (DiffLanguage::Chinese, "empty") => "\u{6ca1}\u{6709}\u{5dee}\u{5f02}",
        (_, "empty") => "No differences",
        _ => "",
    }
}
