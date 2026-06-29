use std::path::PathBuf;

use git_agent::diff_tool::{
    DiffArgs, DiffCellKind, DiffLanguage, DiffRow, DiffTheme, diff_file_display_label,
    parse_diff_args, parse_side_by_side_diff,
};

#[test]
fn parses_named_diff_tool_arguments() {
    let args = parse_diff_args([
        "git-agent-diff",
        "--title",
        "Compare commit",
        "--left",
        "abc123",
        "--right",
        "working tree",
        "--diff",
        "changes.patch",
        "--theme",
        "light",
        "--language",
        "zh",
    ])
    .unwrap();

    assert_eq!(
        args,
        DiffArgs {
            title: "Compare commit".to_owned(),
            left_label: "abc123".to_owned(),
            right_label: "working tree".to_owned(),
            diff: PathBuf::from("changes.patch"),
            theme: DiffTheme::Light,
            language: DiffLanguage::Chinese,
        }
    );
}

#[test]
fn diff_tool_binary_uses_windows_gui_subsystem() {
    let source = include_str!("../src/bin/git-agent-diff.rs");

    assert!(source.contains("windows_subsystem = \"windows\""));
    assert!(source.contains("git_agent::diff_tool::DiffToolApp::run_from_env()"));
}

#[test]
fn parses_unified_diff_into_side_by_side_rows() {
    let diff = "\
diff --git a/Dockerfile b/Dockerfile
index 7c379a761..1b7f63f83 100644
--- a/Dockerfile
+++ b/Dockerfile
@@ -10,4 +10,5 @@ WORKDIR /build
 COPY package.json pnpm-lock.yaml ./
-RUN pnpm install
+RUN pnpm install --frozen-lockfile
+RUN pnpm audit
 CMD pnpm build
";

    let files = parse_side_by_side_diff(diff);

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].left_path, "a/Dockerfile");
    assert_eq!(files[0].right_path, "b/Dockerfile");
    assert!(matches!(&files[0].rows[0], DiffRow::Meta(text) if text.contains("index")));
    assert!(matches!(&files[0].rows[1], DiffRow::Hunk(text) if text.contains("WORKDIR")));

    let rows = files[0]
        .rows
        .iter()
        .filter_map(|row| match row {
            DiffRow::Line(line) => Some(line),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(rows[0].left_line, Some(10));
    assert_eq!(rows[0].right_line, Some(10));
    assert_eq!(rows[0].left_text, "COPY package.json pnpm-lock.yaml ./");
    assert_eq!(rows[0].right_text, rows[0].left_text);
    assert_eq!(rows[0].left_kind, DiffCellKind::Context);
    assert_eq!(rows[0].right_kind, DiffCellKind::Context);

    assert_eq!(rows[1].left_line, Some(11));
    assert_eq!(rows[1].right_line, Some(11));
    assert_eq!(rows[1].left_text, "RUN pnpm install");
    assert_eq!(rows[1].right_text, "RUN pnpm install --frozen-lockfile");
    assert_eq!(rows[1].left_kind, DiffCellKind::Removed);
    assert_eq!(rows[1].right_kind, DiffCellKind::Added);

    assert_eq!(rows[2].left_line, None);
    assert_eq!(rows[2].right_line, Some(12));
    assert_eq!(rows[2].left_text, "");
    assert_eq!(rows[2].right_text, "RUN pnpm audit");
    assert_eq!(rows[2].left_kind, DiffCellKind::Empty);
    assert_eq!(rows[2].right_kind, DiffCellKind::Added);
}

#[test]
fn file_headers_use_side_labels_instead_of_git_a_b_prefixes() {
    assert_eq!(
        diff_file_display_label("27c00838", "a/src/asd.text"),
        "[27c00838]/src/asd.text"
    );
    assert_eq!(
        diff_file_display_label("worktree", "b/src/asd.text"),
        "[worktree]/src/asd.text"
    );
    assert_eq!(
        diff_file_display_label("worktree", "src/asd.text"),
        "[worktree]/src/asd.text"
    );
}
