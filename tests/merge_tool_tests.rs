use std::path::PathBuf;
use std::{env, fs, process::Command};

use git_agent::merge_tool::{
    MergeArgs, MergeConnectorDebug, MergeLanguage, MergeLineKind, MergeSide, MergeTheme,
    merge_debug_label, merge_language_label, merge_theme_label, parse_merge_args, three_way_merge,
    write_merge_output,
};

#[test]
fn parses_named_merge_tool_arguments() {
    let args = parse_merge_args([
        "git-agent-merge",
        "--base",
        "base.txt",
        "--local",
        "local.txt",
        "--remote",
        "remote.txt",
        "--output",
        "merged.txt",
    ])
    .unwrap();

    assert_eq!(
        args,
        MergeArgs {
            base: PathBuf::from("base.txt"),
            local: PathBuf::from("local.txt"),
            remote: PathBuf::from("remote.txt"),
            output: PathBuf::from("merged.txt"),
            repo_root: None,
            stage: false,
            theme: MergeTheme::Dark,
            language: MergeLanguage::English,
        }
    );
}

#[test]
fn merge_tool_uses_a_custom_icon_matching_the_main_app_palette() {
    let source = include_str!("../src/merge_tool.rs");
    let logo = include_str!("../assets/icons/logo-git-agent-merge.svg");

    assert!(source.contains(".with_icon(merge_app_icon_data())"));
    assert!(source.contains("fn merge_app_icon_data()"));
    assert!(source.contains("let green = [21, 196, 151, 255]"));
    assert!(source.contains("let blue = [47, 111, 234, 255]"));
    assert!(source.contains("paint_merge_icon_line(&mut rgba, 24, 17, 24, 34, green)"));
    assert!(logo.contains("stroke=\"#15C497\""));
    assert!(logo.contains("stroke=\"#2F6FEA\""));
    assert!(logo.contains("m-6-6 6 6 6-6"));
    assert!(logo.contains("<circle cx=\"48\" cy=\"39\""));
}

#[test]
fn parses_positional_merge_tool_arguments() {
    let args = parse_merge_args([
        "git-agent-merge",
        "base.txt",
        "local.txt",
        "remote.txt",
        "merged.txt",
    ])
    .unwrap();

    assert_eq!(args.base, PathBuf::from("base.txt"));
    assert_eq!(args.local, PathBuf::from("local.txt"));
    assert_eq!(args.remote, PathBuf::from("remote.txt"));
    assert_eq!(args.output, PathBuf::from("merged.txt"));
    assert_eq!(args.repo_root, None);
    assert!(!args.stage);
    assert_eq!(args.theme, MergeTheme::Dark);
    assert_eq!(args.language, MergeLanguage::English);
}

#[test]
fn parses_theme_and_language_options() {
    let args = parse_merge_args([
        "git-agent-merge",
        "--base",
        "base.txt",
        "--local",
        "local.txt",
        "--remote",
        "remote.txt",
        "--output",
        "merged.txt",
        "--theme",
        "light",
        "--language",
        "zh",
        "--repo-root",
        "D:/repo",
        "--stage",
    ])
    .unwrap();

    assert_eq!(args.theme, MergeTheme::Light);
    assert_eq!(args.language, MergeLanguage::Chinese);
    assert_eq!(args.repo_root, Some(PathBuf::from("D:/repo")));
    assert!(args.stage);
}

#[test]
fn merge_tool_layout_uses_fixed_regions_and_unique_scroll_ids() {
    let source = include_str!("../src/merge_tool.rs");

    assert!(source.contains("TopBottomPanel::top(\"merge_toolbar\")"));
    assert!(source.contains("TopBottomPanel::bottom(\"merge_footer\")"));
    assert!(source.contains("CentralPanel::default()"));
    assert!(source.contains("merge_editor_columns("));
    assert!(!source.contains("ui.columns(3"));
    assert!(source.contains("\"merge_local_scroll\""));
    assert!(source.contains("\"merge_result_scroll\""));
    assert!(source.contains("\"merge_remote_scroll\""));
    assert!(source.contains("id_salt(scroll_id)"));
    assert!(source.contains("toggle_theme("));
    assert!(source.contains("toggle_language("));
    assert!(source.contains("side_conflict_nav("));
    assert!(source.contains("fn nav_icon_button("));
    assert!(source.contains("fn paint_nav_chevron("));
    assert!(source.contains("NavDirection::Previous"));
    assert!(source.contains("NavDirection::Next"));
    assert!(source.contains("MergeLineAction::Take"));
    assert!(source.contains("MergeLineAction::Drop"));
    assert!(source.contains("egui::Button::new(\"X\")"));
    assert!(source.contains("MergeSide::Local => \">>\""));
    assert!(source.contains("MergeSide::Remote => \"<<\""));
    assert!(source.contains("MERGE_COLUMN_GAP"));
    assert!(source.contains(".color(Color32::WHITE)"));
    assert!(source.contains("offset: [3, 4]"));
    assert!(source.contains("bg_stroke = egui::Stroke::NONE"));
    assert!(source.contains("shared_scroll_y"));
    assert!(source.contains("let frame_scroll_y = result_scroll_y;"));
    assert!(source.contains(".vertical_scroll_offset(scroll_y)"));
    assert!(source.contains("merge_editable_result_row("));
    assert!(source.contains("paint_merge_block_connectors("));
    assert!(source.contains("merge_block_result_rect("));
    assert!(source.contains("merge_theme_label(app.language, app.theme)"));
    assert!(source.contains("merge_language_label(app.language)"));
    assert!(source.contains("ui.add_space(14.0);"));
    assert!(source.contains(".corner_radius(egui::CornerRadius::same(MERGE_PANEL_RADIUS))"));
    assert!(!source.contains("egui::Button::new(\"^\")"));
    assert!(!source.contains("egui::Button::new(\"v\")"));
    assert!(source.contains("\"使用我的版本\""));
    assert!(source.contains("\"使用他的版本\""));
    assert!(!source.contains(".stroke("));
    assert!(!source.contains("ui.separator()"));
    assert!(source.contains("\"合并修订\""));
    assert!(source.contains("\"中文\""));
}

#[test]
fn merge_toolbar_labels_show_current_theme_and_language() {
    assert_eq!(
        merge_theme_label(MergeLanguage::Chinese, MergeTheme::Light),
        "白天"
    );
    assert_eq!(
        merge_theme_label(MergeLanguage::Chinese, MergeTheme::Dark),
        "黑夜"
    );
    assert_eq!(merge_language_label(MergeLanguage::Chinese), "中文");
    assert_eq!(merge_language_label(MergeLanguage::English), "EN");
}

#[test]
fn merge_toolbar_labels_show_debug_state() {
    assert_eq!(
        merge_debug_label(MergeLanguage::Chinese, MergeConnectorDebug::Off),
        "辅助线"
    );
    assert_eq!(
        merge_debug_label(MergeLanguage::Chinese, MergeConnectorDebug::Guides),
        "隐藏辅助线"
    );
    assert_eq!(
        merge_debug_label(MergeLanguage::Chinese, MergeConnectorDebug::Log),
        "日志辅助线"
    );
    assert_eq!(
        merge_debug_label(MergeLanguage::English, MergeConnectorDebug::Off),
        "Guides"
    );
}

#[test]
fn write_merge_output_can_stage_resolved_file() {
    let root = env::temp_dir().join(format!("git-agent-merge-stage-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    run_git(&root, &["init"]);

    let output = root.join("story.txt");
    let args = MergeArgs {
        base: root.join("base.txt"),
        local: root.join("local.txt"),
        remote: root.join("remote.txt"),
        output: output.clone(),
        repo_root: Some(root.clone()),
        stage: true,
        theme: MergeTheme::Light,
        language: MergeLanguage::Chinese,
    };

    write_merge_output(&args, "resolved\n").unwrap();

    assert_eq!(fs::read_to_string(&output).unwrap(), "resolved\n");
    let cached = git_output(&root, &["diff", "--cached", "--name-only"]);
    assert_eq!(cached.trim(), "story.txt");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn merge_tool_apply_uses_background_write_task() {
    let source = include_str!("../src/merge_tool.rs");
    assert!(source.contains("write_task: Option<Receiver<anyhow::Result<()>>>"));
    assert!(source.contains("fn poll_write_task(&mut self, ctx: &egui::Context)"));
    assert!(source.contains("thread::spawn(move ||"));
    assert!(source.contains("self.write_task = Some(receiver);"));
    assert!(source.contains("ctx.request_repaint_after("));
    assert!(source.contains("mt(self.language, \"applying\")"));
    assert!(source.contains(".add_enabled("));
    assert!(source.contains("!writing"));
}

#[test]
fn auto_merges_non_overlapping_line_changes() {
    let merged = three_way_merge(
        "line one\nline two\nline three\n",
        "line one local\nline two\nline three\n",
        "line one\nline two remote\nline three\n",
    );

    assert_eq!(
        merged.result_text(),
        "line one local\nline two remote\nline three\n"
    );
    assert!(merged.conflicts().is_empty());
}

#[test]
fn auto_merges_deletion_when_other_side_keeps_base_line() {
    let merged = three_way_merge("one\ntwo\n", "one\n", "one\ntwo\n");

    assert_eq!(merged.result_text(), "one\n");
    assert!(merged.conflicts().is_empty());

    let merged = three_way_merge("one\ntwo\n", "one\ntwo\n", "one\n");

    assert_eq!(merged.result_text(), "one\n");
    assert!(merged.conflicts().is_empty());
}

#[test]
fn delete_modify_conflict_remains_resolvable() {
    let mut merged = three_way_merge("batch commit A\n", "", "batch commit A\nthis n\n");

    let conflicts = merged.conflicts();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].base, vec!["batch commit A"]);
    assert!(conflicts[0].local.is_empty());
    assert_eq!(conflicts[0].remote, vec!["batch commit A", "this n"]);
    assert_eq!(merged.result_text(), "");

    merged.accept_conflict_side_only(0, MergeSide::Remote);
    assert!(merged.conflict_side_resolved(0, MergeSide::Local));
    assert!(merged.conflict_side_resolved(0, MergeSide::Remote));
    assert_eq!(merged.result_text(), "batch commit A\nthis n\n");
}

#[test]
fn keeps_conflicting_lines_as_resolvable_blocks() {
    let merged = three_way_merge(
        "shared line from base\nmain keeps this file\n",
        "shared line from main branch\nmain keeps this file\n",
        "shared line from feature branch\nmain keeps this file\n",
    );

    let conflicts = merged.conflicts();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].base, vec!["shared line from base"]);
    assert_eq!(conflicts[0].local, vec!["shared line from main branch"]);
    assert_eq!(conflicts[0].remote, vec!["shared line from feature branch"]);
    assert!(
        merged
            .lines
            .iter()
            .any(|line| line.kind == MergeLineKind::Conflict)
    );
}

#[test]
fn conflict_sides_can_be_taken_or_dropped_independently() {
    let mut merged = three_way_merge(
        "shared line from base\nmain keeps this file\n",
        "shared line from main branch\nmain keeps this file\n",
        "shared line from feature branch\nmain keeps this file\n",
    );

    assert_eq!(merged.result_text(), "main keeps this file\n");

    merged.take_conflict_side(0, MergeSide::Local);
    assert!(merged.conflict_side_resolved(0, MergeSide::Local));
    assert!(!merged.conflict_side_resolved(0, MergeSide::Remote));
    assert_eq!(
        merged.result_text(),
        "shared line from main branch\nmain keeps this file\n"
    );

    merged.take_conflict_side(0, MergeSide::Remote);
    assert!(merged.conflict_side_resolved(0, MergeSide::Remote));
    assert_eq!(
        merged.result_text(),
        "shared line from main branch\nshared line from feature branch\nmain keeps this file\n"
    );

    let mut merged = three_way_merge("base\n", "local\n", "remote\n");
    merged.drop_conflict_side(0, MergeSide::Local);
    merged.take_conflict_side(0, MergeSide::Remote);
    assert_eq!(merged.result_text(), "remote\n");
}

fn run_git(root: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(root)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

fn git_output(root: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?} failed");
    String::from_utf8(output.stdout).unwrap()
}
