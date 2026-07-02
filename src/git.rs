use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

const HISTORY_COMMIT_LIMIT: usize = 50_000;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Commit {
    pub hash: String,
    pub short_hash: String,
    pub parents: Vec<String>,
    pub author: String,
    pub date: String,
    pub relative_time: String,
    pub subject: String,
    pub refs: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FileChange {
    pub status: String,
    pub path: String,
    pub diff_path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub current: bool,
    pub remote: bool,
    pub upstream: Option<UpstreamStatus>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Remote {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RepositoryConfig {
    pub config_path: PathBuf,
    pub gitignore_path: PathBuf,
    pub user_name: String,
    pub user_email: String,
    pub uses_global_user: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UpstreamStatus {
    pub name: String,
    pub ahead: usize,
    pub behind: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StashEntry {
    pub selector: String,
    pub relative_time: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub target: String,
    pub subject: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorktreeFile {
    pub index_status: char,
    pub worktree_status: char,
    pub path: String,
    pub display_path: String,
}

impl WorktreeFile {
    pub fn is_conflicted(&self) -> bool {
        matches!(
            (self.index_status, self.worktree_status),
            ('A', 'A') | ('D', 'D') | ('U', _) | (_, 'U')
        )
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RepositorySnapshot {
    pub root: PathBuf,
    pub branch: String,
    pub merge_message: Option<String>,
    #[serde(default)]
    pub rebase_in_progress: bool,
    pub upstream: Option<UpstreamStatus>,
    pub branches: Vec<Branch>,
    pub remotes: Vec<Remote>,
    pub stashes: Vec<StashEntry>,
    pub tags: Vec<Tag>,
    pub status: Vec<String>,
    pub staged: Vec<WorktreeFile>,
    pub unstaged: Vec<WorktreeFile>,
    pub commits: Vec<Commit>,
    pub date_commits: Vec<Commit>,
    pub topology_commits: Vec<Commit>,
    pub all_date_commits: Vec<Commit>,
    pub all_topology_commits: Vec<Commit>,
    pub config: RepositoryConfig,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitOrder {
    Date,
    Topology,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommitScope {
    CurrentBranch,
    AllBranches,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CommitDetails {
    pub hash: String,
    pub files: Vec<FileChange>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FileDiff {
    pub key: String,
    pub text: String,
}

pub fn open_repository(path: impl AsRef<Path>) -> Result<RepositorySnapshot> {
    let root = discover_root(path.as_ref())?;
    let branch = git_output(&root, &["branch", "--show-current"])
        .unwrap_or_else(|_| "HEAD".to_owned())
        .trim()
        .to_owned();
    let branch = if branch.is_empty() {
        "HEAD".to_owned()
    } else {
        branch
    };

    let status = git_output(&root, &["status", "--short", "--untracked-files=all"])
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let (staged, unstaged) = parse_status_entries(&status);

    let mut branches = load_branches(&root).unwrap_or_default();
    if branches.is_empty() && branch != "HEAD" {
        branches.push(Branch {
            name: branch.clone(),
            current: true,
            remote: false,
            upstream: None,
        });
    }
    let remotes = load_remotes(&root).unwrap_or_default();
    let config = load_repository_config(&root);
    let merge_message = load_merge_message(&root, &branch);
    let rebase_in_progress = rebase_in_progress(&root);
    let upstream = load_upstream_status(&root).ok().flatten();
    let stashes = load_stashes(&root).unwrap_or_default();
    let tags = load_tags(&root).unwrap_or_default();
    let date_commits = load_commits(
        &root,
        HISTORY_COMMIT_LIMIT,
        CommitOrder::Date,
        CommitScope::CurrentBranch,
    )?;
    let topology_commits = load_commits(
        &root,
        HISTORY_COMMIT_LIMIT,
        CommitOrder::Topology,
        CommitScope::CurrentBranch,
    )?;
    let all_date_commits = load_commits(
        &root,
        HISTORY_COMMIT_LIMIT,
        CommitOrder::Date,
        CommitScope::AllBranches,
    )?;
    let all_topology_commits = load_commits(
        &root,
        HISTORY_COMMIT_LIMIT,
        CommitOrder::Topology,
        CommitScope::AllBranches,
    )?;
    let commits = date_commits.clone();

    Ok(RepositorySnapshot {
        root,
        branch,
        merge_message,
        rebase_in_progress,
        upstream,
        branches,
        remotes,
        stashes,
        tags,
        status,
        staged,
        unstaged,
        commits,
        date_commits,
        topology_commits,
        all_date_commits,
        all_topology_commits,
        config,
    })
}

pub fn init_repository(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    let output = git_command()
        .arg("-C")
        .arg(path)
        .arg("init")
        .output()
        .with_context(|| format!("failed to run git init in {}", path.display()))?;

    if !output.status.success() {
        return Err(anyhow!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    discover_root(path)
}

pub fn clone_repository(url: &str, destination: impl AsRef<Path>) -> Result<PathBuf> {
    let destination = destination.as_ref();
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let output = git_command()
        .arg("clone")
        .arg(url)
        .arg(destination)
        .output()
        .with_context(|| format!("failed to run git clone {}", url))?;

    if !output.status.success() {
        return Err(anyhow!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    discover_root(destination)
}

pub fn validate_remote_url(url: &str) -> Result<()> {
    let url = url.trim();
    if url.is_empty() {
        return Err(anyhow!("remote URL is empty"));
    }

    let output = git_command()
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["ls-remote", url])
        .output()
        .with_context(|| format!("failed to validate remote URL {}", url))?;

    if !output.status.success() {
        return Err(anyhow!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

pub fn load_commit_details(root: impl AsRef<Path>, hash: &str) -> Result<CommitDetails> {
    let output = git_output(
        root.as_ref(),
        &["show", "--format=", "--name-status", "--find-renames", hash],
    )?;

    let files = parse_file_changes(&output);

    Ok(CommitDetails {
        hash: hash.to_owned(),
        files,
    })
}

pub fn search_commits_by_changed_file(root: impl AsRef<Path>, query: &str) -> Result<Vec<String>> {
    let raw_query = query.trim();
    if raw_query.is_empty() {
        return Ok(Vec::new());
    }

    let max_count = format!("--max-count={HISTORY_COMMIT_LIMIT}");
    let path_output = git_output(
        root.as_ref(),
        &[
            "log",
            "--date-order",
            &max_count,
            "--name-only",
            "--format=%x1e%H",
        ],
    )?;
    let content_regex = literal_git_regex(raw_query);
    let content_output = git_output(
        root.as_ref(),
        &[
            "log",
            "--date-order",
            &max_count,
            "--regexp-ignore-case",
            "-G",
            &content_regex,
            "--format=%H",
        ],
    )?;

    let mut hashes = parse_changed_file_search_log(&path_output, raw_query);
    for hash in parse_hash_lines(&content_output) {
        if !hashes.iter().any(|existing| existing == &hash) {
            hashes.push(hash);
        }
    }
    Ok(hashes)
}

fn parse_changed_file_search_log(output: &str, query: &str) -> Vec<String> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }

    let mut hashes = Vec::new();
    for record in output
        .split('\x1e')
        .filter(|record| !record.trim().is_empty())
    {
        let mut lines = record.lines().filter(|line| !line.trim().is_empty());
        let Some(hash) = lines.next().map(str::trim) else {
            continue;
        };
        if lines.any(|path| path.to_lowercase().contains(&query)) {
            hashes.push(hash.to_owned());
        }
    }
    hashes
}

fn parse_hash_lines(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

fn literal_git_regex(query: &str) -> String {
    let mut escaped = String::new();
    for ch in query.chars() {
        if matches!(ch, '\\' | '.' | '^' | '$' | '*' | '[' | ']') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

pub fn load_file_diff(root: impl AsRef<Path>, hash: &str, path: &str) -> Result<FileDiff> {
    let text = git_output(
        root.as_ref(),
        &[
            "show",
            "--format=",
            "--find-renames",
            "--unified=80",
            hash,
            "--",
            path,
        ],
    )?;
    let key = diff_key(hash, path);

    Ok(FileDiff { key, text })
}

pub fn diff_key(hash: &str, path: &str) -> String {
    format!("{hash}:{path}")
}

pub fn load_worktree_diff(
    root: impl AsRef<Path>,
    path: &str,
    staged: bool,
    untracked: bool,
) -> Result<FileDiff> {
    let root = root.as_ref();
    let text = if untracked && !staged {
        load_untracked_file_diff(root, path)?
    } else if staged {
        git_output(root, &["diff", "--cached", "--", path])?
    } else {
        git_output(root, &["diff", "--", path])?
    };
    let key = worktree_diff_key(path, staged);
    Ok(FileDiff { key, text })
}

pub fn diff_worktree_against_commit(root: impl AsRef<Path>, hash: &str) -> Result<String> {
    git_output(
        root.as_ref(),
        &["diff", "--no-ext-diff", "--find-renames", hash, "--"],
    )
}

fn load_untracked_file_diff(root: &Path, path: &str) -> Result<String> {
    let content = fs::read_to_string(root.join(path)).with_context(|| {
        format!(
            "failed to read untracked file {}",
            root.join(path).display()
        )
    })?;
    Ok(new_file_unified_diff(path, &content))
}

fn new_file_unified_diff(path: &str, content: &str) -> String {
    let line_count = content.lines().count();
    let hunk_target = if line_count == 0 {
        "+0,0".to_owned()
    } else {
        format!("+1,{line_count}")
    };
    let mut diff = format!(
        "diff --git a/{path} b/{path}\nnew file mode 100644\nindex 0000000..0000000\n--- /dev/null\n+++ b/{path}\n@@ -0,0 {hunk_target} @@\n"
    );
    for line in content.lines() {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }
    if !content.is_empty() && !content.ends_with('\n') {
        diff.push_str("\\ No newline at end of file\n");
    }
    diff
}

pub fn conflict_file_versions(
    root: impl AsRef<Path>,
    path: &str,
) -> Result<(String, String, String)> {
    let root = root.as_ref();
    let base = git_output(root, &["show", &format!(":1:{path}")]).unwrap_or_default();
    let local = git_output(root, &["show", &format!(":2:{path}")]).unwrap_or_default();
    let remote = git_output(root, &["show", &format!(":3:{path}")]).unwrap_or_default();
    Ok((base, local, remote))
}

pub fn worktree_diff_key(path: &str, staged: bool) -> String {
    format!(
        "worktree:{}:{path}",
        if staged { "staged" } else { "unstaged" }
    )
}

pub fn checkout_commit(root: impl AsRef<Path>, hash: &str) -> Result<()> {
    git_output(root.as_ref(), &["checkout", hash]).map(|_| ())
}

pub fn cherry_pick_commit(root: impl AsRef<Path>, hash: &str) -> Result<()> {
    git_output(root.as_ref(), &["cherry-pick", hash]).map(|_| ())
}

pub fn cherry_pick_commits(root: impl AsRef<Path>, hashes: &[String]) -> Result<()> {
    if hashes.is_empty() {
        return Ok(());
    }
    let args = cherry_pick_args(hashes);
    git_output(root.as_ref(), &args).map(|_| ())
}

fn cherry_pick_args(hashes: &[String]) -> Vec<&str> {
    let mut args = Vec::with_capacity(hashes.len() + 1);
    args.push("cherry-pick");
    args.extend(hashes.iter().map(String::as_str));
    args
}

pub fn revert_commit(root: impl AsRef<Path>, hash: &str) -> Result<()> {
    git_output(root.as_ref(), &["revert", "--no-edit", hash]).map(|_| ())
}

pub fn reset_to_commit(root: impl AsRef<Path>, hash: &str, mode: ResetMode) -> Result<()> {
    git_output(root.as_ref(), &["reset", mode.flag(), hash]).map(|_| ())
}

pub fn discard_all_changes(root: impl AsRef<Path>) -> Result<()> {
    let root = root.as_ref();
    git_output(root, &["reset", "--hard"])?;
    git_output(root, &["clean", "-fd"]).map(|_| ())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

impl ResetMode {
    pub fn flag(self) -> &'static str {
        match self {
            Self::Soft => "--soft",
            Self::Mixed => "--mixed",
            Self::Hard => "--hard",
        }
    }
}

pub fn checkout_branch(root: impl AsRef<Path>, name: &str) -> Result<()> {
    git_output(root.as_ref(), &["checkout", name]).map(|_| ())
}

pub fn create_branch(root: impl AsRef<Path>, name: &str, hash: &str, checkout: bool) -> Result<()> {
    if checkout {
        git_output(root.as_ref(), &["checkout", "-b", name, hash]).map(|_| ())
    } else {
        git_output(root.as_ref(), &["branch", name, hash]).map(|_| ())
    }
}

pub fn create_branch_from_head(root: impl AsRef<Path>, name: &str, checkout: bool) -> Result<()> {
    if checkout {
        git_output(root.as_ref(), &["checkout", "-b", name]).map(|_| ())
    } else {
        git_output(root.as_ref(), &["branch", name]).map(|_| ())
    }
}

pub fn checkout_remote_branch(
    root: impl AsRef<Path>,
    remote_branch: &str,
    local_branch: &str,
) -> Result<()> {
    git_output(
        root.as_ref(),
        &["checkout", "-b", local_branch, "--track", remote_branch],
    )
    .map(|_| ())
}

pub fn merge_branch(root: impl AsRef<Path>, name: &str) -> Result<()> {
    let root = root.as_ref();
    if merge_in_progress(root) {
        return Ok(());
    }
    git_output_allowing_new_conflicts(root, &["merge", name])
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MergeOptions {
    pub commit_merge: bool,
    pub include_messages: bool,
    pub force_merge_commit: bool,
    pub rebase: bool,
    pub detect_renames: bool,
    pub rename_threshold: u8,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            commit_merge: true,
            include_messages: false,
            force_merge_commit: false,
            rebase: false,
            detect_renames: false,
            rename_threshold: 90,
        }
    }
}

fn merge_commit_args(target: &str, options: MergeOptions) -> Vec<String> {
    if options.rebase {
        return vec!["rebase".to_owned(), target.to_owned()];
    }

    let mut args = vec!["merge".to_owned()];
    if !options.commit_merge {
        args.push("--no-commit".to_owned());
    }
    if options.include_messages {
        args.push("--log".to_owned());
    }
    if options.force_merge_commit {
        args.push("--no-ff".to_owned());
    }
    if options.detect_renames {
        args.push("-X".to_owned());
        args.push(format!(
            "find-renames={}%",
            options.rename_threshold.clamp(1, 100)
        ));
    }
    args.push(target.to_owned());
    args
}

pub fn merge_commit(root: impl AsRef<Path>, target: &str, options: MergeOptions) -> Result<()> {
    let root = root.as_ref();
    let args = merge_commit_args(target, options);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    if options.rebase {
        return git_output(root, &refs).map(|_| ());
    }
    if merge_in_progress(root) {
        return Ok(());
    }
    git_output_allowing_new_conflicts(root, &refs)
}

fn archive_args(output_path: &Path, folder_prefix: &str, target: &str) -> Vec<String> {
    let format = if output_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
    {
        "zip"
    } else {
        "tar"
    };
    let mut args = vec![
        "archive".to_owned(),
        format!("--format={format}"),
        "--output".to_owned(),
        output_path.to_string_lossy().to_string(),
    ];
    let folder_prefix = folder_prefix.trim();
    if !folder_prefix.is_empty() {
        let normalized_prefix = if folder_prefix.ends_with('/') || folder_prefix.ends_with('\\') {
            folder_prefix.replace('\\', "/")
        } else {
            format!("{}/", folder_prefix.replace('\\', "/"))
        };
        args.push(format!("--prefix={normalized_prefix}"));
    }
    args.push(target.trim().to_owned());
    args
}

pub fn archive(
    root: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    folder_prefix: &str,
    target: &str,
) -> Result<()> {
    let target = target.trim();
    if target.is_empty() {
        return Err(anyhow!("archive target is required"));
    }
    let output_path = output_path.as_ref();
    if output_path.as_os_str().is_empty() {
        return Err(anyhow!("archive output path is required"));
    }
    let args = archive_args(output_path, folder_prefix, target);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    git_output(root.as_ref(), &refs).map(|_| ())
}

pub fn rebase_current_onto(root: impl AsRef<Path>, name: &str) -> Result<()> {
    git_output(root.as_ref(), &["rebase", name]).map(|_| ())
}

pub fn rebase_continue(root: impl AsRef<Path>) -> Result<()> {
    rebase_control(root.as_ref(), "--continue")
}

pub fn rebase_skip(root: impl AsRef<Path>) -> Result<()> {
    rebase_control(root.as_ref(), "--skip")
}

pub fn rebase_abort(root: impl AsRef<Path>) -> Result<()> {
    rebase_control(root.as_ref(), "--abort")
}

fn rebase_control(root: &Path, action: &str) -> Result<()> {
    let output = git_command()
        .arg("-C")
        .arg(root)
        .env("GIT_EDITOR", "true")
        .env("GIT_SEQUENCE_EDITOR", "true")
        .args(["rebase", action])
        .output()
        .with_context(|| format!("failed to run git rebase {action}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let detail = if stderr.is_empty() { stdout } else { stderr };
        return Err(anyhow!("git rebase {action} failed: {detail}"));
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractiveRebaseAction {
    Pick,
    Squash,
    Drop,
}

impl InteractiveRebaseAction {
    fn todo_command(self) -> &'static str {
        match self {
            Self::Pick => "pick",
            Self::Squash => "squash",
            Self::Drop => "drop",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractiveRebaseTodoItem {
    pub action: InteractiveRebaseAction,
    pub hash: String,
    pub subject: String,
}

fn interactive_rebase_todo(items: &[InteractiveRebaseTodoItem]) -> String {
    let mut todo = String::new();
    for item in items {
        let subject = item.subject.replace(['\r', '\n'], " ");
        todo.push_str(item.action.todo_command());
        todo.push(' ');
        todo.push_str(item.hash.trim());
        if !subject.trim().is_empty() {
            todo.push(' ');
            todo.push_str(subject.trim());
        }
        todo.push('\n');
    }
    todo
}

pub fn interactive_rebase(
    root: impl AsRef<Path>,
    base: &str,
    items: &[InteractiveRebaseTodoItem],
) -> Result<()> {
    let root = root.as_ref();
    if rebase_in_progress(root) {
        return Err(anyhow!(
            "interactive rebase is already in progress; run git rebase --continue, --abort, or --skip first"
        ));
    }

    let base = base.trim();
    if base.is_empty() {
        return Err(anyhow!("interactive rebase base is required"));
    }
    if items.is_empty() {
        return Err(anyhow!("interactive rebase requires at least one commit"));
    }
    if items
        .first()
        .is_some_and(|item| item.action == InteractiveRebaseAction::Squash)
    {
        return Err(anyhow!("first interactive rebase commit cannot be squash"));
    }

    let temp_dir = interactive_rebase_temp_dir();
    fs::create_dir_all(&temp_dir).context("failed to create interactive rebase temp dir")?;
    let todo_path = temp_dir.join("git-rebase-todo");
    fs::write(&todo_path, interactive_rebase_todo(items))
        .context("failed to write interactive rebase todo")?;
    let sequence_editor = write_rebase_sequence_editor(&temp_dir, &todo_path)?;
    let editor = write_rebase_noop_editor(&temp_dir)?;

    let mut command = git_command();
    command
        .arg("-C")
        .arg(root)
        .env("GIT_SEQUENCE_EDITOR", sequence_editor)
        .env("GIT_EDITOR", editor)
        .args(["rebase", "-i", "--autosquash"]);
    if base == "--root" {
        command.arg("--root");
    } else {
        command.arg(base);
    }

    let output = command
        .output()
        .with_context(|| format!("failed to run git rebase -i --autosquash {base}"))?;
    let _ = fs::remove_dir_all(&temp_dir);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let detail = if stderr.is_empty() { stdout } else { stderr };
        return Err(anyhow!(
            "git rebase -i --autosquash {} failed: {}",
            base,
            detail
        ));
    }

    Ok(())
}

fn interactive_rebase_temp_dir() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    env::temp_dir().join(format!("git-agent-rebase-{}-{stamp}", std::process::id()))
}

#[cfg(target_os = "windows")]
fn write_rebase_sequence_editor(dir: &Path, todo_path: &Path) -> Result<String> {
    let script_path = dir.join("sequence-editor.cmd");
    fs::write(
        &script_path,
        format!(
            "@echo off\r\ncopy /Y \"{}\" \"%~1\" >NUL\r\n",
            todo_path.display()
        ),
    )
    .context("failed to write interactive rebase sequence editor")?;
    Ok(format!("\"{}\"", script_path.display()))
}

#[cfg(not(target_os = "windows"))]
fn write_rebase_sequence_editor(dir: &Path, todo_path: &Path) -> Result<String> {
    let script_path = dir.join("sequence-editor.sh");
    fs::write(
        &script_path,
        format!("#!/bin/sh\ncp '{}' \"$1\"\n", todo_path.display()),
    )
    .context("failed to write interactive rebase sequence editor")?;
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
        .context("failed to mark interactive rebase sequence editor executable")?;
    Ok(script_path.to_string_lossy().to_string())
}

#[cfg(target_os = "windows")]
fn write_rebase_noop_editor(dir: &Path) -> Result<String> {
    let script_path = dir.join("noop-editor.cmd");
    fs::write(&script_path, "@echo off\r\nexit /b 0\r\n")
        .context("failed to write interactive rebase noop editor")?;
    Ok(format!("\"{}\"", script_path.display()))
}

#[cfg(not(target_os = "windows"))]
fn write_rebase_noop_editor(dir: &Path) -> Result<String> {
    let script_path = dir.join("noop-editor.sh");
    fs::write(&script_path, "#!/bin/sh\nexit 0\n")
        .context("failed to write interactive rebase noop editor")?;
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
        .context("failed to mark interactive rebase noop editor executable")?;
    Ok(script_path.to_string_lossy().to_string())
}

pub fn rename_branch(root: impl AsRef<Path>, old_name: &str, new_name: &str) -> Result<()> {
    let args = rename_branch_args(old_name, new_name);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    git_output(root.as_ref(), &refs).map(|_| ())
}

pub fn fetch_remote_branch(root: impl AsRef<Path>, remote_branch: &str) -> Result<()> {
    let args = fetch_remote_branch_args(remote_branch)?;
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    git_output(root.as_ref(), &refs).map(|_| ())
}

pub fn set_branch_upstream(
    root: impl AsRef<Path>,
    local_branch: &str,
    remote_branch: &str,
) -> Result<()> {
    let args = set_branch_upstream_args(local_branch, remote_branch);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    git_output(root.as_ref(), &refs).map(|_| ())
}

pub fn unset_branch_upstream(root: impl AsRef<Path>, local_branch: &str) -> Result<()> {
    let args = unset_branch_upstream_args(local_branch);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    git_output(root.as_ref(), &refs).map(|_| ())
}

pub fn delete_branch(root: impl AsRef<Path>, name: &str, force: bool) -> Result<()> {
    if force {
        git_output(root.as_ref(), &["branch", "-D", name]).map(|_| ())
    } else {
        git_output(root.as_ref(), &["branch", "-d", name]).map(|_| ())
    }
}

pub fn delete_remote_branch(root: impl AsRef<Path>, remote_branch: &str) -> Result<()> {
    let (remote, branch) = split_remote_branch(remote_branch)?;
    git_output(root.as_ref(), &["push", remote, "--delete", branch]).map(|_| ())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmoduleAddOptions {
    pub source: String,
    pub local_path: String,
    pub source_branch: String,
    pub recursive: bool,
}

fn add_submodule_args(options: &SubmoduleAddOptions) -> Vec<String> {
    let mut args = vec!["submodule".to_owned(), "add".to_owned()];
    let source_branch = options.source_branch.trim();
    if !source_branch.is_empty() {
        args.push("-b".to_owned());
        args.push(source_branch.to_owned());
    }
    args.push(options.source.trim().to_owned());
    args.push(options.local_path.trim().replace('\\', "/"));
    args
}

fn submodule_recursive_update_args(local_path: &str) -> Vec<String> {
    vec![
        "submodule".to_owned(),
        "update".to_owned(),
        "--init".to_owned(),
        "--recursive".to_owned(),
        "--".to_owned(),
        local_path.trim().replace('\\', "/"),
    ]
}

pub fn add_submodule(root: impl AsRef<Path>, options: SubmoduleAddOptions) -> Result<()> {
    if options.source.trim().is_empty() {
        return Err(anyhow!("submodule source is required"));
    }
    if options.local_path.trim().is_empty() {
        return Err(anyhow!("submodule local path is required"));
    }
    let args = add_submodule_args(&options);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    git_output(root.as_ref(), &refs)?;
    if options.recursive {
        let update_args = submodule_recursive_update_args(&options.local_path);
        let refs = update_args.iter().map(String::as_str).collect::<Vec<_>>();
        git_output(root.as_ref(), &refs)?;
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubtreeAddOptions {
    pub source: String,
    pub local_path: String,
    pub ref_name: String,
    pub squash: bool,
}

fn add_subtree_args(options: &SubtreeAddOptions) -> Vec<String> {
    let mut args = vec![
        "subtree".to_owned(),
        "add".to_owned(),
        "--prefix".to_owned(),
        options.local_path.trim().replace('\\', "/"),
        options.source.trim().to_owned(),
        options.ref_name.trim().to_owned(),
    ];
    if options.squash {
        args.push("--squash".to_owned());
    }
    args
}

pub fn add_subtree(root: impl AsRef<Path>, options: SubtreeAddOptions) -> Result<()> {
    if options.source.trim().is_empty() {
        return Err(anyhow!("subtree source is required"));
    }
    if options.local_path.trim().is_empty() {
        return Err(anyhow!("subtree local path is required"));
    }
    if options.ref_name.trim().is_empty() {
        return Err(anyhow!("subtree ref is required"));
    }
    let args = add_subtree_args(&options);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    git_output(root.as_ref(), &refs).map(|_| ())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LfsTrackOptions {
    pub original_patterns: Vec<String>,
    pub patterns: Vec<String>,
}

fn lfs_install_args() -> Vec<String> {
    vec!["lfs".to_owned(), "install".to_owned(), "--local".to_owned()]
}

fn lfs_track_args(pattern: &str) -> Vec<String> {
    vec![
        "lfs".to_owned(),
        "track".to_owned(),
        pattern.trim().to_owned(),
    ]
}

fn lfs_untrack_args(pattern: &str) -> Vec<String> {
    vec![
        "lfs".to_owned(),
        "untrack".to_owned(),
        pattern.trim().to_owned(),
    ]
}

fn lfs_simple_args(action: &str) -> Vec<String> {
    vec!["lfs".to_owned(), action.to_owned()]
}

fn run_git_args(root: &Path, args: Vec<String>) -> Result<String> {
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    git_output(root, &refs)
}

pub fn lfs_tracked_patterns(root: impl AsRef<Path>) -> Result<Vec<String>> {
    let attributes_path = root.as_ref().join(".gitattributes");
    let text = fs::read_to_string(attributes_path).unwrap_or_default();
    Ok(parse_lfs_gitattributes_patterns(&text))
}

fn parse_lfs_gitattributes_patterns(text: &str) -> Vec<String> {
    normalized_lfs_patterns(
        &text
            .lines()
            .filter(|line| line.contains("filter=lfs"))
            .filter_map(|line| line.split_whitespace().next())
            .map(unquote_gitattributes_pattern)
            .collect::<Vec<_>>(),
    )
}

fn unquote_gitattributes_pattern(pattern: &str) -> String {
    pattern
        .trim()
        .trim_matches('"')
        .replace("\\ ", " ")
        .to_owned()
}

fn normalized_lfs_patterns(patterns: &[String]) -> Vec<String> {
    let mut normalized = Vec::<String>::new();
    for pattern in patterns {
        let pattern = pattern.trim();
        if pattern.is_empty() || normalized.iter().any(|existing| existing == pattern) {
            continue;
        }
        normalized.push(pattern.to_owned());
    }
    normalized
}

pub fn configure_lfs_patterns(root: impl AsRef<Path>, options: LfsTrackOptions) -> Result<()> {
    let root = root.as_ref();
    let original = normalized_lfs_patterns(&options.original_patterns);
    let patterns = normalized_lfs_patterns(&options.patterns);
    run_git_args(root, lfs_install_args())?;

    for pattern in original
        .iter()
        .filter(|pattern| !patterns.iter().any(|candidate| candidate == *pattern))
    {
        run_git_args(root, lfs_untrack_args(pattern))?;
    }
    for pattern in patterns
        .iter()
        .filter(|pattern| !original.iter().any(|candidate| candidate == *pattern))
    {
        run_git_args(root, lfs_track_args(pattern))?;
    }

    Ok(())
}

pub fn lfs_pull(root: impl AsRef<Path>) -> Result<()> {
    run_git_args(root.as_ref(), lfs_simple_args("pull")).map(|_| ())
}

pub fn lfs_fetch(root: impl AsRef<Path>) -> Result<()> {
    run_git_args(root.as_ref(), lfs_simple_args("fetch")).map(|_| ())
}

pub fn lfs_checkout(root: impl AsRef<Path>) -> Result<()> {
    run_git_args(root.as_ref(), lfs_simple_args("checkout")).map(|_| ())
}

pub fn lfs_prune(root: impl AsRef<Path>) -> Result<()> {
    run_git_args(root.as_ref(), lfs_simple_args("prune")).map(|_| ())
}

fn rename_branch_args(old_name: &str, new_name: &str) -> Vec<String> {
    vec![
        "branch".to_owned(),
        "-m".to_owned(),
        old_name.to_owned(),
        new_name.to_owned(),
    ]
}

fn fetch_remote_branch_args(remote_branch: &str) -> Result<Vec<String>> {
    let (remote, branch) = split_remote_branch(remote_branch)?;
    Ok(vec![
        "fetch".to_owned(),
        remote.to_owned(),
        branch.to_owned(),
    ])
}

fn push_branch_to_remote_args(remote: &str, branch: &str) -> Vec<String> {
    vec![
        "push".to_owned(),
        "-u".to_owned(),
        remote.to_owned(),
        branch.to_owned(),
    ]
}

fn set_branch_upstream_args(local_branch: &str, remote_branch: &str) -> Vec<String> {
    vec![
        "branch".to_owned(),
        format!("--set-upstream-to={remote_branch}"),
        local_branch.to_owned(),
    ]
}

fn unset_branch_upstream_args(local_branch: &str) -> Vec<String> {
    vec![
        "branch".to_owned(),
        "--unset-upstream".to_owned(),
        local_branch.to_owned(),
    ]
}

fn split_remote_branch(remote_branch: &str) -> Result<(&str, &str)> {
    let (remote, branch) = remote_branch
        .split_once('/')
        .ok_or_else(|| anyhow!("remote branch must look like remote/name"))?;
    if remote.trim().is_empty() || branch.trim().is_empty() {
        return Err(anyhow!("remote branch must look like remote/name"));
    }
    Ok((remote, branch))
}

pub fn create_tag(root: impl AsRef<Path>, name: &str, hash: &str) -> Result<()> {
    git_output(root.as_ref(), &["tag", name, hash]).map(|_| ())
}

pub fn create_tag_at_head(root: impl AsRef<Path>, name: &str) -> Result<()> {
    git_output(root.as_ref(), &["tag", name]).map(|_| ())
}

pub fn checkout_tag(root: impl AsRef<Path>, name: &str) -> Result<()> {
    git_output(root.as_ref(), &["checkout", name]).map(|_| ())
}

pub fn delete_tag(root: impl AsRef<Path>, name: &str) -> Result<()> {
    git_output(root.as_ref(), &["tag", "-d", name]).map(|_| ())
}

pub fn stage_path(root: impl AsRef<Path>, path: &str) -> Result<()> {
    git_output(root.as_ref(), &["add", "--", path]).map(|_| ())
}

pub fn stage_all(root: impl AsRef<Path>) -> Result<()> {
    git_output(root.as_ref(), &["add", "--all"]).map(|_| ())
}

pub fn unstage_path(root: impl AsRef<Path>, path: &str) -> Result<()> {
    git_output(root.as_ref(), &["restore", "--staged", "--", path]).map(|_| ())
}

pub fn unstage_all(root: impl AsRef<Path>) -> Result<()> {
    git_output(root.as_ref(), &["restore", "--staged", "--", "."]).map(|_| ())
}

pub fn discard_path(root: impl AsRef<Path>, path: &str) -> Result<()> {
    git_output(root.as_ref(), &["checkout", "--", path]).map(|_| ())
}

pub fn clean_untracked_path(root: impl AsRef<Path>, path: &str) -> Result<()> {
    git_output(root.as_ref(), &["clean", "-fd", "--", path]).map(|_| ())
}

pub fn add_to_gitignore(root: impl AsRef<Path>, pattern: &str) -> Result<()> {
    let root = root.as_ref();
    let pattern = normalize_gitignore_pattern(pattern);
    if pattern.is_empty() {
        return Ok(());
    }

    let ignore_path = root.join(".gitignore");
    let existing = fs::read_to_string(&ignore_path).unwrap_or_default();
    if existing.lines().any(|line| line.trim() == pattern) {
        return Ok(());
    }

    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(&pattern);
    next.push('\n');
    fs::write(&ignore_path, next)
        .with_context(|| format!("write {}", ignore_path.display()))
        .map(|_| ())
}

fn normalize_gitignore_pattern(pattern: &str) -> String {
    let mut normalized = pattern.replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_owned();
    }
    normalized.trim().to_owned()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConflictSide {
    Ours,
    Theirs,
}

pub fn accept_conflict_side(root: impl AsRef<Path>, path: &str, side: ConflictSide) -> Result<()> {
    let root = root.as_ref();
    let selector = match side {
        ConflictSide::Ours => "--ours",
        ConflictSide::Theirs => "--theirs",
    };
    git_output(root, &["checkout", selector, "--", path])?;
    git_output(root, &["add", "--", path]).map(|_| ())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CommitOptions {
    pub amend: bool,
    pub no_verify: bool,
    pub gpg_sign: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PullOptions {
    pub commit_merge: bool,
    pub include_tags: bool,
    pub force_merge_commit: bool,
    pub rebase: bool,
}

impl Default for PullOptions {
    fn default() -> Self {
        Self {
            commit_merge: true,
            include_tags: false,
            force_merge_commit: false,
            rebase: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FetchOptions {
    pub all_remotes: bool,
    pub prune_tracking: bool,
    pub fetch_tags: bool,
    pub force_tags: bool,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            all_remotes: true,
            prune_tracking: true,
            fetch_tags: false,
            force_tags: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PushBranchSpec {
    pub local_branch: String,
    pub remote_branch: String,
    pub track: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PushOptions {
    pub push_tags: bool,
    pub force: bool,
}

pub fn commit_args(message: &str, options: CommitOptions) -> Vec<String> {
    let mut args = vec!["commit".to_owned()];
    if options.amend {
        args.push("--amend".to_owned());
    }
    if options.no_verify {
        args.push("--no-verify".to_owned());
    }
    if options.gpg_sign {
        args.push("-S".to_owned());
    }
    args.push("-m".to_owned());
    args.push(message.to_owned());
    args
}

pub fn commit_with_options(
    root: impl AsRef<Path>,
    message: &str,
    options: CommitOptions,
) -> Result<()> {
    let args = commit_args(message, options);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    git_output(root.as_ref(), &refs).map(|_| ())
}

pub fn fetch_with_options(root: impl AsRef<Path>, options: FetchOptions) -> Result<()> {
    let args = fetch_args(options);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    git_output(root.as_ref(), &refs).map(|_| ())
}

fn fetch_args(options: FetchOptions) -> Vec<String> {
    let mut args = vec!["fetch".to_owned()];
    if options.all_remotes {
        args.push("--all".to_owned());
    }
    if options.prune_tracking {
        args.push("--prune".to_owned());
    }
    if options.fetch_tags {
        args.push("--tags".to_owned());
        if options.force_tags {
            args.push("--force".to_owned());
        }
    }
    args
}

pub fn fetch_remote(root: impl AsRef<Path>, remote: &str) -> Result<()> {
    git_output(root.as_ref(), &["fetch", remote, "--prune"]).map(|_| ())
}

pub fn pull_from_remote(
    root: impl AsRef<Path>,
    remote: &str,
    branch: &str,
    options: PullOptions,
) -> Result<()> {
    let args = pull_args(remote, branch, options);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    git_output(root.as_ref(), &refs).map(|_| ())
}

fn pull_args(remote: &str, branch: &str, options: PullOptions) -> Vec<String> {
    let mut args = vec!["pull".to_owned()];
    if options.rebase {
        args.push("--rebase".to_owned());
    } else {
        if !options.commit_merge {
            args.push("--no-commit".to_owned());
        }
        if options.include_tags {
            args.push("--tags".to_owned());
        }
        if options.force_merge_commit {
            args.push("--no-ff".to_owned());
        }
    }
    if options.rebase && options.include_tags {
        args.push("--tags".to_owned());
    }
    args.push(remote.to_owned());
    args.push(branch.to_owned());
    args
}

pub fn pull(root: impl AsRef<Path>) -> Result<()> {
    git_output(root.as_ref(), &["pull"]).map(|_| ())
}

pub fn push(root: impl AsRef<Path>) -> Result<()> {
    git_output(root.as_ref(), &["push"]).map(|_| ())
}

pub fn push_set_upstream(root: impl AsRef<Path>, remote: &str, branch: &str) -> Result<()> {
    git_output(root.as_ref(), &["push", "-u", remote, branch]).map(|_| ())
}

pub fn push_selected(
    root: impl AsRef<Path>,
    remote: &str,
    branches: &[PushBranchSpec],
    options: PushOptions,
) -> Result<()> {
    let root = root.as_ref();
    for branch in branches {
        let args = push_branch_args(remote, branch, options.force);
        let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        git_output(root, &refs)?;
    }
    if options.push_tags {
        let args = push_tags_args(remote);
        let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        git_output(root, &refs)?;
    }
    Ok(())
}

fn push_branch_args(remote: &str, branch: &PushBranchSpec, force: bool) -> Vec<String> {
    let mut args = vec!["push".to_owned()];
    if force {
        args.push("--force-with-lease".to_owned());
    }
    if branch.track {
        args.push("-u".to_owned());
    }
    args.push(remote.to_owned());
    args.push(format!(
        "{}:refs/heads/{}",
        branch.local_branch, branch.remote_branch
    ));
    args
}

fn push_tags_args(remote: &str) -> Vec<String> {
    vec!["push".to_owned(), remote.to_owned(), "--tags".to_owned()]
}

#[cfg(test)]
fn push_tag_args(remote: &str, tag: &str) -> Vec<String> {
    vec![
        "push".to_owned(),
        remote.to_owned(),
        format!("refs/tags/{tag}"),
    ]
}

pub fn push_tag(root: impl AsRef<Path>, remote: &str, tag: &str) -> Result<()> {
    let tag_ref = format!("refs/tags/{tag}");
    git_output(root.as_ref(), &["push", remote, &tag_ref]).map(|_| ())
}

pub fn github_credential_manager_login() -> Result<()> {
    let output = git_command()
        .args(["credential-manager", "github", "login"])
        .output()
        .context("failed to run git credential-manager github login")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let message = if stderr.is_empty() { stdout } else { stderr };
        return Err(anyhow!(
            "git credential-manager github login failed: {}",
            message
        ));
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StashOptions {
    pub staged_files: bool,
    pub keep_staged: bool,
    pub include_untracked: bool,
    pub include_ignored: bool,
}

fn stash_push_args(message: &str, options: StashOptions) -> Vec<String> {
    let mut args = vec!["stash".to_owned(), "push".to_owned()];
    if options.staged_files {
        args.push("--staged".to_owned());
    }
    if options.keep_staged {
        args.push("--keep-index".to_owned());
    }
    if options.include_ignored {
        args.push("--all".to_owned());
    } else if options.include_untracked {
        args.push("--include-untracked".to_owned());
    }
    let message = message.trim();
    if !message.is_empty() {
        args.push("-m".to_owned());
        args.push(message.to_owned());
    }
    args
}

pub fn stash_push(root: impl AsRef<Path>, message: &str, options: StashOptions) -> Result<()> {
    let args = stash_push_args(message, options);
    let args = args.iter().map(String::as_str).collect::<Vec<_>>();
    git_output(root.as_ref(), &args).map(|_| ())
}

pub fn stash_apply(root: impl AsRef<Path>, selector: &str) -> Result<()> {
    git_output(root.as_ref(), &["stash", "apply", selector]).map(|_| ())
}

pub fn stash_pop(root: impl AsRef<Path>, selector: &str) -> Result<()> {
    git_output(root.as_ref(), &["stash", "pop", selector]).map(|_| ())
}

pub fn stash_drop(root: impl AsRef<Path>, selector: &str) -> Result<()> {
    git_output(root.as_ref(), &["stash", "drop", selector]).map(|_| ())
}

fn parse_file_changes(output: &str) -> Vec<FileChange> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let status = parts.next()?.trim();
            let paths = parts.map(str::to_owned).collect::<Vec<_>>();
            let path = paths.join(" -> ");
            let diff_path = paths.last().cloned().unwrap_or_default();
            (!status.is_empty() && !path.is_empty()).then(|| FileChange {
                status: status.to_owned(),
                path,
                diff_path,
            })
        })
        .collect()
}

fn parse_status_entries(lines: &[String]) -> (Vec<WorktreeFile>, Vec<WorktreeFile>) {
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();

    for line in lines {
        if let Some(file) = parse_status_entry(line) {
            if file.index_status != ' ' && file.index_status != '?' {
                staged.push(file.clone());
            }
            if file.worktree_status != ' ' || file.index_status == '?' {
                unstaged.push(file);
            }
        }
    }

    (staged, unstaged)
}

fn parse_status_entry(line: &str) -> Option<WorktreeFile> {
    let mut chars = line.chars();
    let index_status = chars.next().unwrap_or(' ');
    let worktree_status = chars.next().unwrap_or(' ');
    let raw_path = line.get(3..)?.trim();
    if raw_path.is_empty() {
        return None;
    }

    let path = raw_path
        .split(" -> ")
        .last()
        .unwrap_or(raw_path)
        .trim()
        .to_owned();

    Some(WorktreeFile {
        index_status,
        worktree_status,
        path,
        display_path: raw_path.to_owned(),
    })
}

fn load_branches(root: &Path) -> Result<Vec<Branch>> {
    let output = git_output(
        root,
        &[
            "branch",
            "--all",
            "--format=%(HEAD)%09%(refname:short)%09%(refname)%09%(upstream:short)",
        ],
    )?;

    let mut branches = parse_branches(&output);
    for branch in branches.iter_mut().filter(|branch| !branch.remote) {
        let Some(upstream_name) = branch
            .upstream
            .as_ref()
            .map(|upstream| upstream.name.clone())
        else {
            continue;
        };
        if let Ok((ahead, behind)) = load_branch_upstream_counts(root, &branch.name, &upstream_name)
        {
            branch.upstream = Some(UpstreamStatus {
                name: upstream_name,
                ahead,
                behind,
            });
        }
    }
    Ok(branches)
}

fn parse_branches(output: &str) -> Vec<Branch> {
    output
        .lines()
        .filter_map(|line| {
            let parts = if line.contains('\t') {
                line.split('\t').collect::<Vec<_>>()
            } else if line.contains('\x1f') {
                line.split('\x1f').collect::<Vec<_>>()
            } else {
                line.split("%x1f").collect::<Vec<_>>()
            };
            let head = parts.first().copied().unwrap_or_default();
            let name = parts.get(1)?.trim().to_owned();
            let refname = parts.get(2).copied().unwrap_or_default();
            let upstream_name = parts.get(3).copied().unwrap_or_default().trim();
            let local = refname.starts_with("refs/heads/");
            let remote = refname.starts_with("refs/remotes/");
            if !local && !remote {
                return None;
            }
            let upstream = (!remote && !upstream_name.is_empty()).then(|| UpstreamStatus {
                name: upstream_name.to_owned(),
                ahead: 0,
                behind: 0,
            });
            (!name.is_empty()).then(|| Branch {
                remote,
                current: head.trim() == "*",
                name,
                upstream,
            })
        })
        .collect()
}

fn load_branch_upstream_counts(
    root: &Path,
    branch: &str,
    upstream: &str,
) -> Result<(usize, usize)> {
    let range = format!("{branch}...{upstream}");
    let counts = git_output(root, &["rev-list", "--left-right", "--count", &range])?;
    let mut parts = counts.split_whitespace();
    let ahead = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let behind = parts.next().unwrap_or("0").parse().unwrap_or(0);
    Ok((ahead, behind))
}

fn load_remotes(root: &Path) -> Result<Vec<Remote>> {
    let output = git_output(root, &["remote", "-v"])?;
    let mut remotes = Vec::<Remote>::new();

    for line in output.lines() {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else {
            continue;
        };
        let Some(url) = parts.next() else {
            continue;
        };
        let kind = parts.next().unwrap_or_default();
        let remote = if let Some(existing) = remotes.iter_mut().find(|remote| remote.name == name) {
            existing
        } else {
            remotes.push(Remote {
                name: name.to_owned(),
                fetch_url: String::new(),
                push_url: String::new(),
            });
            remotes.last_mut().expect("remote was just pushed")
        };

        if kind == "(fetch)" {
            remote.fetch_url = url.to_owned();
        } else if kind == "(push)" {
            remote.push_url = url.to_owned();
        }
    }

    Ok(remotes)
}

fn load_merge_message(root: &Path, current_branch: &str) -> Option<String> {
    let path = git_path(root, "MERGE_MSG")?;
    let message = fs::read_to_string(path).ok()?;
    let message = message.trim_end().to_owned();
    (!message.trim().is_empty()).then(|| format_merge_message_for_branch(&message, current_branch))
}

fn format_merge_message_for_branch(message: &str, current_branch: &str) -> String {
    if current_branch.trim().is_empty() || current_branch == "HEAD" {
        return message.to_owned();
    }
    let Some(first_line_end) = message.find('\n') else {
        return format_merge_message_subject_for_branch(message, current_branch);
    };
    let subject = &message[..first_line_end];
    let rest = &message[first_line_end..];
    format!(
        "{}{}",
        format_merge_message_subject_for_branch(subject, current_branch),
        rest
    )
}

fn format_merge_message_subject_for_branch(subject: &str, current_branch: &str) -> String {
    if (subject.starts_with("Merge branch ")
        || subject.starts_with("Merge remote-tracking branch "))
        && !subject.contains(" into ")
    {
        format!("{subject} into {current_branch}")
    } else {
        subject.to_owned()
    }
}

fn merge_in_progress(root: &Path) -> bool {
    git_path(root, "MERGE_HEAD").is_some_and(|path| path.exists())
}

fn rebase_in_progress(root: &Path) -> bool {
    ["rebase-merge", "rebase-apply"]
        .iter()
        .any(|name| git_path(root, name).is_some_and(|path| path.exists()))
}

pub fn repository_rebase_in_progress(root: impl AsRef<Path>) -> bool {
    rebase_in_progress(root.as_ref())
}

fn load_repository_config(root: &Path) -> RepositoryConfig {
    let config_path = git_path(root, "config").unwrap_or_else(|| root.join(".git").join("config"));
    let gitignore_path = root.join(".gitignore");
    let local_user_name = git_config_value(root, &["config", "--local", "--get", "user.name"]);
    let local_user_email = git_config_value(root, &["config", "--local", "--get", "user.email"]);
    let effective_user_name = if local_user_name.is_empty() {
        git_config_value(root, &["config", "--get", "user.name"])
    } else {
        local_user_name.clone()
    };
    let effective_user_email = if local_user_email.is_empty() {
        git_config_value(root, &["config", "--get", "user.email"])
    } else {
        local_user_email.clone()
    };

    RepositoryConfig {
        config_path,
        gitignore_path,
        user_name: effective_user_name,
        user_email: effective_user_email,
        uses_global_user: local_user_name.is_empty() && local_user_email.is_empty(),
    }
}

fn git_config_value(root: &Path, args: &[&str]) -> String {
    git_output(root, args).unwrap_or_default().trim().to_owned()
}

fn git_path(root: &Path, name: &str) -> Option<PathBuf> {
    let raw = git_output(root, &["rev-parse", "--git-path", name]).ok()?;
    let path = PathBuf::from(raw.trim());
    if path.is_absolute() {
        Some(path)
    } else {
        Some(root.join(path))
    }
}

fn load_upstream_status(root: &Path) -> Result<Option<UpstreamStatus>> {
    let upstream = git_output(
        root,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    );
    let Ok(upstream) = upstream else {
        return Ok(None);
    };
    let upstream = upstream.trim().to_owned();
    if upstream.is_empty() {
        return Ok(None);
    }

    let counts = git_output(
        root,
        &["rev-list", "--left-right", "--count", "HEAD...@{u}"],
    )?;
    let mut parts = counts.split_whitespace();
    let ahead = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let behind = parts.next().unwrap_or("0").parse().unwrap_or(0);

    Ok(Some(UpstreamStatus {
        name: upstream,
        ahead,
        behind,
    }))
}

fn load_stashes(root: &Path) -> Result<Vec<StashEntry>> {
    let output = git_output(root, &["stash", "list", "--format=%gd%x1f%cr%x1f%gs"])?;
    Ok(parse_stashes(&output))
}

fn load_tags(root: &Path) -> Result<Vec<Tag>> {
    let output = git_output(
        root,
        &[
            "tag",
            "--list",
            "--sort=-creatordate",
            "--format=%(refname:short)%09%(objectname:short)%09%(subject)",
        ],
    )?;
    Ok(parse_tags(&output))
}

fn parse_tags(output: &str) -> Vec<Tag> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = if line.contains('\t') {
                line.split('\t').collect::<Vec<_>>()
            } else if line.contains('\x1f') {
                line.split('\x1f').collect::<Vec<_>>()
            } else {
                line.split("%x1f").collect::<Vec<_>>()
            }
            .into_iter();
            let name = parts.next()?.trim();
            let target = parts.next().unwrap_or_default().trim();
            let subject = parts.next().unwrap_or_default().trim();
            (!name.is_empty()).then(|| Tag {
                name: name.to_owned(),
                target: target.to_owned(),
                subject: subject.to_owned(),
            })
        })
        .collect()
}

fn parse_stashes(output: &str) -> Vec<StashEntry> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\x1f');
            let selector = parts.next()?.trim();
            let relative_time = parts.next().unwrap_or_default().trim();
            let message = parts.next().unwrap_or_default().trim();
            (!selector.is_empty()).then(|| StashEntry {
                selector: selector.to_owned(),
                relative_time: relative_time.to_owned(),
                message: message.to_owned(),
            })
        })
        .collect()
}

fn discover_root(path: &Path) -> Result<PathBuf> {
    let output = git_command()
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .with_context(|| format!("failed to run git in {}", path.display()))?;

    if !output.status.success() {
        return Err(anyhow!("{} is not a git repository", path.display()));
    }

    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim(),
    ))
}

fn load_commits(
    root: &Path,
    limit: usize,
    order: CommitOrder,
    scope: CommitScope,
) -> Result<Vec<Commit>> {
    if !has_head(root) {
        return Ok(Vec::new());
    }

    let max_count = format!("--max-count={limit}");
    let order_arg = match order {
        CommitOrder::Date => "--date-order",
        CommitOrder::Topology => "--topo-order",
    };
    let mut args = vec![
        "log",
        order_arg,
        &max_count,
        "--date=format-local:%Y-%m-%d %H:%M",
        "--decorate=short",
    ];
    if scope == CommitScope::AllBranches {
        args.push("--all");
    }
    args.push("--format=%H%x1f%P%x1f%an%x1f%cd%x1f%ar%x1f%D%x1f%s");
    let output = git_output(root, &args)?;

    Ok(output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\x1f');
            let hash = parts.next()?.to_owned();
            let parents = parts
                .next()
                .unwrap_or_default()
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let author = parts.next().unwrap_or_default().to_owned();
            let date = parts.next().unwrap_or_default().to_owned();
            let relative_time = parts.next().unwrap_or_default().to_owned();
            let refs = parts
                .next()
                .unwrap_or_default()
                .split(", ")
                .filter_map(|name| {
                    let name = name.trim();
                    (!name.is_empty()).then(|| name.to_owned())
                })
                .collect::<Vec<_>>();
            let subject = parts.next().unwrap_or_default().to_owned();
            let short_hash = hash.chars().take(8).collect();

            Some(Commit {
                hash,
                short_hash,
                parents,
                author,
                date,
                relative_time,
                subject,
                refs,
            })
        })
        .collect())
}

fn has_head(root: &Path) -> bool {
    git_command()
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--verify", "HEAD"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn git_output(root: &Path, args: &[&str]) -> Result<String> {
    let output = git_command()
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;

    if !output.status.success() {
        return Err(anyhow!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_output_allowing_new_conflicts(root: &Path, args: &[&str]) -> Result<()> {
    let had_conflicts = has_unmerged_paths(root);
    let output = git_command()
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;

    if output.status.success() || (!had_conflicts && has_unmerged_paths(root)) {
        return Ok(());
    }

    Err(anyhow!(
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn has_unmerged_paths(root: &Path) -> bool {
    git_command()
        .arg("-C")
        .arg(root)
        .args(["diff", "--name-only", "--diff-filter=U"])
        .output()
        .map(|output| output.status.success() && !output.stdout.is_empty())
        .unwrap_or(false)
}

fn git_command() -> Command {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("git");
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("git")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_commit_limit_supports_large_repositories() {
        assert!(HISTORY_COMMIT_LIMIT >= 50_000);
    }

    #[test]
    fn parses_changed_file_search_log_by_path() {
        let output = "\x1eabc123\nsrc/styles/pretty.scss\nREADME.md\n\x1edef456\nsrc/main.rs\n\x1efed789\ncomponents/AntButton.vue\n";

        let matches = parse_changed_file_search_log(output, "ant");

        assert_eq!(matches, vec!["fed789"]);
    }

    #[test]
    fn parses_content_search_hash_lines_and_escapes_literal_regex() {
        assert_eq!(
            parse_hash_lines("abc123\n\n def456 \n"),
            vec!["abc123", "def456"]
        );
        assert_eq!(literal_git_regex(".ant[foo]*"), r"\.ant\[foo\]\*");
    }

    #[test]
    fn parses_name_status_output() {
        let changes = parse_file_changes("M\tsrc/app.rs\nA\tREADME.md\nR100\told.rs\tnew.rs\n");

        assert_eq!(changes.len(), 3);
        assert_eq!(changes[0].status, "M");
        assert_eq!(changes[0].path, "src/app.rs");
        assert_eq!(changes[0].diff_path, "src/app.rs");
        assert_eq!(changes[2].status, "R100");
        assert_eq!(changes[2].path, "old.rs -> new.rs");
        assert_eq!(changes[2].diff_path, "new.rs");
    }

    #[test]
    fn parses_short_status_into_staged_and_unstaged() {
        let lines = vec![
            "A  src/main.rs".to_owned(),
            " M src/app.rs".to_owned(),
            "?? README.md".to_owned(),
            "R  old.rs -> new.rs".to_owned(),
        ];

        let (staged, unstaged) = parse_status_entries(&lines);

        assert_eq!(
            staged
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["src/main.rs", "new.rs"]
        );
        assert_eq!(
            unstaged
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["src/app.rs", "README.md"]
        );
    }

    #[test]
    fn open_repository_requests_all_untracked_files() {
        let source = include_str!("git.rs");
        assert!(source.contains("\"--untracked-files=all\""));
    }

    #[test]
    fn discard_all_changes_resets_tracked_and_cleans_untracked_files() {
        let source = include_str!("git.rs");
        let helper_start = source.find("pub fn discard_all_changes(").unwrap();
        let helper_end = source[helper_start..]
            .find("pub fn checkout_branch(")
            .unwrap();
        let helper_source = &source[helper_start..helper_start + helper_end];

        assert!(helper_source.contains("&[\"reset\", \"--hard\"]"));
        assert!(helper_source.contains("&[\"clean\", \"-fd\"]"));
        assert!(
            helper_source.find("reset").unwrap() < helper_source.find("clean").unwrap(),
            "tracked changes should reset before untracked files are cleaned"
        );
    }

    #[test]
    fn stash_push_args_reflect_dialog_options() {
        assert_eq!(
            stash_push_args("WIP", StashOptions::default()),
            vec!["stash", "push", "-m", "WIP"]
        );
        assert_eq!(
            stash_push_args(
                "",
                StashOptions {
                    staged_files: true,
                    keep_staged: true,
                    include_untracked: true,
                    include_ignored: true,
                },
            ),
            vec!["stash", "push", "--staged", "--keep-index", "--all"]
        );
    }

    #[test]
    fn merge_branch_conflicts_return_ok_so_ui_can_reload_conflict_state() {
        let root =
            std::env::temp_dir().join(format!("git-agent-merge-conflict-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        git_command()
            .arg("-C")
            .arg(&root)
            .args(["init"])
            .output()
            .unwrap();
        git_command()
            .arg("-C")
            .arg(&root)
            .args(["config", "user.name", "Merge Tester"])
            .output()
            .unwrap();
        git_command()
            .arg("-C")
            .arg(&root)
            .args(["config", "user.email", "merge-tester@example.com"])
            .output()
            .unwrap();
        git_command()
            .arg("-C")
            .arg(&root)
            .args(["checkout", "-b", "main"])
            .output()
            .unwrap();
        fs::write(root.join("story.txt"), "base\n").unwrap();
        git_command()
            .arg("-C")
            .arg(&root)
            .args(["add", "story.txt"])
            .output()
            .unwrap();
        git_command()
            .arg("-C")
            .arg(&root)
            .args(["commit", "-m", "base"])
            .output()
            .unwrap();
        git_command()
            .arg("-C")
            .arg(&root)
            .args(["checkout", "-b", "feature-conflict"])
            .output()
            .unwrap();
        fs::write(root.join("story.txt"), "feature\n").unwrap();
        git_command()
            .arg("-C")
            .arg(&root)
            .args(["commit", "-am", "feature"])
            .output()
            .unwrap();
        git_command()
            .arg("-C")
            .arg(&root)
            .args(["checkout", "main"])
            .output()
            .unwrap();
        fs::write(root.join("story.txt"), "main\n").unwrap();
        git_command()
            .arg("-C")
            .arg(&root)
            .args(["commit", "-am", "main"])
            .output()
            .unwrap();

        let merge_result = merge_branch(&root, "feature-conflict");
        let snapshot = open_repository(&root).unwrap();
        let staged_conflict = snapshot.staged.iter().any(WorktreeFile::is_conflicted);
        let unstaged_conflict = snapshot.unstaged.iter().any(WorktreeFile::is_conflicted);
        let merge_message = snapshot.merge_message.clone().unwrap_or_default();
        let repeated_merge_result = merge_branch(&root, "feature-conflict");

        fs::remove_dir_all(&root).unwrap();
        assert!(merge_result.is_ok(), "{merge_result:?}");
        assert!(repeated_merge_result.is_ok(), "{repeated_merge_result:?}");
        assert!(staged_conflict);
        assert!(unstaged_conflict);
        assert!(merge_message.starts_with("Merge branch 'feature-conflict' into main"));
        assert!(merge_message.contains("# Conflicts:"));
        assert!(merge_message.contains("story.txt"));
    }

    #[test]
    fn worktree_diff_for_untracked_file_contains_full_file_body() {
        let root =
            std::env::temp_dir().join(format!("git-agent-untracked-diff-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        git_command()
            .arg("-C")
            .arg(&root)
            .arg("init")
            .output()
            .unwrap();
        fs::write(
            root.join("src/new.rs"),
            "fn main() {\n    println!(\"hi\");\n}\n",
        )
        .unwrap();

        let diff = load_worktree_diff(&root, "src/new.rs", false, true).unwrap();

        assert!(diff.text.contains("--- /dev/null"));
        assert!(diff.text.contains("+++ b/src/new.rs"));
        assert!(diff.text.contains("+fn main() {"));
        assert!(diff.text.contains("+    println!(\"hi\");"));
        assert!(diff.text.contains("+}"));

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn add_to_gitignore_appends_unique_normalized_patterns() {
        let root =
            std::env::temp_dir().join(format!("git-agent-ignore-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        add_to_gitignore(&root, r".\src\").unwrap();
        add_to_gitignore(&root, "src/").unwrap();
        add_to_gitignore(&root, r"src\app.rs").unwrap();

        let content = fs::read_to_string(root.join(".gitignore")).unwrap();
        assert_eq!(content, "src/\nsrc/app.rs\n");

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn marks_unmerged_status_entries_as_conflicted() {
        let lines = vec![
            "UU story.txt".to_owned(),
            "AA both-added.txt".to_owned(),
            "DU deleted-by-us.txt".to_owned(),
        ];

        let (staged, unstaged) = parse_status_entries(&lines);

        assert_eq!(staged.len(), 3);
        assert_eq!(unstaged.len(), 3);
        assert!(staged.iter().all(WorktreeFile::is_conflicted));
        assert!(unstaged.iter().all(WorktreeFile::is_conflicted));
    }

    #[test]
    fn parses_remote_verbose_output() {
        let output =
            "origin\thttps://example/repo.git (fetch)\norigin\thttps://example/repo.git (push)\n";
        let root = Path::new(".");
        let _ = root;
        let mut remotes = Vec::<Remote>::new();
        for line in output.lines() {
            let mut parts = line.split_whitespace();
            let name = parts.next().unwrap();
            let url = parts.next().unwrap();
            let kind = parts.next().unwrap();
            let remote =
                if let Some(existing) = remotes.iter_mut().find(|remote| remote.name == name) {
                    existing
                } else {
                    remotes.push(Remote {
                        name: name.to_owned(),
                        fetch_url: String::new(),
                        push_url: String::new(),
                    });
                    remotes.last_mut().unwrap()
                };
            if kind == "(fetch)" {
                remote.fetch_url = url.to_owned();
            } else {
                remote.push_url = url.to_owned();
            }
        }

        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0].name, "origin");
        assert_eq!(remotes[0].fetch_url, "https://example/repo.git");
        assert_eq!(remotes[0].push_url, "https://example/repo.git");
    }

    #[test]
    fn parses_branch_list_output_with_tabs() {
        let output = " \tfeature-batch\trefs/heads/feature-batch\torigin/feature-batch\n \tfeature-clean\trefs/heads/feature-clean\t\n*\t(HEAD detached at c02dcf4)\t(HEAD detached at c02dcf4)\t\n*\tmain\trefs/heads/main\torigin/main\n";

        let branches = parse_branches(output);

        assert_eq!(branches.len(), 3);
        assert_eq!(branches[0].name, "feature-batch");
        assert!(!branches[0].remote);
        assert!(!branches[0].current);
        assert_eq!(
            branches[0]
                .upstream
                .as_ref()
                .map(|upstream| upstream.name.as_str()),
            Some("origin/feature-batch")
        );
        assert_eq!(branches[2].name, "main");
        assert!(branches[2].current);
        assert_eq!(
            branches[2]
                .upstream
                .as_ref()
                .map(|upstream| upstream.name.as_str()),
            Some("origin/main")
        );
    }

    #[test]
    fn parses_remote_branch_refs_from_branch_list_output() {
        let output = " \tlocal-test/main\trefs/remotes/local-test/main\n \torigin/feature\trefs/remotes/origin/feature\n";

        let branches = parse_branches(output);

        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].name, "local-test/main");
        assert!(branches[0].remote);
        assert!(!branches[0].current);
        assert_eq!(branches[1].name, "origin/feature");
        assert!(branches[1].remote);
    }

    #[test]
    fn parses_stash_list_output() {
        let stashes = parse_stashes("stash@{0}\x1f2 hours ago\x1fWIP on main: abc init\n");

        assert_eq!(stashes.len(), 1);
        assert_eq!(stashes[0].selector, "stash@{0}");
        assert_eq!(stashes[0].relative_time, "2 hours ago");
        assert_eq!(stashes[0].message, "WIP on main: abc init");
    }

    #[test]
    fn parses_tag_list_output() {
        let tags = parse_tags("v1.0.0\x1fabcd1234\x1frelease commit\n");

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "v1.0.0");
        assert_eq!(tags[0].target, "abcd1234");
        assert_eq!(tags[0].subject, "release commit");
    }

    #[test]
    fn parses_tag_list_output_when_separator_is_literal() {
        let tags = parse_tags("v3.6.0%x1f20c49fd0%x1fMerge branch\n");

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "v3.6.0");
        assert_eq!(tags[0].target, "20c49fd0");
        assert_eq!(tags[0].subject, "Merge branch");
    }

    #[test]
    fn push_tag_args_target_selected_remote_and_local_tag_ref() {
        let args = push_tag_args("origin", "v1.0.0");

        assert_eq!(args, vec!["push", "origin", "refs/tags/v1.0.0"]);
    }

    #[test]
    fn branch_operation_args_target_selected_branch_and_remote() {
        assert_eq!(
            rename_branch_args("feature-old", "feature-new"),
            vec!["branch", "-m", "feature-old", "feature-new"]
        );
        assert_eq!(
            fetch_remote_branch_args("origin/feature-batch").unwrap(),
            vec!["fetch", "origin", "feature-batch"]
        );
        assert_eq!(
            push_branch_to_remote_args("origin", "feature-batch"),
            vec!["push", "-u", "origin", "feature-batch"]
        );
        assert_eq!(
            set_branch_upstream_args("feature-batch", "origin/feature-batch"),
            vec![
                "branch",
                "--set-upstream-to=origin/feature-batch",
                "feature-batch"
            ]
        );
        assert_eq!(
            unset_branch_upstream_args("feature-batch"),
            vec!["branch", "--unset-upstream", "feature-batch"]
        );
        assert!(fetch_remote_branch_args("origin").is_err());
    }

    #[test]
    fn fetch_args_respect_dialog_options() {
        assert_eq!(
            fetch_args(FetchOptions::default()),
            vec!["fetch", "--all", "--prune"]
        );
        assert_eq!(
            fetch_args(FetchOptions {
                all_remotes: false,
                prune_tracking: false,
                fetch_tags: true,
                force_tags: false,
            }),
            vec!["fetch", "--tags"]
        );
        assert_eq!(
            fetch_args(FetchOptions {
                all_remotes: true,
                prune_tracking: true,
                fetch_tags: true,
                force_tags: true,
            }),
            vec!["fetch", "--all", "--prune", "--tags", "--force"]
        );
    }

    #[test]
    fn pull_args_target_selected_remote_branch_and_options() {
        assert_eq!(
            pull_args("origin", "main", PullOptions::default()),
            vec!["pull", "origin", "main"]
        );
        assert_eq!(
            pull_args(
                "origin",
                "feature/ui",
                PullOptions {
                    commit_merge: false,
                    include_tags: true,
                    force_merge_commit: true,
                    rebase: false,
                }
            ),
            vec![
                "pull",
                "--no-commit",
                "--tags",
                "--no-ff",
                "origin",
                "feature/ui"
            ]
        );
        assert_eq!(
            pull_args(
                "origin",
                "main",
                PullOptions {
                    rebase: true,
                    force_merge_commit: true,
                    commit_merge: false,
                    include_tags: false,
                }
            ),
            vec!["pull", "--rebase", "origin", "main"]
        );
    }

    #[test]
    fn merge_commit_args_reflect_dialog_options() {
        assert_eq!(
            merge_commit_args("abc123", MergeOptions::default()),
            vec!["merge", "abc123"]
        );
        assert_eq!(
            merge_commit_args(
                "abc123",
                MergeOptions {
                    commit_merge: false,
                    include_messages: true,
                    force_merge_commit: true,
                    rebase: false,
                    detect_renames: true,
                    rename_threshold: 90,
                },
            ),
            vec![
                "merge",
                "--no-commit",
                "--log",
                "--no-ff",
                "-X",
                "find-renames=90%",
                "abc123"
            ]
        );
        assert_eq!(
            merge_commit_args(
                "abc123",
                MergeOptions {
                    rebase: true,
                    ..MergeOptions::default()
                },
            ),
            vec!["rebase", "abc123"]
        );
    }

    #[test]
    fn archive_args_reflect_dialog_inputs() {
        assert_eq!(
            archive_args(std::path::Path::new("D:/repo/archive.zip"), "", "HEAD",),
            vec![
                "archive",
                "--format=zip",
                "--output",
                "D:/repo/archive.zip",
                "HEAD"
            ]
        );
        assert_eq!(
            archive_args(
                std::path::Path::new("D:/repo/archive.tar"),
                "release",
                "abc123",
            ),
            vec![
                "archive",
                "--format=tar",
                "--output",
                "D:/repo/archive.tar",
                "--prefix=release/",
                "abc123"
            ]
        );
        assert_eq!(
            archive_args(
                std::path::Path::new("D:/repo/archive.tar.gz"),
                "release/",
                "feature",
            ),
            vec![
                "archive",
                "--format=tar",
                "--output",
                "D:/repo/archive.tar.gz",
                "--prefix=release/",
                "feature"
            ]
        );
    }

    #[test]
    fn interactive_rebase_todo_uses_ordered_actions() {
        let items = vec![
            InteractiveRebaseTodoItem {
                action: InteractiveRebaseAction::Pick,
                hash: "aaa111".to_owned(),
                subject: "oldest".to_owned(),
            },
            InteractiveRebaseTodoItem {
                action: InteractiveRebaseAction::Squash,
                hash: "bbb222".to_owned(),
                subject: "middle".to_owned(),
            },
            InteractiveRebaseTodoItem {
                action: InteractiveRebaseAction::Drop,
                hash: "ccc333".to_owned(),
                subject: "newest".to_owned(),
            },
        ];

        assert_eq!(
            interactive_rebase_todo(&items),
            "pick aaa111 oldest\nsquash bbb222 middle\ndrop ccc333 newest\n"
        );
    }

    #[test]
    fn rebase_in_progress_detects_rebase_state_directories() -> Result<()> {
        let root = interactive_rebase_temp_dir();
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root)?;
        git_output(&root, &["init"])?;

        assert!(!rebase_in_progress(&root));
        assert!(!repository_rebase_in_progress(&root));

        let rebase_merge = git_path(&root, "rebase-merge").unwrap();
        fs::create_dir_all(&rebase_merge)?;
        assert!(rebase_in_progress(&root));
        assert!(repository_rebase_in_progress(&root));
        fs::remove_dir_all(&rebase_merge)?;
        assert!(!repository_rebase_in_progress(&root));

        let rebase_apply = git_path(&root, "rebase-apply").unwrap();
        fs::create_dir_all(&rebase_apply)?;
        assert!(rebase_in_progress(&root));
        assert!(repository_rebase_in_progress(&root));
        let error = interactive_rebase(
            &root,
            "HEAD~1",
            &[InteractiveRebaseTodoItem {
                action: InteractiveRebaseAction::Drop,
                hash: "abc123".to_owned(),
                subject: "drop me".to_owned(),
            }],
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("already in progress"));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn rebase_control_actions_resume_skip_or_abort_existing_rebase() -> Result<()> {
        let root =
            std::env::temp_dir().join(format!("git-agent-rebase-control-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root)?;
        git_output(&root, &["init"])?;
        git_output(&root, &["config", "user.email", "tester@example.com"])?;
        git_output(&root, &["config", "user.name", "Git Agent Test"])?;

        fs::write(root.join("story.txt"), "base\n")?;
        git_output(&root, &["add", "."])?;
        git_output(&root, &["commit", "-m", "base"])?;
        git_output(&root, &["checkout", "-b", "feature"])?;
        fs::write(root.join("story.txt"), "feature\n")?;
        git_output(&root, &["add", "."])?;
        git_output(&root, &["commit", "-m", "feature"])?;
        git_output(&root, &["checkout", "master"])?;
        fs::write(root.join("story.txt"), "master\n")?;
        git_output(&root, &["add", "."])?;
        git_output(&root, &["commit", "-m", "master"])?;

        let conflict = git_output(&root, &["rebase", "feature"]).unwrap_err();
        assert!(conflict.to_string().contains("could not apply"));
        assert!(repository_rebase_in_progress(&root));

        let continue_error = rebase_continue(&root).unwrap_err().to_string();
        assert!(continue_error.contains("needs merge") || continue_error.contains("unmerged"));

        rebase_abort(&root)?;
        assert!(!repository_rebase_in_progress(&root));

        git_output(&root, &["rebase", "feature"]).unwrap_err();
        assert!(repository_rebase_in_progress(&root));
        rebase_skip(&root)?;
        assert!(!repository_rebase_in_progress(&root));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn interactive_rebase_executes_generated_todo() -> Result<()> {
        let root = interactive_rebase_temp_dir();
        fs::create_dir_all(&root)?;
        git_output(&root, &["init"])?;
        git_output(&root, &["config", "user.email", "tester@example.com"])?;
        git_output(&root, &["config", "user.name", "Git Agent Test"])?;

        fs::write(root.join("file.txt"), "base\n")?;
        git_output(&root, &["add", "."])?;
        git_output(&root, &["commit", "-m", "base"])?;
        let base_hash = git_output(&root, &["rev-parse", "HEAD"])?.trim().to_owned();

        fs::write(root.join("file.txt"), "one\n")?;
        git_output(&root, &["add", "."])?;
        git_output(&root, &["commit", "-m", "one"])?;
        let first_hash = git_output(&root, &["rev-parse", "HEAD"])?.trim().to_owned();

        fs::write(root.join("file.txt"), "two\n")?;
        git_output(&root, &["add", "."])?;
        git_output(&root, &["commit", "-m", "two"])?;
        let second_hash = git_output(&root, &["rev-parse", "HEAD"])?.trim().to_owned();

        interactive_rebase(
            &root,
            &base_hash,
            &[
                InteractiveRebaseTodoItem {
                    action: InteractiveRebaseAction::Pick,
                    hash: first_hash,
                    subject: "one".to_owned(),
                },
                InteractiveRebaseTodoItem {
                    action: InteractiveRebaseAction::Drop,
                    hash: second_hash,
                    subject: "two".to_owned(),
                },
            ],
        )?;

        let log = git_output(&root, &["log", "--format=%s"])?;
        let _ = fs::remove_dir_all(&root);
        assert!(log.contains("one"));
        assert!(log.contains("base"));
        assert!(!log.contains("two"));
        Ok(())
    }

    #[test]
    fn submodule_and_subtree_args_match_sourcetree_dialogs() {
        assert_eq!(
            add_submodule_args(&SubmoduleAddOptions {
                source: "https://example.com/lib.git".to_owned(),
                local_path: "vendor/lib".to_owned(),
                source_branch: "main".to_owned(),
                recursive: true,
            }),
            vec![
                "submodule",
                "add",
                "-b",
                "main",
                "https://example.com/lib.git",
                "vendor/lib"
            ]
        );
        assert_eq!(
            submodule_recursive_update_args("vendor/lib"),
            vec![
                "submodule",
                "update",
                "--init",
                "--recursive",
                "--",
                "vendor/lib"
            ]
        );
        assert_eq!(
            add_subtree_args(&SubtreeAddOptions {
                source: "https://example.com/lib.git".to_owned(),
                local_path: "third_party/lib".to_owned(),
                ref_name: "main".to_owned(),
                squash: true,
            }),
            vec![
                "subtree",
                "add",
                "--prefix",
                "third_party/lib",
                "https://example.com/lib.git",
                "main",
                "--squash"
            ]
        );
    }

    #[test]
    fn lfs_args_and_attribute_parser_cover_tracking_flow() {
        assert_eq!(lfs_install_args(), vec!["lfs", "install", "--local"]);
        assert_eq!(lfs_track_args("*.psd"), vec!["lfs", "track", "*.psd"]);
        assert_eq!(lfs_untrack_args("*.zip"), vec!["lfs", "untrack", "*.zip"]);
        assert_eq!(lfs_simple_args("pull"), vec!["lfs", "pull"]);
        assert_eq!(lfs_simple_args("fetch"), vec!["lfs", "fetch"]);
        assert_eq!(lfs_simple_args("checkout"), vec!["lfs", "checkout"]);
        assert_eq!(lfs_simple_args("prune"), vec!["lfs", "prune"]);
        assert_eq!(
            parse_lfs_gitattributes_patterns(
                "*.psd filter=lfs diff=lfs merge=lfs -text\nassets/** filter=lfs diff=lfs merge=lfs -text\n*.txt text\n"
            ),
            vec!["*.psd", "assets/**"]
        );
        assert_eq!(
            normalized_lfs_patterns(&[
                " *.psd ".to_owned(),
                "*.psd".to_owned(),
                "*.mp4".to_owned()
            ]),
            vec!["*.psd", "*.mp4"]
        );
    }

    #[test]
    fn push_args_target_selected_branches_tags_force_and_tracking() {
        assert_eq!(
            push_branch_args(
                "origin",
                &PushBranchSpec {
                    local_branch: "main".to_owned(),
                    remote_branch: "main".to_owned(),
                    track: true,
                },
                false,
            ),
            vec!["push", "-u", "origin", "main:refs/heads/main"]
        );
        assert_eq!(
            push_branch_args(
                "upstream",
                &PushBranchSpec {
                    local_branch: "feature/ui".to_owned(),
                    remote_branch: "review/ui".to_owned(),
                    track: false,
                },
                true,
            ),
            vec![
                "push",
                "--force-with-lease",
                "upstream",
                "feature/ui:refs/heads/review/ui"
            ]
        );
        assert_eq!(push_tags_args("origin"), vec!["push", "origin", "--tags"]);
    }

    #[test]
    fn loads_repository_settings_config_for_ui() {
        let root =
            std::env::temp_dir().join(format!("git-agent-repo-settings-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        git_command()
            .arg("-C")
            .arg(&root)
            .arg("init")
            .output()
            .unwrap();
        git_command()
            .arg("-C")
            .arg(&root)
            .args(["config", "--local", "user.name", "Ado Wang"])
            .output()
            .unwrap();
        git_command()
            .arg("-C")
            .arg(&root)
            .args(["config", "--local", "user.email", "adoin.wang@qq.com"])
            .output()
            .unwrap();

        let config = load_repository_config(&root);

        assert_eq!(config.gitignore_path, root.join(".gitignore"));
        assert_eq!(config.user_name, "Ado Wang");
        assert_eq!(config.user_email, "adoin.wang@qq.com");
        assert!(!config.uses_global_user);
        assert!(config.config_path.ends_with("config"));

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn batch_cherry_pick_args_keep_requested_order() {
        let hashes = vec!["old123".to_owned(), "new456".to_owned()];

        let args = cherry_pick_args(&hashes);

        assert_eq!(args, vec!["cherry-pick", "old123", "new456"]);
    }

    #[test]
    fn commit_args_include_selected_options() {
        assert_eq!(
            commit_args(
                "update",
                CommitOptions {
                    amend: true,
                    no_verify: true,
                    gpg_sign: true,
                },
            ),
            vec!["commit", "--amend", "--no-verify", "-S", "-m", "update"]
        );
        assert_eq!(
            commit_args("plain", CommitOptions::default()),
            vec!["commit", "-m", "plain"]
        );
    }
}
