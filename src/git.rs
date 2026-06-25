use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use anyhow::{Context, Result, anyhow};

const HISTORY_COMMIT_LIMIT: usize = 50_000;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Debug, Default)]
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

#[derive(Clone, Debug, Default)]
pub struct FileChange {
    pub status: String,
    pub path: String,
    pub diff_path: String,
}

#[derive(Clone, Debug, Default)]
pub struct Branch {
    pub name: String,
    pub current: bool,
    pub remote: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Remote {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct UpstreamStatus {
    pub name: String,
    pub ahead: usize,
    pub behind: usize,
}

#[derive(Clone, Debug, Default)]
pub struct StashEntry {
    pub selector: String,
    pub relative_time: String,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct Tag {
    pub name: String,
    pub target: String,
    pub subject: String,
}

#[derive(Clone, Debug, Default)]
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

#[derive(Clone, Debug, Default)]
pub struct RepositorySnapshot {
    pub root: PathBuf,
    pub branch: String,
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

#[derive(Clone, Debug, Default)]
pub struct CommitDetails {
    pub hash: String,
    pub files: Vec<FileChange>,
}

#[derive(Clone, Debug, Default)]
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
        });
    }
    let remotes = load_remotes(&root).unwrap_or_default();
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

pub fn delete_branch(root: impl AsRef<Path>, name: &str, force: bool) -> Result<()> {
    if force {
        git_output(root.as_ref(), &["branch", "-D", name]).map(|_| ())
    } else {
        git_output(root.as_ref(), &["branch", "-d", name]).map(|_| ())
    }
}

pub fn delete_remote_branch(root: impl AsRef<Path>, remote_branch: &str) -> Result<()> {
    let (remote, branch) = remote_branch
        .split_once('/')
        .ok_or_else(|| anyhow!("remote branch must look like remote/name"))?;
    if remote.trim().is_empty() || branch.trim().is_empty() {
        return Err(anyhow!("remote branch must look like remote/name"));
    }
    git_output(root.as_ref(), &["push", remote, "--delete", branch]).map(|_| ())
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

pub fn fetch(root: impl AsRef<Path>) -> Result<()> {
    git_output(root.as_ref(), &["fetch", "--all", "--prune"]).map(|_| ())
}

pub fn pull(root: impl AsRef<Path>) -> Result<()> {
    git_output(root.as_ref(), &["pull", "--ff-only"]).map(|_| ())
}

pub fn push(root: impl AsRef<Path>) -> Result<()> {
    git_output(root.as_ref(), &["push"]).map(|_| ())
}

pub fn push_set_upstream(root: impl AsRef<Path>, remote: &str, branch: &str) -> Result<()> {
    git_output(root.as_ref(), &["push", "-u", remote, branch]).map(|_| ())
}

pub fn stash_push(root: impl AsRef<Path>, message: &str) -> Result<()> {
    if message.trim().is_empty() {
        git_output(root.as_ref(), &["stash", "push", "--include-untracked"]).map(|_| ())
    } else {
        git_output(
            root.as_ref(),
            &["stash", "push", "--include-untracked", "-m", message],
        )
        .map(|_| ())
    }
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
            "--format=%(HEAD)%09%(refname:short)%09%(refname)",
        ],
    )?;

    Ok(parse_branches(&output))
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
            (!name.is_empty()).then(|| Branch {
                remote: refname.starts_with("refs/remotes/"),
                current: head.trim() == "*",
                name,
            })
        })
        .collect()
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
        let output = " \tfeature-batch\trefs/heads/feature-batch\n \tfeature-clean\trefs/heads/feature-clean\n*\tmain\trefs/heads/main\n";

        let branches = parse_branches(output);

        assert_eq!(branches.len(), 3);
        assert_eq!(branches[0].name, "feature-batch");
        assert!(!branches[0].remote);
        assert!(!branches[0].current);
        assert_eq!(branches[2].name, "main");
        assert!(branches[2].current);
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
