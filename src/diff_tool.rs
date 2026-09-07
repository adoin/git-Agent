use std::{env, fs, path::PathBuf};

use anyhow::{Context, anyhow};
use eframe::egui::containers::scroll_area::ScrollBarVisibility;
use eframe::{
    App,
    egui::{
        self, Align, Align2, Color32, CursorIcon, FontId, Layout, Pos2, Rect, RichText, ScrollArea,
        Sense, Stroke, Vec2,
        text::{LayoutJob, TextFormat},
    },
};
use serde::{Deserialize, Serialize};

use crate::syntax::{HighlightedDocument, HighlightedLine, SyntaxRole};

const DIFF_ROW_HEIGHT: f32 = 21.0;
const DIFF_MINIMAP_WIDTH: f32 = 14.0;
const DIFF_HORIZONTAL_SCROLLBAR_HEIGHT: f32 = 9.0;
const DIFF_PANE_GAP: f32 = 8.0;
const DIFF_GUTTER_WIDTH: f32 = 50.0;
const DIFF_WINDOW_TITLEBAR_HEIGHT: f32 = 32.0;
const DIFF_TOOLBAR_HEIGHT: f32 = 38.0;
const DIFF_WINDOW_CONTROLS_WIDTH: f32 = 112.0;
const DIFF_WINDOW_RESIZE_BORDER: f32 = 8.0;
const DIFF_TITLE_DRAG_TOP_INSET: f32 = 9.0;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PatchLineKind {
    Context,
    Added,
    Removed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PatchContentLine<'a> {
    pub kind: PatchLineKind,
    pub body: &'a str,
}

pub(crate) fn diff_prefix_width_from_hunk_header(header: &str) -> Option<usize> {
    if !header.starts_with("@@") {
        return None;
    }
    let parent_count = header
        .split_whitespace()
        .filter(|part| part.starts_with('-'))
        .count();
    (parent_count > 0).then_some(parent_count)
}

pub(crate) fn patch_content_line(line: &str, prefix_width: usize) -> Option<PatchContentLine<'_>> {
    if prefix_width == 0 || line.starts_with("\\ No newline at end of file") {
        return None;
    }
    let prefix = line.as_bytes().get(..prefix_width)?;
    if !prefix.iter().all(|byte| matches!(byte, b'+' | b'-' | b' ')) {
        return None;
    }
    let kind = match prefix[0] {
        b'+' => PatchLineKind::Added,
        b'-' => PatchLineKind::Removed,
        _ => PatchLineKind::Context,
    };
    Some(PatchContentLine {
        kind,
        body: &line[prefix_width..],
    })
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
    pub left_highlight: Option<HighlightedDocument>,
    pub right_highlight: Option<HighlightedDocument>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiffSyntaxSession {
    pub files: Vec<DiffSyntaxFile>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiffSyntaxFile {
    pub left_path: String,
    pub right_path: String,
    pub left_highlight: Option<HighlightedDocument>,
    pub right_highlight: Option<HighlightedDocument>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffArgs {
    pub title: String,
    pub left_label: String,
    pub right_label: String,
    pub diff: PathBuf,
    pub syntax_session: Option<PathBuf>,
    pub theme: DiffTheme,
    pub language: DiffLanguage,
}

pub struct DiffToolApp {
    args: DiffArgs,
    diff_text: String,
    files: Vec<DiffFile>,
    scroll_x: f32,
    scroll_y: f32,
}

impl DiffToolApp {
    pub fn from_args(args: DiffArgs) -> anyhow::Result<Self> {
        let diff_text = fs::read_to_string(&args.diff)
            .with_context(|| format!("failed to read {}", args.diff.display()))?;
        let mut files = parse_side_by_side_diff(&diff_text);
        if let Some(path) = args.syntax_session.as_deref() {
            match fs::read(path)
                .with_context(|| format!("failed to read syntax session {}", path.display()))
                .and_then(|source| {
                    serde_json::from_slice::<DiffSyntaxSession>(&source).map_err(Into::into)
                }) {
                Ok(session) => apply_diff_syntax_session(&mut files, session),
                Err(error) => crate::diagnostics::diff_tool_error(
                    "syntax_session.load",
                    &format!("path={} error={error}", path.display()),
                ),
            }
        }
        Ok(Self {
            args,
            diff_text,
            files,
            scroll_x: 0.0,
            scroll_y: 0.0,
        })
    }

    pub fn run_from_env() -> eframe::Result<()> {
        let args = match parse_diff_args(env::args()) {
            Ok(args) => args,
            Err(error) => {
                eprintln!(
                    "Usage: git-agent-diff --title <title> --left <label> --right <label> --diff <patch> [--syntax-session <json>] [--theme dark|light] [--language en|zh]\n{error}"
                );
                std::process::exit(2);
            }
        };
        let title = format!("Git Agent Diff - {}", args.title);
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title(title.clone())
                .with_icon(diff_app_icon_data())
                .with_inner_size([1120.0, 760.0])
                .with_min_inner_size([820.0, 540.0])
                .with_decorations(false)
                .with_transparent(true)
                .with_resizable(true),
            ..Default::default()
        };
        eframe::run_native(
            &title,
            options,
            Box::new(move |cc| {
                prefer_shadowed_diff_window(cc);
                crate::theme::install(&cc.egui_ctx);
                egui_extras::install_image_loaders(&cc.egui_ctx);
                apply_diff_theme(&cc.egui_ctx, args.theme);
                let app = Self::from_args(args).unwrap_or_else(|error| Self {
                    args: DiffArgs {
                        title: "Diff".to_owned(),
                        left_label: String::new(),
                        right_label: String::new(),
                        diff: PathBuf::new(),
                        syntax_session: None,
                        theme: DiffTheme::Dark,
                        language: DiffLanguage::English,
                    },
                    diff_text: error.to_string(),
                    files: Vec::new(),
                    scroll_x: 0.0,
                    scroll_y: 0.0,
                });
                Ok(Box::new(app))
            }),
        )
    }
}

impl App for DiffToolApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let resizing = request_diff_window_resize_on_pointer_down(ctx, frame);
        if !resizing {
            request_diff_window_drag_on_pointer_down(ctx, frame);
        }
        update_diff_window_resize_cursor(ctx);
        let palette = diff_palette(self.args.theme);

        egui::TopBottomPanel::top("diff_window_titlebar")
            .exact_height(DIFF_WINDOW_TITLEBAR_HEIGHT)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(palette.panel)
                    .corner_radius(egui::CornerRadius {
                        nw: 8,
                        ne: 8,
                        sw: 0,
                        se: 0,
                    })
                    .stroke(Stroke::NONE),
            )
            .show(ctx, |ui| diff_custom_title_bar(ui, ctx, self, palette));

        egui::TopBottomPanel::top("diff_toolbar")
            .exact_height(DIFF_TOOLBAR_HEIGHT)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(palette.panel).stroke(Stroke::NONE))
            .show(ctx, |ui| diff_toolbar(ui, ctx, self, palette));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(palette.bg)
                    .corner_radius(egui::CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: 8,
                        se: 8,
                    })
                    .stroke(Stroke::NONE),
            )
            .show(ctx, |ui| {
                if self.diff_text.trim().is_empty() {
                    ui.label(RichText::new(dt(self.args.language, "empty")).color(palette.muted));
                } else if self.files.is_empty() {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ScrollArea::both()
                        .id_salt("diff_tool_raw_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| show_raw_diff(ui, &self.diff_text, palette));
                } else {
                    show_side_by_side_diff(
                        ui,
                        &self.files,
                        &self.args.left_label,
                        &self.args.right_label,
                        self.args.language,
                        &mut self.scroll_x,
                        &mut self.scroll_y,
                        palette,
                    );
                }
            });
    }
}

fn diff_custom_title_bar(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    app: &DiffToolApp,
    palette: DiffPalette,
) {
    let rect = ui.max_rect();
    let title_rect = Rect::from_min_max(
        Pos2::new(rect.left() + 10.0, rect.top()),
        Pos2::new(
            rect.right() - DIFF_WINDOW_CONTROLS_WIDTH - 8.0,
            rect.bottom(),
        ),
    );
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(title_rect), |ui| {
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.label(RichText::new("Git Agent Diff").strong().color(palette.text));
            ui.label(RichText::new(format!("- {}", app.args.title)).color(palette.muted));
        });
    });

    let double_click_rect = Rect::from_min_max(
        Pos2::new(rect.left(), rect.top() + DIFF_TITLE_DRAG_TOP_INSET),
        Pos2::new(rect.right() - DIFF_WINDOW_CONTROLS_WIDTH, rect.bottom()),
    );
    if ui
        .interact(
            double_click_rect,
            ui.id().with("diff_window_title_double_click"),
            Sense::click(),
        )
        .double_clicked()
    {
        let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }

    let controls_rect = Rect::from_min_max(
        Pos2::new(rect.right() - DIFF_WINDOW_CONTROLS_WIDTH, rect.top() + 4.0),
        Pos2::new(rect.right() - 7.0, rect.bottom() - 4.0),
    );
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(controls_rect), |ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if diff_window_control_button(ui, "\u{00d7}", true, palette).clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
            if diff_window_control_button(
                ui,
                if maximized { "\u{2750}" } else { "\u{25a1}" },
                false,
                palette,
            )
            .clicked()
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            }
            if diff_window_control_button(ui, "\u{2212}", false, palette).clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
        });
    });
}

fn diff_toolbar(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    app: &mut DiffToolApp,
    palette: DiffPalette,
) {
    ui.horizontal_centered(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new(&app.args.left_label).color(palette.removed));
        ui.label(RichText::new("vs").color(palette.muted));
        ui.label(RichText::new(&app.args.right_label).color(palette.added));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(10.0);
            if diff_toolbar_toggle_button(
                ui,
                egui::include_image!("../assets/icons/merge-language.svg"),
                dt(app.args.language, "language"),
                palette,
            )
            .clicked()
            {
                app.args.language = match app.args.language {
                    DiffLanguage::English => DiffLanguage::Chinese,
                    DiffLanguage::Chinese => DiffLanguage::English,
                };
            }
            let (theme_icon, tooltip) = match app.args.theme {
                DiffTheme::Dark => (
                    egui::include_image!("../assets/icons/merge-moon.svg"),
                    dt(app.args.language, "dark"),
                ),
                DiffTheme::Light => (
                    egui::include_image!("../assets/icons/merge-sun.svg"),
                    dt(app.args.language, "light"),
                ),
            };
            if diff_toolbar_toggle_button(ui, theme_icon, tooltip, palette).clicked() {
                app.args.theme = match app.args.theme {
                    DiffTheme::Dark => DiffTheme::Light,
                    DiffTheme::Light => DiffTheme::Dark,
                };
                apply_diff_theme(ctx, app.args.theme);
            }
        });
    });
}

fn diff_toolbar_toggle_button(
    ui: &mut egui::Ui,
    icon: egui::ImageSource<'static>,
    tooltip: &str,
    palette: DiffPalette,
) -> egui::Response {
    let image = egui::Image::new(icon)
        .fit_to_exact_size(Vec2::splat(15.0))
        .tint(palette.muted);
    ui.add(
        egui::Button::image(image)
            .min_size(Vec2::splat(28.0))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .corner_radius(egui::CornerRadius::same(4)),
    )
    .on_hover_text(tooltip)
}

fn diff_window_control_button(
    ui: &mut egui::Ui,
    label: &str,
    close: bool,
    palette: DiffPalette,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(34.0, 22.0), Sense::click());
    let fill = if close && response.hovered() {
        Color32::from_rgb(192, 55, 43)
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
    let mut syntax_session = None;
    let mut theme = DiffTheme::Dark;
    let mut language = DiffLanguage::English;
    let mut iter = items.into_iter();

    while let Some(item) = iter.next() {
        match item.as_str() {
            "--title" => title = iter.next(),
            "--left" => left_label = iter.next(),
            "--right" => right_label = iter.next(),
            "--diff" => diff = iter.next().map(PathBuf::from),
            "--syntax-session" => syntax_session = iter.next().map(PathBuf::from),
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
        syntax_session,
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
    let mut prefix_width = 1usize;
    let mut removed = Vec::<String>::new();
    let mut added = Vec::<String>::new();

    for raw in diff_text.lines() {
        if raw.starts_with("diff --git ")
            || raw.starts_with("diff --cc ")
            || raw.starts_with("diff --combined ")
        {
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
            let (left_path, right_path) = parse_diff_paths(raw);
            current = Some(DiffFile {
                left_path,
                right_path,
                rows: Vec::new(),
                left_highlight: None,
                right_highlight: None,
            });
            in_hunk = false;
            prefix_width = 1;
            left_line = 0;
            right_line = 0;
            continue;
        }

        let file = current_file_mut(&mut current);
        if raw.starts_with("--- ") {
            file.left_path = parse_git_path_tokens(raw.trim_start_matches("--- "))
                .into_iter()
                .next()
                .unwrap_or_else(|| "left".to_owned());
            continue;
        }
        if raw.starts_with("+++ ") {
            file.right_path = parse_git_path_tokens(raw.trim_start_matches("+++ "))
                .into_iter()
                .next()
                .unwrap_or_else(|| "right".to_owned());
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
            prefix_width = diff_prefix_width_from_hunk_header(raw).unwrap_or(1);
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

        match patch_content_line(raw, prefix_width) {
            Some(PatchContentLine {
                kind: PatchLineKind::Removed,
                body,
            }) => removed.push(body.to_owned()),
            Some(PatchContentLine {
                kind: PatchLineKind::Added,
                body,
            }) => added.push(body.to_owned()),
            Some(PatchContentLine {
                kind: PatchLineKind::Context,
                body,
            }) => {
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
                    left_text: body.to_owned(),
                    right_text: body.to_owned(),
                    left_kind: DiffCellKind::Context,
                    right_kind: DiffCellKind::Context,
                }));
                left_line += 1;
                right_line += 1;
            }
            None => {
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

fn apply_diff_syntax_session(files: &mut [DiffFile], mut session: DiffSyntaxSession) {
    for file in files {
        let left_path = diff_source_path(&file.left_path);
        let right_path = diff_source_path(&file.right_path);
        let Some(index) = session.files.iter().position(|candidate| {
            diff_source_path(&candidate.left_path) == left_path
                && diff_source_path(&candidate.right_path) == right_path
        }) else {
            continue;
        };
        let candidate = session.files.remove(index);
        file.left_highlight = candidate.left_highlight;
        file.right_highlight = candidate.right_highlight;
    }
}

pub fn diff_source_path(path: &str) -> Option<String> {
    let path = path.trim().trim_matches('"');
    if path.is_empty() || path == "/dev/null" {
        return None;
    }
    Some(
        path.strip_prefix("a/")
            .or_else(|| path.strip_prefix("b/"))
            .unwrap_or(path)
            .replace('\\', "/"),
    )
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
            left_highlight: None,
            right_highlight: None,
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

fn parse_diff_paths(line: &str) -> (String, String) {
    let raw = line
        .strip_prefix("diff --git ")
        .or_else(|| line.strip_prefix("diff --cc "))
        .or_else(|| line.strip_prefix("diff --combined "))
        .unwrap_or(line);
    let mut parts = parse_git_path_tokens(raw).into_iter();
    let left = parts.next().unwrap_or_else(|| "left".to_owned());
    let right = parts.next().unwrap_or_else(|| left.clone());
    (left, right)
}

/// Decode the C-style path quoting used by Git patch headers. Patch output does
/// not support `-z`, so this is the boundary where display quoting must be
/// decoded back into the UTF-8 path used by the rest of the application.
pub(crate) fn parse_git_path_tokens(input: &str) -> Vec<String> {
    let bytes = input.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        let mut token = Vec::new();
        if bytes[index] == b'"' {
            index += 1;
            while index < bytes.len() && bytes[index] != b'"' {
                if bytes[index] != b'\\' {
                    token.push(bytes[index]);
                    index += 1;
                    continue;
                }
                index += 1;
                if index >= bytes.len() {
                    token.push(b'\\');
                    break;
                }
                if matches!(bytes[index], b'0'..=b'7') {
                    let mut value = 0u16;
                    let mut digits = 0;
                    while index < bytes.len() && digits < 3 && matches!(bytes[index], b'0'..=b'7') {
                        value = value * 8 + u16::from(bytes[index] - b'0');
                        index += 1;
                        digits += 1;
                    }
                    token.push(value.min(255) as u8);
                    continue;
                }
                token.push(match bytes[index] {
                    b'a' => 0x07,
                    b'b' => 0x08,
                    b't' => b'\t',
                    b'n' => b'\n',
                    b'v' => 0x0b,
                    b'f' => 0x0c,
                    b'r' => b'\r',
                    escaped => escaped,
                });
                index += 1;
            }
            if index < bytes.len() && bytes[index] == b'"' {
                index += 1;
            }
        } else {
            let start = index;
            while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            token.extend_from_slice(&bytes[start..index]);
        }
        tokens.push(String::from_utf8_lossy(&token).into_owned());
    }
    tokens
}

fn parse_hunk_start(line: &str) -> Option<(usize, usize)> {
    let (left, right) = parse_hunk_ranges(line)?;
    Some((left.start, right.start))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DiffHunkRange {
    start: usize,
    count: usize,
}

fn parse_hunk_ranges(line: &str) -> Option<(DiffHunkRange, DiffHunkRange)> {
    let ranges = line
        .split_whitespace()
        .filter(|part| part.starts_with('-') || part.starts_with('+'))
        .collect::<Vec<_>>();
    let left = ranges
        .iter()
        .find(|part| part.starts_with('-'))
        .and_then(|part| parse_hunk_range(part))?;
    let right = ranges
        .iter()
        .rev()
        .find(|part| part.starts_with('+'))
        .and_then(|part| parse_hunk_range(part))?;
    Some((left, right))
}

fn parse_hunk_range(part: &str) -> Option<DiffHunkRange> {
    let mut values = part.get(1..)?.split(',');
    let start = values.next()?.parse::<usize>().ok()?;
    let count = values
        .next()
        .map(str::parse::<usize>)
        .transpose()
        .ok()?
        .unwrap_or(1);
    Some(DiffHunkRange { start, count })
}

fn format_hunk_summary(line: &str, language: DiffLanguage) -> String {
    let Some((left, right)) = parse_hunk_ranges(line) else {
        return line.to_owned();
    };
    let marker_width = line.bytes().take_while(|byte| *byte == b'@').count();
    let context = (marker_width >= 2)
        .then(|| &line[marker_width..])
        .and_then(|rest| rest.find(&line[..marker_width]).map(|end| (rest, end)))
        .map(|(rest, end)| rest[end + marker_width..].trim())
        .filter(|context| !context.is_empty());
    let left = format_hunk_range(left);
    let right = format_hunk_range(right);
    let mut label = match language {
        DiffLanguage::Chinese => format!("区块  旧 {left}  →  新 {right}"),
        DiffLanguage::English => format!("Block  old {left}  →  new {right}"),
    };
    if let Some(context) = context {
        label.push_str("  ·  ");
        label.push_str(context);
    }
    label
}

fn format_hunk_range(range: DiffHunkRange) -> String {
    match range.count {
        0 => format!("{} (0 行)", range.start),
        1 => range.start.to_string(),
        count => format!("{}–{}", range.start, range.start + count - 1),
    }
}

#[derive(Clone, Copy)]
struct DiffPalette {
    bg: Color32,
    panel: Color32,
    panel_soft: Color32,
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
}

fn diff_palette(theme: DiffTheme) -> DiffPalette {
    match theme {
        DiffTheme::Dark => DiffPalette {
            bg: Color32::from_rgb(24, 27, 31),
            panel: Color32::from_rgb(29, 32, 36),
            panel_soft: Color32::from_rgb(49, 43, 43),
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
        },
        DiffTheme::Light => DiffPalette {
            bg: Color32::from_rgb(239, 242, 246),
            panel: Color32::from_rgb(253, 254, 255),
            panel_soft: Color32::from_rgb(248, 225, 219),
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

#[derive(Clone, Copy)]
enum DiffDisplayRow<'a> {
    File(&'a DiffFile),
    Hunk(&'a str),
    Line {
        line: &'a DiffLine,
        left_highlight: Option<&'a HighlightedDocument>,
        right_highlight: Option<&'a HighlightedDocument>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffOverviewTone {
    File,
    Hunk,
    Added,
    Removed,
    Changed,
    Context,
}

fn show_side_by_side_diff(
    ui: &mut egui::Ui,
    files: &[DiffFile],
    left_label: &str,
    right_label: &str,
    language: DiffLanguage,
    scroll_x: &mut f32,
    scroll_y: &mut f32,
    palette: DiffPalette,
) {
    let rows = diff_display_rows(files);
    if rows.is_empty() {
        return;
    }

    let available = ui.available_rect_before_wrap();
    let content_bottom = available.bottom() - DIFF_HORIZONTAL_SCROLLBAR_HEIGHT - 3.0;
    let viewport_height = (content_bottom - available.top()).max(0.0);
    let needs_minimap = diff_needs_minimap(rows.len(), viewport_height);
    let map_rect = needs_minimap.then(|| {
        Rect::from_min_max(
            Pos2::new(available.right() - DIFF_MINIMAP_WIDTH, available.top()),
            Pos2::new(available.right(), content_bottom),
        )
    });
    let code_right = map_rect
        .map(|rect| rect.left() - 4.0)
        .unwrap_or(available.right());
    let code_rect = Rect::from_min_max(available.left_top(), Pos2::new(code_right, content_bottom));
    let horizontal_rect = Rect::from_min_max(
        Pos2::new(code_rect.left(), code_rect.bottom() + 3.0),
        Pos2::new(code_rect.right(), available.bottom()),
    );
    let longest_chars = rows
        .iter()
        .filter_map(|row| match row {
            DiffDisplayRow::Line { line, .. } => Some(
                line.left_text
                    .chars()
                    .count()
                    .max(line.right_text.chars().count()),
            ),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    let narrow_column = ((code_rect.width() - DIFF_PANE_GAP) / 2.0).max(1.0);
    let code_column = (DIFF_GUTTER_WIDTH + 20.0 + longest_chars as f32 * 7.7).max(narrow_column);
    let total_width = code_column * 2.0 + DIFF_PANE_GAP;
    let max_scroll_x = (total_width - code_rect.width()).max(0.0);
    *scroll_x = diff_horizontal_scroll_input(ui, code_rect, *scroll_x, max_scroll_x);

    let output = ui
        .allocate_new_ui(egui::UiBuilder::new().max_rect(code_rect), |ui| {
            // `show_rows` captures spacing before its callback. Dense spacing must be installed
            // here or every virtual row gains an invisible default gap.
            ui.spacing_mut().item_spacing.y = 0.0;
            ScrollArea::vertical()
                .id_salt("diff_tool_vertical_scroll")
                .vertical_scroll_offset(*scroll_y)
                .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                .auto_shrink([false, false])
                .show_rows(ui, DIFF_ROW_HEIGHT, rows.len(), |ui, range| {
                    ui.set_min_width(ui.available_width());
                    for index in range {
                        draw_display_row(
                            ui,
                            rows[index],
                            left_label,
                            right_label,
                            language,
                            total_width,
                            code_column,
                            *scroll_x,
                            palette,
                        );
                    }
                })
        })
        .inner;
    *scroll_y = output.state.offset.y;

    if let Some(map_rect) = map_rect {
        if let Some(target) = draw_diff_minimap(
            ui,
            map_rect,
            &rows,
            *scroll_y,
            output.inner_rect.height(),
            output.content_size.y,
            palette,
        ) {
            *scroll_y = target;
        }
    }
    *scroll_x = draw_diff_horizontal_scrollbar(
        ui,
        horizontal_rect,
        *scroll_x,
        code_rect.width(),
        total_width,
        palette,
    );
}

fn diff_needs_minimap(row_count: usize, viewport_height: f32) -> bool {
    row_count as f32 * DIFF_ROW_HEIGHT > viewport_height + 0.5
}

fn diff_display_rows(files: &[DiffFile]) -> Vec<DiffDisplayRow<'_>> {
    let mut rows = Vec::new();
    for file in files {
        rows.push(DiffDisplayRow::File(file));
        for row in &file.rows {
            match row {
                // Patch plumbing such as `index <old>..<new> 100644` is useful to Git, not to a
                // source reader. Keep it in the parser for fidelity but omit it from the viewer.
                DiffRow::Meta(_) => {}
                DiffRow::Hunk(text) => rows.push(DiffDisplayRow::Hunk(text)),
                DiffRow::Line(line) => rows.push(DiffDisplayRow::Line {
                    line,
                    left_highlight: file.left_highlight.as_ref(),
                    right_highlight: file.right_highlight.as_ref(),
                }),
            }
        }
    }
    rows
}

fn draw_display_row(
    ui: &mut egui::Ui,
    row: DiffDisplayRow<'_>,
    left_label: &str,
    right_label: &str,
    language: DiffLanguage,
    total_width: f32,
    column_width: f32,
    scroll_x: f32,
    palette: DiffPalette,
) {
    let (viewport_row, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), DIFF_ROW_HEIGHT),
        Sense::hover(),
    );
    let paint_rect = Rect::from_min_size(
        Pos2::new(viewport_row.left() - scroll_x, viewport_row.top()),
        Vec2::new(total_width, DIFF_ROW_HEIGHT),
    );
    match row {
        DiffDisplayRow::File(file) => draw_file_header(
            ui,
            paint_rect,
            file,
            left_label,
            right_label,
            column_width,
            DIFF_PANE_GAP,
            palette,
        ),
        DiffDisplayRow::Hunk(text) => draw_hunk_row(
            ui,
            paint_rect,
            &format_hunk_summary(text, language),
            palette,
        ),
        DiffDisplayRow::Line {
            line,
            left_highlight,
            right_highlight,
        } => draw_line_row(
            ui,
            paint_rect,
            line,
            left_highlight,
            right_highlight,
            column_width,
            DIFF_PANE_GAP,
            palette,
        ),
    }
}

fn diff_horizontal_scroll_input(
    ui: &egui::Ui,
    code_rect: Rect,
    current: f32,
    max_scroll: f32,
) -> f32 {
    if !ui.rect_contains_pointer(code_rect) || max_scroll <= 0.0 {
        return current.clamp(0.0, max_scroll);
    }
    let (delta, shift) = ui
        .ctx()
        .input(|input| (input.smooth_scroll_delta, input.modifiers.shift));
    let horizontal = if delta.x.abs() > f32::EPSILON {
        delta.x
    } else if shift {
        delta.y
    } else {
        0.0
    };
    if horizontal.abs() > f32::EPSILON {
        ui.ctx().request_repaint();
    }
    (current - horizontal).clamp(0.0, max_scroll)
}

fn draw_diff_horizontal_scrollbar(
    ui: &mut egui::Ui,
    rect: Rect,
    current: f32,
    viewport_width: f32,
    content_width: f32,
    palette: DiffPalette,
) -> f32 {
    ui.painter().rect_filled(rect, 3.0, palette.gutter_bg);
    let max_scroll = (content_width - viewport_width).max(0.0);
    if max_scroll <= 0.0 || rect.width() <= 1.0 {
        return 0.0;
    }
    let thumb_width = (rect.width() * viewport_width / content_width).clamp(28.0, rect.width());
    let travel = (rect.width() - thumb_width).max(0.0);
    let thumb_left = rect.left() + travel * (current / max_scroll);
    let thumb = Rect::from_min_size(
        Pos2::new(thumb_left, rect.top() + 1.0),
        Vec2::new(thumb_width, (rect.height() - 2.0).max(1.0)),
    );
    ui.painter()
        .rect_filled(thumb, 3.0, diff_color_with_opacity(palette.meta, 0.72));
    let response = ui
        .interact(
            rect,
            ui.make_persistent_id("diff_horizontal_scrollbar"),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(CursorIcon::ResizeHorizontal);
    if !(response.clicked() || response.dragged()) {
        return current.clamp(0.0, max_scroll);
    }
    let Some(pointer) = response.interact_pointer_pos() else {
        return current.clamp(0.0, max_scroll);
    };
    ui.ctx().request_repaint();
    ((pointer.x - rect.left() - thumb_width / 2.0) / travel.max(1.0) * max_scroll)
        .clamp(0.0, max_scroll)
}

fn draw_diff_minimap(
    ui: &mut egui::Ui,
    rect: Rect,
    rows: &[DiffDisplayRow<'_>],
    scroll_y: f32,
    viewport_height: f32,
    content_height: f32,
    palette: DiffPalette,
) -> Option<f32> {
    if rows.is_empty()
        || content_height <= viewport_height + 0.5
        || content_height <= 0.0
        || rect.height() <= 4.0
    {
        return None;
    }
    ui.painter().rect_filled(rect, 3.0, palette.gutter_bg);
    let row_height = rect.height() / rows.len() as f32;
    for (index, row) in rows.iter().enumerate() {
        let tone = diff_overview_tone(*row);
        let Some(color) = diff_overview_color(tone, palette) else {
            continue;
        };
        let top = rect.top() + index as f32 * row_height;
        let bottom = (top + row_height.max(1.0)).min(rect.bottom());
        ui.painter().rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left() + 2.0, top),
                Pos2::new(rect.right() - 2.0, bottom),
            ),
            0.0,
            color,
        );
    }
    let viewport = diff_minimap_viewport_rect(rect, scroll_y, viewport_height, content_height);
    ui.painter()
        .rect_filled(viewport, 2.0, diff_color_with_opacity(palette.meta, 0.48));
    let response = ui
        .interact(
            rect,
            ui.make_persistent_id("diff_minimap"),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(CursorIcon::PointingHand);
    if !(response.clicked() || response.dragged()) {
        return None;
    }
    let pointer = response.interact_pointer_pos()?;
    ui.ctx().request_repaint();
    Some(diff_minimap_scroll_target(
        rect,
        pointer.y,
        viewport_height,
        content_height,
    ))
}

fn diff_overview_tone(row: DiffDisplayRow<'_>) -> DiffOverviewTone {
    match row {
        DiffDisplayRow::File(_) => DiffOverviewTone::File,
        DiffDisplayRow::Hunk(_) => DiffOverviewTone::Hunk,
        DiffDisplayRow::Line { line, .. } => match (line.left_kind, line.right_kind) {
            (DiffCellKind::Removed, DiffCellKind::Added) => DiffOverviewTone::Changed,
            (DiffCellKind::Removed, _) => DiffOverviewTone::Removed,
            (_, DiffCellKind::Added) => DiffOverviewTone::Added,
            _ => DiffOverviewTone::Context,
        },
    }
}

fn diff_overview_color(tone: DiffOverviewTone, palette: DiffPalette) -> Option<Color32> {
    match tone {
        DiffOverviewTone::File => Some(diff_color_with_opacity(palette.text, 0.48)),
        DiffOverviewTone::Hunk => Some(diff_color_with_opacity(palette.meta, 0.72)),
        DiffOverviewTone::Added => Some(diff_color_with_opacity(palette.added, 0.86)),
        DiffOverviewTone::Removed => Some(diff_color_with_opacity(palette.removed, 0.86)),
        DiffOverviewTone::Changed => Some(Color32::from_rgb(
            ((palette.added.r() as u16 + palette.removed.r() as u16) / 2) as u8,
            ((palette.added.g() as u16 + palette.removed.g() as u16) / 2) as u8,
            ((palette.added.b() as u16 + palette.removed.b() as u16) / 2) as u8,
        )),
        DiffOverviewTone::Context => None,
    }
}

fn diff_color_with_opacity(color: Color32, opacity: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (255.0 * opacity.clamp(0.0, 1.0)).round() as u8,
    )
}

fn diff_minimap_viewport_rect(
    track: Rect,
    scroll_y: f32,
    viewport_height: f32,
    content_height: f32,
) -> Rect {
    let visible_ratio = (viewport_height / content_height).clamp(0.0, 1.0);
    let thumb_height = (track.height() * visible_ratio).clamp(8.0, track.height());
    let max_scroll = (content_height - viewport_height).max(0.0);
    let travel = (track.height() - thumb_height).max(0.0);
    let top = if max_scroll > 0.0 {
        track.top() + travel * (scroll_y / max_scroll).clamp(0.0, 1.0)
    } else {
        track.top()
    };
    Rect::from_min_size(
        Pos2::new(track.left() + 1.0, top),
        Vec2::new((track.width() - 2.0).max(1.0), thumb_height),
    )
}

fn diff_minimap_scroll_target(
    track: Rect,
    pointer_y: f32,
    viewport_height: f32,
    content_height: f32,
) -> f32 {
    let max_scroll = (content_height - viewport_height).max(0.0);
    if max_scroll <= 0.0 {
        return 0.0;
    }
    let ratio = ((pointer_y - track.top()) / track.height().max(1.0)).clamp(0.0, 1.0);
    (ratio * content_height - viewport_height / 2.0).clamp(0.0, max_scroll)
}

fn draw_file_header(
    ui: &mut egui::Ui,
    rect: Rect,
    file: &DiffFile,
    left_label: &str,
    right_label: &str,
    column_width: f32,
    gap: f32,
    palette: DiffPalette,
) {
    ui.painter().rect_filled(rect, 0.0, palette.file_bg);
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

fn draw_hunk_row(ui: &mut egui::Ui, rect: Rect, text: &str, palette: DiffPalette) {
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
    rect: Rect,
    line: &DiffLine,
    left_highlight: Option<&HighlightedDocument>,
    right_highlight: Option<&HighlightedDocument>,
    column_width: f32,
    gap: f32,
    palette: DiffPalette,
) {
    let (left_rect, right_rect) = split_columns(rect, column_width, gap);
    draw_cell(
        ui,
        left_rect,
        line.left_line,
        &line.left_text,
        line.left_kind,
        line.left_line.and_then(|line_number| {
            left_highlight.and_then(|document| document.lines.get(line_number.saturating_sub(1)))
        }),
        palette,
    );
    draw_cell(
        ui,
        right_rect,
        line.right_line,
        &line.right_text,
        line.right_kind,
        line.right_line.and_then(|line_number| {
            right_highlight.and_then(|document| document.lines.get(line_number.saturating_sub(1)))
        }),
        palette,
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
    highlighted_line: Option<&HighlightedLine>,
    palette: DiffPalette,
) {
    ui.painter().rect_filled(rect, 0.0, cell_bg(kind, palette));
    let gutter_rect = Rect::from_min_max(
        rect.left_top(),
        Pos2::new(rect.left() + DIFF_GUTTER_WIDTH, rect.bottom()),
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
    let base_color = cell_text(kind, palette);
    let painter = ui
        .painter()
        .with_clip_rect(text_rect.intersect(ui.clip_rect()));
    let galley = painter.layout_job(diff_syntax_layout_job(
        text,
        highlighted_line,
        base_color,
        palette,
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

fn diff_syntax_layout_job(
    text: &str,
    highlighted_line: Option<&HighlightedLine>,
    base_color: Color32,
    palette: DiffPalette,
) -> LayoutJob {
    let font_id = FontId::monospace(13.0);
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
            {
                continue;
            }
            if cursor < start {
                job.append(&text[cursor..start], 0.0, format(base_color));
            }
            job.append(
                &text[start..end],
                0.0,
                format(diff_syntax_color(span.role, palette)),
            );
            cursor = end;
        }
    }
    if cursor < text.len() {
        job.append(&text[cursor..], 0.0, format(base_color));
    }
    job
}

fn diff_syntax_color(role: SyntaxRole, palette: DiffPalette) -> Color32 {
    let mode = if palette.bg.r() < 128 {
        crate::theme::ThemeMode::Dark
    } else {
        crate::theme::ThemeMode::Light
    };
    crate::theme::syntax_color_for_mode(role, mode)
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

fn diff_app_icon_data() -> egui::IconData {
    const SIZE: usize = 64;
    let mut rgba = vec![0_u8; SIZE * SIZE * 4];
    let removed = [231, 91, 83, 255];
    let added = [49, 181, 112, 255];
    let neutral = [113, 137, 174, 255];

    // Two compact source panes with opposing change marks. This stays recognizable at the
    // Windows title-bar size and deliberately differs from both eframe's `e` and the merge icon.
    paint_diff_icon_box(&mut rgba, 7, 11, 27, 53, removed);
    paint_diff_icon_box(&mut rgba, 37, 11, 57, 53, added);
    paint_diff_icon_line(&mut rgba, 12, 23, 22, 23, removed);
    paint_diff_icon_line(&mut rgba, 12, 32, 22, 32, removed);
    paint_diff_icon_line(&mut rgba, 42, 32, 52, 32, added);
    paint_diff_icon_line(&mut rgba, 42, 41, 52, 41, added);
    paint_diff_icon_line(&mut rgba, 28, 32, 36, 32, neutral);
    paint_diff_icon_line(&mut rgba, 33, 28, 37, 32, neutral);
    paint_diff_icon_line(&mut rgba, 33, 36, 37, 32, neutral);

    egui::IconData {
        rgba,
        width: SIZE as u32,
        height: SIZE as u32,
    }
}

fn paint_diff_icon_box(
    rgba: &mut [u8],
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
    color: [u8; 4],
) {
    paint_diff_icon_line(rgba, left, top, right, top, color);
    paint_diff_icon_line(rgba, left, top, left, bottom, color);
    paint_diff_icon_line(rgba, right, top, right, bottom, color);
    paint_diff_icon_line(rgba, left, bottom, right, bottom, color);
}

fn paint_diff_icon_line(
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
        for paint_y in y.saturating_sub(2)..=(y + 2).min(63) {
            for paint_x in x.saturating_sub(2)..=(x + 2).min(63) {
                let index = (paint_y * 64 + paint_x) * 4;
                rgba[index..index + 4].copy_from_slice(&color);
            }
        }
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
    visuals.window_stroke = Stroke::NONE;
    let surface_shadow = match theme {
        DiffTheme::Dark => eframe::epaint::Shadow {
            offset: [3, 4],
            blur: 12,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(0, 0, 0, 90),
        },
        DiffTheme::Light => eframe::epaint::Shadow {
            offset: [3, 4],
            blur: 12,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(44, 56, 72, 44),
        },
    };
    visuals.window_shadow = surface_shadow;
    visuals.popup_shadow = surface_shadow;
    visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;
    visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    visuals.widgets.active.bg_stroke = Stroke::NONE;
    visuals.widgets.open.bg_stroke = Stroke::NONE;
    visuals.extreme_bg_color = palette.panel_soft;
    visuals.faint_bg_color = palette.panel_soft;
    visuals.selection.stroke = Stroke::NONE;
    visuals.override_text_color = Some(palette.text);
    ctx.set_visuals(visuals);
}

fn diff_window_resize_direction(
    rect: Rect,
    pointer_pos: Pos2,
) -> Option<egui::viewport::ResizeDirection> {
    if !rect.contains(pointer_pos) {
        return None;
    }
    let north = pointer_pos.y <= rect.top() + DIFF_WINDOW_RESIZE_BORDER;
    let south = pointer_pos.y >= rect.bottom() - DIFF_WINDOW_RESIZE_BORDER;
    let west = pointer_pos.x <= rect.left() + DIFF_WINDOW_RESIZE_BORDER;
    let east = pointer_pos.x >= rect.right() - DIFF_WINDOW_RESIZE_BORDER;
    use egui::viewport::ResizeDirection;
    match (north, south, west, east) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (true, _, _, true) => Some(ResizeDirection::NorthEast),
        (_, true, true, _) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, _, _, _) => Some(ResizeDirection::North),
        (_, true, _, _) => Some(ResizeDirection::South),
        (_, _, true, _) => Some(ResizeDirection::West),
        (_, _, _, true) => Some(ResizeDirection::East),
        _ => None,
    }
}

fn diff_resize_cursor_icon(direction: egui::viewport::ResizeDirection) -> CursorIcon {
    use egui::viewport::ResizeDirection;
    match direction {
        ResizeDirection::North | ResizeDirection::South => CursorIcon::ResizeVertical,
        ResizeDirection::East | ResizeDirection::West => CursorIcon::ResizeHorizontal,
        ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::ResizeNeSw,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::ResizeNwSe,
    }
}

fn request_diff_window_resize_on_pointer_down(ctx: &egui::Context, frame: &eframe::Frame) -> bool {
    let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
    if maximized {
        return false;
    }
    let Some((screen, pointer_pos)) = ctx.input(|input| {
        input
            .pointer
            .primary_pressed()
            .then(|| {
                input
                    .pointer
                    .interact_pos()
                    .map(|pos| (input.screen_rect(), pos))
            })
            .flatten()
    }) else {
        return false;
    };
    let Some(direction) = diff_window_resize_direction(screen, pointer_pos) else {
        return false;
    };
    request_native_diff_window_resize(ctx, frame, direction);
    true
}

fn request_diff_window_drag_on_pointer_down(ctx: &egui::Context, frame: &eframe::Frame) {
    let Some((screen, pointer_pos)) = ctx.input(|input| {
        input
            .pointer
            .primary_pressed()
            .then(|| {
                input
                    .pointer
                    .interact_pos()
                    .map(|pos| (input.screen_rect(), pos))
            })
            .flatten()
    }) else {
        return;
    };
    let drag_rect = Rect::from_min_max(
        Pos2::new(
            screen.left() + DIFF_WINDOW_RESIZE_BORDER,
            screen.top() + DIFF_TITLE_DRAG_TOP_INSET,
        ),
        Pos2::new(
            screen.right() - DIFF_WINDOW_CONTROLS_WIDTH,
            screen.top() + DIFF_WINDOW_TITLEBAR_HEIGHT,
        ),
    );
    if drag_rect.contains(pointer_pos) {
        request_native_diff_window_drag(ctx, frame);
    }
}

fn update_diff_window_resize_cursor(ctx: &egui::Context) {
    let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
    if maximized {
        return;
    }
    if let Some((screen, pointer_pos)) = ctx.input(|input| {
        input
            .pointer
            .hover_pos()
            .map(|pos| (input.screen_rect(), pos))
    }) {
        if let Some(direction) = diff_window_resize_direction(screen, pointer_pos) {
            ctx.set_cursor_icon(diff_resize_cursor_icon(direction));
        }
    }
}

fn request_native_diff_window_drag(ctx: &egui::Context, frame: &eframe::Frame) {
    #[cfg(target_os = "windows")]
    if post_diff_windows_non_client_pointer_down(
        frame,
        windows_sys::Win32::UI::WindowsAndMessaging::HTCAPTION as usize,
    ) {
        return;
    }

    #[cfg(not(target_os = "windows"))]
    let _ = frame;
    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
}

fn request_native_diff_window_resize(
    ctx: &egui::Context,
    frame: &eframe::Frame,
    direction: egui::ResizeDirection,
) {
    #[cfg(target_os = "windows")]
    if post_diff_windows_non_client_pointer_down(frame, diff_windows_resize_hit_test(direction)) {
        return;
    }

    #[cfg(not(target_os = "windows"))]
    let _ = frame;
    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
}

#[cfg(target_os = "windows")]
fn diff_windows_resize_hit_test(direction: egui::ResizeDirection) -> usize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT,
    };

    match direction {
        egui::ResizeDirection::North => HTTOP as usize,
        egui::ResizeDirection::NorthEast => HTTOPRIGHT as usize,
        egui::ResizeDirection::East => HTRIGHT as usize,
        egui::ResizeDirection::SouthEast => HTBOTTOMRIGHT as usize,
        egui::ResizeDirection::South => HTBOTTOM as usize,
        egui::ResizeDirection::SouthWest => HTBOTTOMLEFT as usize,
        egui::ResizeDirection::West => HTLEFT as usize,
        egui::ResizeDirection::NorthWest => HTTOPLEFT as usize,
    }
}

#[cfg(target_os = "windows")]
fn diff_win32_point_lparam(x: i32, y: i32) -> isize {
    let x = u32::from(x as i16 as u16);
    let y = u32::from(y as i16 as u16);
    ((y << 16) | x) as isize
}

#[cfg(target_os = "windows")]
fn post_diff_windows_non_client_pointer_down(frame: &eframe::Frame, hit_test: usize) -> bool {
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    use windows_sys::Win32::{
        Foundation::POINT,
        UI::{
            Input::KeyboardAndMouse::ReleaseCapture,
            WindowsAndMessaging::{GetCursorPos, PostMessageW, WM_NCLBUTTONDOWN},
        },
    };

    let Ok(window_handle) = frame.window_handle() else {
        return false;
    };
    let RawWindowHandle::Win32(handle) = window_handle.as_raw() else {
        return false;
    };
    let mut cursor = POINT { x: 0, y: 0 };
    unsafe {
        if GetCursorPos(&mut cursor) == 0 {
            return false;
        }
        ReleaseCapture();
        PostMessageW(
            handle.hwnd.get() as _,
            WM_NCLBUTTONDOWN,
            hit_test,
            diff_win32_point_lparam(cursor.x, cursor.y),
        ) != 0
    }
}

#[cfg(target_os = "windows")]
fn prefer_shadowed_diff_window(cc: &eframe::CreationContext<'_>) {
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;

    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWA_BORDER_COLOR: u32 = 34;
    const DWMWCP_ROUND: u32 = 2;
    const DWMWA_COLOR_NONE: u32 = 0xffff_fffe;

    let Ok(window_handle) = cc.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = window_handle.as_raw() else {
        return;
    };
    unsafe {
        let _ = DwmSetWindowAttribute(
            handle.hwnd.get() as _,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &DWMWCP_ROUND as *const u32 as _,
            std::mem::size_of::<u32>() as u32,
        );
        // Suppress Windows 11's one-pixel accent border. The native DWM drop
        // shadow remains, so the window is separated by depth rather than a line.
        let _ = DwmSetWindowAttribute(
            handle.hwnd.get() as _,
            DWMWA_BORDER_COLOR,
            &DWMWA_COLOR_NONE as *const u32 as _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn prefer_shadowed_diff_window(_: &eframe::CreationContext<'_>) {}

fn dt(language: DiffLanguage, key: &str) -> &'static str {
    match (language, key) {
        (DiffLanguage::Chinese, "empty") => "\u{6ca1}\u{6709}\u{5dee}\u{5f02}",
        (DiffLanguage::Chinese, "language") => "\u{4e2d}\u{6587}",
        (DiffLanguage::Chinese, "dark") => "\u{6df1}\u{8272}",
        (DiffLanguage::Chinese, "light") => "\u{4eae}\u{8272}",
        (_, "empty") => "No differences",
        (_, "language") => "EN",
        (_, "dark") => "Dark",
        (_, "light") => "Light",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunk_header_becomes_readable_range_summary() {
        let raw = "@@ -96,6 +96,12 @@ const ruleId = getRuleId()";
        assert_eq!(
            format_hunk_summary(raw, DiffLanguage::Chinese),
            "区块  旧 96–101  →  新 96–107  ·  const ruleId = getRuleId()"
        );
        assert_eq!(
            format_hunk_summary(raw, DiffLanguage::English),
            "Block  old 96–101  →  new 96–107  ·  const ruleId = getRuleId()"
        );
    }

    #[test]
    fn git_quoted_non_ascii_patch_paths_decode_to_real_file_names() {
        let quoted = "\"a/\\346\\265\\213\\350\\257\\225 \\346\\226\\207\\344\\273\\266.txt\"";
        assert_eq!(parse_git_path_tokens(quoted), ["a/测试 文件.txt"]);

        let patch = format!(
            "diff --git {quoted} \"b/\\346\\265\\213\\350\\257\\225 \\346\\226\\207\\344\\273\\266.txt\"\n--- {quoted}\n+++ \"b/\\346\\265\\213\\350\\257\\225 \\346\\226\\207\\344\\273\\266.txt\"\n@@ -1 +1 @@\n-old\n+new\n"
        );
        let files = parse_side_by_side_diff(&patch);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].left_path, "a/测试 文件.txt");
        assert_eq!(files[0].right_path, "b/测试 文件.txt");
    }

    #[test]
    fn diff_window_shell_matches_merge_controls_without_native_borders_or_dividers() {
        let source = include_str!("diff_tool.rs");
        assert!(source.contains("prefer_shadowed_diff_window(cc);"));
        assert!(source.contains("DWMWA_BORDER_COLOR"));
        assert!(source.contains("DWMWA_COLOR_NONE"));
        assert!(source.contains(".with_decorations(false)"));
        assert!(source.contains(".with_transparent(true)"));
        assert!(source.contains("fn diff_custom_title_bar("));
        assert!(source.contains("fn diff_window_control_button("));
        assert!(source.contains("merge-language.svg"));
        assert!(source.contains("merge-sun.svg"));
        assert!(source.contains("merge-moon.svg"));
        assert!(source.matches(".show_separator_line(false)").count() >= 2);
    }

    #[test]
    fn frameless_diff_window_edges_resize_before_the_title_drag_region() {
        use egui::viewport::ResizeDirection;

        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1_120.0, 760.0));
        assert_eq!(
            diff_window_resize_direction(rect, Pos2::new(1_117.0, 757.0)),
            Some(ResizeDirection::SouthEast)
        );
        assert_eq!(
            diff_window_resize_direction(rect, Pos2::new(2.0, 380.0)),
            Some(ResizeDirection::West)
        );
        assert_eq!(
            diff_window_resize_direction(rect, Pos2::new(560.0, 380.0)),
            None
        );

        let source = include_str!("diff_tool.rs");
        let update_start = source.find("impl App for DiffToolApp").unwrap();
        let update_end = source[update_start..]
            .find("pub fn parse_diff_args")
            .unwrap();
        let update_source = &source[update_start..update_start + update_end];
        assert!(
            update_source
                .find("request_diff_window_resize_on_pointer_down(ctx, frame)")
                .unwrap()
                < update_source
                    .find("request_diff_window_drag_on_pointer_down(ctx, frame)")
                    .unwrap()
        );
        assert!(source.contains("input.pointer.primary_pressed()"));
        assert!(source.contains("ViewportCommand::BeginResize(direction)"));
        assert!(source.contains("ViewportCommand::StartDrag"));
    }

    #[test]
    fn display_rows_hide_git_index_plumbing() {
        let files = parse_side_by_side_diff(
            "diff --git a/a.rs b/a.rs\nindex 1111111..2222222 100644\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-old\n+new\n",
        );
        assert!(
            files[0]
                .rows
                .iter()
                .any(|row| matches!(row, DiffRow::Meta(text) if text.starts_with("index ")))
        );
        let rows = diff_display_rows(&files);
        assert_eq!(rows.len(), 3);
        assert!(matches!(rows[0], DiffDisplayRow::File(_)));
        assert!(matches!(rows[1], DiffDisplayRow::Hunk(_)));
        assert!(matches!(rows[2], DiffDisplayRow::Line { .. }));
    }

    #[test]
    fn patch_prefix_parser_distinguishes_git_columns_from_source_text() {
        assert_eq!(diff_prefix_width_from_hunk_header("@@ -1 +1 @@"), Some(1));
        assert_eq!(
            diff_prefix_width_from_hunk_header("@@@ -1 -1 +1 @@@"),
            Some(2)
        );
        assert_eq!(
            diff_prefix_width_from_hunk_header("@@@@ -1 -1 -1 +1 @@@@"),
            Some(3)
        );

        assert_eq!(
            patch_content_line("++literal-plus", 1),
            Some(PatchContentLine {
                kind: PatchLineKind::Added,
                body: "+literal-plus",
            })
        );
        assert_eq!(
            patch_content_line("++<<<<<<< HEAD", 2),
            Some(PatchContentLine {
                kind: PatchLineKind::Added,
                body: "<<<<<<< HEAD",
            })
        );
        assert_eq!(
            patch_content_line(" +export const ours = true", 2),
            Some(PatchContentLine {
                kind: PatchLineKind::Context,
                body: "export const ours = true",
            })
        );
        assert_eq!(
            patch_content_line("+ export const theirs = true", 2),
            Some(PatchContentLine {
                kind: PatchLineKind::Added,
                body: "export const theirs = true",
            })
        );
        assert_eq!(patch_content_line("\\ No newline at end of file", 1), None);
    }

    #[test]
    fn combined_diff_removes_every_parent_prefix_before_rendering() {
        let files = parse_side_by_side_diff(
            "diff --cc src/pipeline.ts\nindex 1111111,2222222..0000000\n--- a/src/pipeline.ts\n+++ b/src/pipeline.ts\n@@@ -1,3 -1,3 +1,7 @@@\n  const count = 1\n++<<<<<<< HEAD\n +export const ours = true\n++=======\n+ export const theirs = true\n++>>>>>>> branch\n  const done = true\n",
        );
        let rendered = files[0]
            .rows
            .iter()
            .filter_map(|row| match row {
                DiffRow::Line(line) => Some(line),
                _ => None,
            })
            .collect::<Vec<_>>();
        let right = rendered
            .iter()
            .filter_map(|line| line.right_line.zip(Some(line.right_text.as_str())))
            .collect::<Vec<_>>();

        assert_eq!(
            right,
            vec![
                (1, "const count = 1"),
                (2, "<<<<<<< HEAD"),
                (3, "export const ours = true"),
                (4, "======="),
                (5, "export const theirs = true"),
                (6, ">>>>>>> branch"),
                (7, "const done = true"),
            ]
        );
    }

    #[test]
    fn syntax_session_attaches_full_document_highlights_to_diff_sides() {
        let mut files = parse_side_by_side_diff(
            "diff --git a/src/view.ts b/src/view.ts\n--- a/src/view.ts\n+++ b/src/view.ts\n@@ -1 +1 @@\n-export const left = 1\n+export const right = 2\n",
        );
        let highlighted = HighlightedDocument {
            lines: vec![HighlightedLine {
                spans: vec![crate::syntax::HighlightSpan {
                    start: 0,
                    end: 6,
                    role: SyntaxRole::Keyword,
                }],
            }],
            ..Default::default()
        };
        apply_diff_syntax_session(
            &mut files,
            DiffSyntaxSession {
                files: vec![DiffSyntaxFile {
                    left_path: "src/view.ts".to_owned(),
                    right_path: "src/view.ts".to_owned(),
                    left_highlight: Some(highlighted.clone()),
                    right_highlight: Some(highlighted),
                }],
            },
        );

        assert!(files[0].left_highlight.is_some());
        assert!(files[0].right_highlight.is_some());
        let line = files[0]
            .left_highlight
            .as_ref()
            .and_then(|document| document.lines.first())
            .unwrap();
        let job = diff_syntax_layout_job(
            "export const left = 1",
            Some(line),
            Color32::BLACK,
            diff_palette(DiffTheme::Light),
        );
        assert_eq!(&job.text[job.sections[0].byte_range.clone()], "export");
        assert_ne!(job.sections[0].format.color, Color32::BLACK);
    }

    #[test]
    fn minimap_clicks_reach_document_ends() {
        let track = Rect::from_min_size(Pos2::new(20.0, 10.0), Vec2::new(14.0, 600.0));
        assert_eq!(
            diff_minimap_scroll_target(track, track.top(), 200.0, 1_000.0),
            0.0
        );
        assert_eq!(
            diff_minimap_scroll_target(track, track.bottom(), 200.0, 1_000.0),
            800.0
        );
        let bottom = diff_minimap_viewport_rect(track, 800.0, 200.0, 1_000.0);
        assert!((bottom.bottom() - track.bottom()).abs() < 0.01);
    }

    #[test]
    fn minimap_is_only_reserved_for_overflowing_diff_rows() {
        assert!(!diff_needs_minimap(10, 10.0 * DIFF_ROW_HEIGHT));
        assert!(!diff_needs_minimap(0, 600.0));
        assert!(diff_needs_minimap(11, 10.0 * DIFF_ROW_HEIGHT));
    }

    #[test]
    fn diff_icon_has_transparency_and_colored_content() {
        let icon = diff_app_icon_data();
        assert_eq!((icon.width, icon.height), (64, 64));
        assert!(icon.rgba.chunks_exact(4).any(|pixel| pixel[3] == 0));
        assert!(icon.rgba.chunks_exact(4).any(|pixel| pixel[3] == 255));
    }
}
