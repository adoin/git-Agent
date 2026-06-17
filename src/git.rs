use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use anyhow::{Context, Result, anyhow};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Debug, Default)]
pub struct Commit {
    pub hash: String,
    pub short_hash: String,
    pub parents: Vec<String>,
    pub author: String,
    pub relative_time: String,
    pub subject: String,
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

    let status = git_output(&root, &["status", "--short"])
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
    let commits = load_commits(&root, 2_500)?;

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
    })
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

pub fn load_worktree_diff(root: impl AsRef<Path>, path: &str, staged: bool) -> Result<FileDiff> {
    let text = if staged {
        git_output(root.as_ref(), &["diff", "--cached", "--", path])?
    } else {
        git_output(root.as_ref(), &["diff", "--", path])?
    };
    let key = worktree_diff_key(path, staged);
    Ok(FileDiff { key, text })
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

pub fn commit(root: impl AsRef<Path>, message: &str) -> Result<()> {
    git_output(root.as_ref(), &["commit", "-m", message]).map(|_| ())
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
            "--format=%(HEAD)%x1f%(refname:short)%x1f%(refname)",
        ],
    )?;

    Ok(output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\x1f');
            let head = parts.next().unwrap_or_default();
            let name = parts.next()?.trim().to_owned();
            let refname = parts.next().unwrap_or_default();
            (!name.is_empty()).then(|| Branch {
                remote: refname.starts_with("refs/remotes/"),
                current: head == "*",
                name,
            })
        })
        .collect())
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
            "--format=%(refname:short)%x1f%(objectname:short)%x1f%(subject)",
        ],
    )?;
    Ok(parse_tags(&output))
}

fn parse_tags(output: &str) -> Vec<Tag> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\x1f');
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

fn load_commits(root: &Path, limit: usize) -> Result<Vec<Commit>> {
    if !has_head(root) {
        return Ok(Vec::new());
    }

    let max_count = format!("--max-count={limit}");
    let output = git_output(
        root,
        &[
            "log",
            "--date-order",
            "--topo-order",
            &max_count,
            "--format=%H%x1f%P%x1f%an%x1f%ar%x1f%s",
        ],
    )?;

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
            let relative_time = parts.next().unwrap_or_default().to_owned();
            let subject = parts.next().unwrap_or_default().to_owned();
            let short_hash = hash.chars().take(8).collect();

            Some(Commit {
                hash,
                short_hash,
                parents,
                author,
                relative_time,
                subject,
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
    let mut command = Command::new("git");
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
