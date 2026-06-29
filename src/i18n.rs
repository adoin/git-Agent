#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    English,
    Chinese,
}

#[allow(dead_code)]
impl Language {
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "EN",
            Self::Chinese => "中文",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::English => Self::Chinese,
            Self::Chinese => Self::English,
        }
    }
}

pub fn t(language: Language, key: &'static str) -> &'static str {
    if language == Language::Chinese {
        if let Some(value) = ZH_SOURCE
            .iter()
            .find_map(|(candidate, value)| (*candidate == key).then_some(*value))
        {
            return value;
        }
    }

    let entries = match language {
        Language::English => EN,
        Language::Chinese => ZH,
    };

    entries
        .iter()
        .find_map(|(candidate, value)| (*candidate == key).then_some(*value))
        .unwrap_or(key)
}

const ZH_SOURCE: &[(&str, &str)] = &[
    (
        "status.hash_copied",
        "\u{5df2}\u{590d}\u{5236}\u{5b8c}\u{6574} hash",
    ),
    ("diff.blocks", "\u{5dee}\u{5f02}\u{5757}"),
    ("diff.full_file", "\u{5b8c}\u{6574}\u{6587}\u{4ef6}"),
    ("menu.copy", "\u{590d}\u{5236}"),
    ("repo.source.new_tab", "New tab"),
    ("repo.source.close_tab", "\u{5173}\u{95ed}\u{6807}\u{7b7e}"),
    ("repo.source.title", "\u{672c}\u{5730}\u{4ed3}\u{5e93}"),
    ("repo.source.local", "\u{672c}\u{5730}"),
    ("repo.source.remote", "\u{8fdc}\u{7aef}"),
    ("repo.source.clone", "\u{514b}\u{9686}"),
    ("repo.source.add", "\u{6dfb}\u{52a0}"),
    ("repo.source.create", "\u{521b}\u{5efa}"),
    ("repo.source.search", "\u{641c}\u{7d22}"),
    (
        "repo.source.local_repositories",
        "\u{672c}\u{5730}\u{4ed3}\u{5e93}",
    ),
    (
        "repo.source.empty",
        "\u{672a}\u{627e}\u{5230}\u{672c}\u{5730}\u{4ed3}\u{5e93}\u{3002}",
    ),
    ("repo.source.clone_url", "\u{6e90} URL"),
    (
        "repo.source.destination",
        "\u{76ee}\u{6807}\u{8def}\u{5f84}",
    ),
    ("repo.source.browse", "\u{6d4f}\u{89c8}"),
    ("repo.source.pending", "\u{7b49}\u{5f85}\u{6821}\u{9a8c}"),
    ("repo.source.checking", "\u{6821}\u{9a8c}\u{4e2d}"),
    ("repo.source.valid", "\u{6709}\u{6548}"),
    ("repo.source.invalid", "\u{65e0}\u{6548}\u{8fde}\u{63a5}"),
    (
        "commit.cherry_pick_batch",
        "\u{6279}\u{91cf}\u{62e3}\u{9009}",
    ),
    ("commit.cherry_pick_confirm", "\u{786e}\u{5b9a}"),
    (
        "commit.cherry_pick_selected",
        "\u{4e2a}\u{5df2}\u{9009}\u{63d0}\u{4ea4}",
    ),
    (
        "commit.confirm_cherry_pick_batch",
        "\u{62e3}\u{9009}\u{9009}\u{4e2d}\u{7684}\u{63d0}\u{4ea4}\u{ff1f}",
    ),
    ("repo.git_flow", "Git\u{5de5}\u{4f5c}\u{6d41}"),
    ("repo.remote", "\u{8fdc}\u{7aef}"),
    (
        "repo.command_mode",
        "\u{547d}\u{4ee4}\u{884c}\u{6a21}\u{5f0f}",
    ),
    (
        "repo.resource_manager",
        "\u{8d44}\u{6e90}\u{7ba1}\u{7406}\u{5668}",
    ),
    (
        "repo.git_flow.opened",
        "\u{5df2}\u{6253}\u{5f00} Git \u{5de5}\u{4f5c}\u{6d41}",
    ),
    (
        "repo.command_mode.failed",
        "\u{6253}\u{5f00}\u{547d}\u{4ee4}\u{884c}\u{5931}\u{8d25}",
    ),
    (
        "repo.resource_manager.failed",
        "\u{6253}\u{5f00}\u{8d44}\u{6e90}\u{7ba1}\u{7406}\u{5668}\u{5931}\u{8d25}",
    ),
    (
        "repo.remote.missing",
        "\u{672a}\u{914d}\u{7f6e}\u{8fdc}\u{7aef} URL",
    ),
    (
        "repo.remote.failed",
        "\u{6253}\u{5f00}\u{8fdc}\u{7aef} URL \u{5931}\u{8d25}",
    ),
    (
        "repo.source.clone_missing",
        "\u{8bf7}\u{8f93}\u{5165}\u{6e90} URL \u{548c}\u{76ee}\u{6807}\u{8def}\u{5f84}\u{3002}",
    ),
    (
        "repo.source.create_missing",
        "\u{521b}\u{5efa}\u{4ed3}\u{5e93}\u{524d}\u{8bf7}\u{9009}\u{62e9}\u{6587}\u{4ef6}\u{5939}\u{3002}",
    ),
    ("branch.title", "\u{5206}\u{652f}"),
    ("branch.current_badge", "\u{5f53}\u{524d}"),
    ("branch.remote", "\u{8fdc}\u{7aef}\u{5206}\u{652f}"),
    (
        "branch.delete_remote",
        "\u{5220}\u{9664}\u{8fdc}\u{7aef}\u{5206}\u{652f}",
    ),
    (
        "branch.sync_remote",
        "\u{540c}\u{6b65}\u{8fdc}\u{7aef}\u{5206}\u{652f}",
    ),
    (
        "branch.local_alias",
        "\u{672c}\u{5730}\u{5206}\u{652f}\u{522b}\u{540d}",
    ),
    (
        "branch.confirm_delete_remote",
        "\u{5220}\u{9664}\u{8fdc}\u{7aef}\u{5206}\u{652f}\u{ff1f}",
    ),
    ("remote.title", "\u{8fdc}\u{7aef}\u{5206}\u{652f}"),
    (
        "remote.none",
        "\u{6ca1}\u{6709}\u{8fdc}\u{7aef}\u{4ed3}\u{5e93}",
    ),
    (
        "remote.no_branches",
        "\u{672a}\u{83b7}\u{53d6}\u{5230}\u{8fdc}\u{7aef}\u{5206}\u{652f}",
    ),
    ("common.remote", "\u{8fdc}\u{7aef}"),
    (
        "branch.checkout_remote",
        "\u{68c0}\u{51fa}\u{8fdc}\u{7aef}\u{5206}\u{652f}",
    ),
    (
        "menu.open_remote",
        "\u{5728}\u{8fdc}\u{7aef}\u{6253}\u{5f00}\u{63d0}\u{4ea4}",
    ),
    (
        "repo.settings.remote_paths",
        "\u{8fdc}\u{7aef}\u{4ed3}\u{5e93}\u{8def}\u{5f84}",
    ),
    (
        "repo.settings.remote_details",
        "\u{8fdc}\u{7aef}\u{7ec6}\u{8282}",
    ),
    ("repo.settings.name", "\u{540d}\u{79f0}"),
    ("repo.settings.path", "\u{8def}\u{5f84}"),
    (
        "repo.settings.remote_name",
        "\u{8fdc}\u{7aef}\u{540d}\u{79f0}",
    ),
    (
        "repo.settings.default_remote",
        "\u{9ed8}\u{8ba4}\u{8fdc}\u{7aef}",
    ),
    ("repo.settings.url_path", "URL / \u{8def}\u{5f84}"),
    (
        "repo.settings.remote_account",
        "\u{8fdc}\u{7aef}\u{8d26}\u{6237}",
    ),
    (
        "settings.remote_accounts",
        "\u{8fdc}\u{7aef}\u{8d26}\u{6237}",
    ),
    (
        "settings.remote_account_name",
        "\u{8d26}\u{6237}\u{540d}\u{79f0}",
    ),
    ("settings.remote_account_host", "\u{4e3b}\u{673a}"),
    (
        "repo.settings.add_remote",
        "\u{6dfb}\u{52a0}\u{8fdc}\u{7aef}",
    ),
    (
        "repo.settings.edit_remote",
        "\u{7f16}\u{8f91}\u{8fdc}\u{7aef}",
    ),
    (
        "repo.settings.account_validation_failed",
        "\u{8d26}\u{6237}\u{914d}\u{7f6e}\u{6821}\u{9a8c}\u{5931}\u{8d25}",
    ),
    (
        "repo.settings.remote_validation_failed",
        "\u{8fdc}\u{7aef}\u{6821}\u{9a8c}\u{5931}\u{8d25}",
    ),
    ("repo.settings.generic_account", "Generic Account"),
    ("repo.settings.generic_host", "Generic Host"),
    (
        "repo.settings.legacy_account_settings",
        "\u{65e7}\u{7248}\u{8d26}\u{6237}\u{8bbe}\u{7f6e}",
    ),
    (
        "repo.settings.host_type",
        "\u{6258}\u{7ba1}\u{7c7b}\u{578b}",
    ),
    ("repo.settings.unknown", "\u{672a}\u{77e5}"),
    ("repo.settings.host_url", "\u{6258}\u{7ba1}\u{6839} URL"),
    ("repo.settings.username", "\u{7528}\u{6237}\u{540d}"),
    (
        "repo.settings.remote_account_hint",
        "\u{6269}\u{5c55}\u{96c6}\u{6210}\u{7528}\u{4e8e}\u{66f4}\u{6df1}\u{5c42}\u{6b21}\u{5730}\u{4e0e}\u{6258}\u{7ba1}\u{670d}\u{52a1}\u{8fdb}\u{884c}\u{6574}\u{5408}\u{ff0c}\u{5305}\u{62ec}\u{4ece}\u{7f51}\u{7ad9}\u{94fe}\u{63a5}\u{5b9a}\u{4f4d}\u{5df2}\u{6709}\u{514b}\u{9686}\u{548c}\u{521b}\u{5efa}\u{62c9}\u{53d6}\u{8bf7}\u{6c42}\u{3002}",
    ),
    (
        "repo.settings.ignore_list",
        "\u{4ed3}\u{5e93}\u{6307}\u{5b9a}\u{5ffd}\u{7565}\u{5217}\u{8868}",
    ),
    ("repo.settings.user", "\u{7528}\u{6237}\u{4fe1}\u{606f}"),
    (
        "repo.settings.use_global_user",
        "\u{4f7f}\u{7528}\u{5168}\u{5c40}\u{7528}\u{6237}\u{914d}\u{7f6e}",
    ),
    ("repo.settings.full_name", "\u{5168}\u{540d}"),
    (
        "repo.settings.email",
        "\u{7535}\u{5b50}\u{90ae}\u{4ef6}\u{5730}\u{5740}",
    ),
    (
        "repo.settings.commit_links",
        "\u{63d0}\u{4ea4}\u{6587}\u{672c}\u{94fe}\u{63a5}",
    ),
    ("repo.settings.options", "\u{6742}\u{9879}"),
    (
        "repo.settings.auto_refresh",
        "\u{81ea}\u{52a8}\u{5237}\u{65b0}\u{ff08}\u{5173}\u{95ed}\u{540e}\u{4f60}\u{5fc5}\u{987b}\u{624b}\u{52a8}\u{5237}\u{65b0}\u{6b64}\u{4ed3}\u{5e93}\u{ff09}",
    ),
    (
        "repo.settings.background_remote_refresh",
        "\u{5728}\u{540e}\u{53f0}\u{5237}\u{65b0}\u{8fdc}\u{7aef}\u{72b6}\u{6001}\u{ff08}\u{5728}\u{5168}\u{5c40}\u{8bbe}\u{7f6e}\u{91cc}\u{5f00}\u{542f}\u{65f6}\u{ff09}",
    ),
    (
        "repo.settings.edit_config_file",
        "\u{7f16}\u{8f91}\u{914d}\u{7f6e}\u{6587}\u{4ef6}...",
    ),
    (
        "repo.settings.config_failed",
        "\u{6253}\u{5f00}\u{914d}\u{7f6e}\u{6587}\u{4ef6}\u{5931}\u{8d25}",
    ),
    ("repo.settings.add", "\u{6dfb}\u{52a0}"),
    ("repo.settings.edit", "\u{7f16}\u{8f91}"),
    ("repo.settings.remove", "\u{79fb}\u{9664}"),
];

const EN: &[(&str, &str)] = &[
    ("app.title", "Git Agent"),
    ("app.subtitle", "fast visual Git client"),
    ("action.open", "Open"),
    ("action.refresh", "Refresh"),
    ("action.fetch", "Fetch"),
    ("action.pull", "Pull"),
    ("action.push", "Push"),
    ("settings.title", "Settings"),
    ("options.title", "Options"),
    ("repo.settings", "Repository Settings"),
    ("repo.settings.title", "Repository Settings"),
    ("repo.settings.remote_paths", "Remote repository paths"),
    ("repo.settings.remote_details", "Remote Details"),
    ("repo.settings.name", "Name"),
    ("repo.settings.path", "Path"),
    ("repo.settings.remote_name", "Remote name"),
    ("repo.settings.default_remote", "Default remote"),
    ("repo.settings.url_path", "URL / Path"),
    ("repo.settings.remote_account", "Remote Account"),
    ("settings.remote_accounts", "Remote Accounts"),
    ("settings.remote_account_name", "Account name"),
    ("settings.remote_account_host", "Host"),
    ("repo.settings.add_remote", "Add Remote"),
    ("repo.settings.edit_remote", "Edit Remote"),
    (
        "repo.settings.account_validation_failed",
        "Account validation failed",
    ),
    (
        "repo.settings.remote_validation_failed",
        "Remote validation failed",
    ),
    ("repo.settings.generic_account", "Generic Account"),
    ("repo.settings.generic_host", "Generic Host"),
    (
        "repo.settings.legacy_account_settings",
        "Legacy Account Settings",
    ),
    ("repo.settings.host_type", "Host type"),
    ("repo.settings.unknown", "Unknown"),
    ("repo.settings.host_url", "Host root URL"),
    ("repo.settings.username", "Username"),
    (
        "repo.settings.remote_account_hint",
        "Extended integration is used for deeper hosting-service features, including locating existing clones from website links and creating pull requests.",
    ),
    ("repo.settings.ignore_list", "Repository ignore list"),
    ("repo.settings.user", "User Information"),
    (
        "repo.settings.use_global_user",
        "Use global user configuration",
    ),
    ("repo.settings.full_name", "Full name"),
    ("repo.settings.email", "Email address"),
    ("repo.settings.commit_links", "Commit text links"),
    ("repo.settings.options", "Options"),
    (
        "repo.settings.auto_refresh",
        "Automatically refresh this repository",
    ),
    (
        "repo.settings.background_remote_refresh",
        "Refresh remote status in the background",
    ),
    ("repo.settings.edit_config_file", "Edit config file..."),
    ("repo.settings.config_failed", "Failed to open config file"),
    ("repo.settings.add", "Add"),
    ("repo.settings.edit", "Edit"),
    ("repo.settings.remove", "Remove"),
    ("settings.language", "Language"),
    ("status.loading_repo", "Loading repository"),
    ("status.hash_copied", "Full hash copied"),
    ("common.more", "more"),
    ("common.local", "local"),
    ("common.remote", "remote"),
    ("diff.loading", "Loading diff"),
    ("diff.queued", "Diff is queued for loading."),
    ("diff.empty", "No textual diff for this file."),
    ("diff.truncated", "Diff truncated at 1200 lines"),
    ("diff.blocks", "Diff blocks"),
    ("diff.full_file", "Full file"),
    ("repo.title", "Repository"),
    ("repo.none", "No repository loaded"),
    ("repo.source.new_tab", "New tab"),
    ("repo.source.close_tab", "Close tab"),
    ("repo.source.title", "Local Repositories"),
    ("repo.source.local", "Local"),
    ("repo.source.remote", "Remote"),
    ("repo.source.clone", "Clone"),
    ("repo.source.add", "Add"),
    ("repo.source.create", "Create"),
    ("repo.source.search", "Search"),
    ("repo.source.local_repositories", "Local repositories"),
    ("repo.source.empty", "No local repositories found."),
    ("repo.source.clone_url", "Source URL"),
    ("repo.source.destination", "Destination Path"),
    ("repo.source.browse", "Browse"),
    ("repo.source.pending", "Waiting to check"),
    ("repo.source.checking", "Checking"),
    ("repo.source.valid", "Valid"),
    ("repo.source.invalid", "Invalid remote"),
    ("repo.git_flow", "Git Workflow"),
    ("repo.remote", "Remote"),
    ("repo.command_mode", "Command Mode"),
    ("repo.resource_manager", "Resource Manager"),
    ("repo.git_flow.opened", "Git workflow opened"),
    ("repo.command_mode.failed", "Failed to open command mode"),
    (
        "repo.resource_manager.failed",
        "Failed to open resource manager",
    ),
    ("repo.remote.missing", "No remote URL configured"),
    ("repo.remote.failed", "Failed to open remote URL"),
    (
        "repo.source.clone_missing",
        "Enter a source URL and destination path.",
    ),
    (
        "repo.source.create_missing",
        "Choose a folder before creating a repository.",
    ),
    ("branch.current", "Branch"),
    ("branch.title", "Branch"),
    ("branch.current_badge", "Current"),
    ("branch.local", "Local Branches"),
    ("branch.remote", "Remote Branches"),
    ("branch.none", "No branches"),
    ("branch.create", "Create branch"),
    ("branch.name", "Branch name"),
    ("branch.checkout", "Checkout branch"),
    ("branch.checkout_remote", "Checkout remote branch"),
    ("branch.sync_remote", "Sync remote branch"),
    ("branch.local_alias", "Local branch alias"),
    ("branch.delete", "Delete branch"),
    ("branch.delete_remote", "Delete remote branch"),
    ("branch.force_delete", "Force delete"),
    ("branch.confirm_delete", "Delete this branch?"),
    ("branch.confirm_delete_remote", "Delete this remote branch?"),
    ("remote.title", "Remote Branches"),
    ("remote.none", "No remote repositories"),
    ("remote.no_branches", "No fetched remote branches"),
    ("worktree.title", "Working Tree"),
    ("worktree.clean", "Clean"),
    ("worktree.clean_detail", "No pending file changes."),
    ("nav.history", "History"),
    ("worktree.stage_all", "Stage all"),
    ("worktree.unstage_all", "Unstage all"),
    ("worktree.staged", "Staged"),
    ("worktree.unstaged", "Unstaged"),
    ("worktree.stage_file", "Stage file"),
    ("worktree.unstage_file", "Unstage file"),
    ("worktree.discard", "Discard changes"),
    ("worktree.view_tree", "Tree view"),
    ("worktree.view_flat", "Full paths"),
    ("worktree.add_gitignore", "Add to .gitignore"),
    ("worktree.resolve_conflict", "Resolve conflict"),
    ("worktree.resolve_conflicts", "Resolve conflicts"),
    ("worktree.conflicts.title", "Conflicts"),
    (
        "worktree.conflicts.detail",
        "Select a conflicted file to resolve.",
    ),
    ("worktree.conflicts.empty", "No conflicted files"),
    ("worktree.accept_yours", "Accept Yours"),
    ("worktree.accept_theirs", "Accept Theirs"),
    ("worktree.merge", "Merge..."),
    ("stash.title", "Stashes"),
    ("stash.none", "No stashes"),
    ("stash.create", "Stash changes"),
    ("stash.message", "Stash message"),
    ("stash.apply", "Apply stash"),
    ("stash.pop", "Pop stash"),
    ("stash.drop", "Drop stash"),
    ("stash.confirm_drop", "Drop this stash?"),
    ("tag.title", "Tags"),
    ("tag.none", "No tags"),
    ("tag.create", "Create tag"),
    ("tag.name", "Tag name"),
    ("tag.checkout", "Checkout tag"),
    ("tag.push", "Push"),
    ("tag.push_after_create", "Push after create"),
    ("tag.remote", "Remote"),
    ("tag.delete", "Delete tag"),
    ("tag.confirm_delete", "Delete this tag?"),
    ("commit.details", "Commit Details"),
    ("commit.none", "No commits found."),
    ("commit.changed_files", "Changed Files"),
    ("commit.loading_files", "Loading files"),
    (
        "commit.select_to_load_files",
        "Select the commit to load files.",
    ),
    ("commit.diff", "Diff"),
    ("commit.hash", "Hash"),
    ("commit.author", "Author"),
    ("commit.when", "When"),
    ("commit.parents", "Parents"),
    ("commit.panel", "Commit"),
    ("commit.message", "Commit message"),
    ("commit.button", "Commit staged changes"),
    ("commit.button.short", "Commit"),
    ("commit.push_immediately", "Push immediately"),
    ("commit.amend", "Amend last commit"),
    ("commit.history", "Commit message history"),
    ("commit.history_empty", "No commit message history"),
    ("commit.options", "Commit options..."),
    ("commit.no_verify", "Bypass commit hooks"),
    ("commit.gpg_sign", "Sign commit"),
    ("commit.staged_files", "staged file(s)"),
    ("commit.no_changes", "No file changes recorded."),
    ("commit.select_file", "Select a changed file."),
    ("commit.search", "Search commits"),
    ("commit.no_matches", "No matching commits"),
    ("commit.no_commits", "No commits yet"),
    ("commit.stats_loaded", "commits loaded"),
    ("commit.stats_lanes", "graph lanes"),
    ("commit.stats_visible", "visible"),
    (
        "commit.no_commits_hint",
        "Create the first commit, then the graph will render here.",
    ),
    ("dialog.cancel", "Cancel"),
    ("dialog.ok", "OK"),
    ("dialog.create", "Create"),
    ("dialog.checkout", "Checkout"),
    ("dialog.discard", "Discard"),
    ("dialog.close", "Close"),
    ("dialog.error.title", "Git error"),
    ("dialog.error.message", "The Git command returned an error."),
    ("menu.copy_hash", "Copy commit hash"),
    ("menu.copy_short_hash", "Copy short hash"),
    ("menu.copy", "Copy"),
    ("menu.checkout_commit", "Checkout this commit"),
    ("menu.create_branch", "Create branch here"),
    ("menu.create_tag", "Create tag here"),
    ("menu.cherry_pick", "Cherry-pick commit"),
    ("menu.revert", "Revert commit"),
    ("menu.reset", "Reset current branch to here"),
    ("menu.compare_worktree", "Compare with working tree"),
    ("menu.open_remote", "Open commit on remote"),
    ("commit.confirm_cherry_pick", "Cherry-pick this commit?"),
    (
        "commit.confirm_cherry_pick_batch",
        "Cherry-pick selected commits?",
    ),
    ("commit.cherry_pick_batch", "Batch cherry-pick"),
    ("commit.cherry_pick_confirm", "Confirm"),
    ("commit.cherry_pick_selected", "selected commits"),
    ("commit.confirm_revert", "Revert this commit?"),
    (
        "commit.confirm_reset",
        "Reset current branch to this commit?",
    ),
    ("commit.create_from", "Create from commit"),
    ("commit.tag_commit", "Tag commit"),
    ("commit.checkout_confirm", "Checkout commit"),
    (
        "commit.detached_warning",
        "This will put the repository in detached HEAD state.",
    ),
    ("reset.soft", "Soft"),
    ("reset.mixed", "Mixed"),
    ("reset.hard", "Hard"),
];

const ZH: &[(&str, &str)] = &[
    ("app.title", "Git Agent"),
    ("app.subtitle", "高速可视化 Git 客户端"),
    ("action.open", "打开"),
    ("action.refresh", "刷新"),
    ("action.fetch", "获取"),
    ("action.pull", "拉取"),
    ("action.push", "推送"),
    ("settings.title", "设置"),
    ("options.title", "\u{9009}\u{9879}"),
    ("repo.settings", "\u{4ed3}\u{5e93}\u{8bbe}\u{7f6e}"),
    ("repo.settings.title", "\u{4ed3}\u{5e93}\u{8bbe}\u{7f6e}"),
    ("settings.language", "语言"),
    ("status.loading_repo", "正在加载仓库"),
    ("common.more", "更多"),
    ("common.local", "本地"),
    ("common.remote", "远端"),
    ("diff.loading", "正在加载差异"),
    ("diff.queued", "差异正在排队加载。"),
    ("diff.empty", "这个文件没有可显示的文本差异。"),
    ("diff.truncated", "差异已截断到 1200 行"),
    ("repo.title", "仓库"),
    ("repo.none", "未加载仓库"),
    ("branch.current", "当前分支"),
    ("branch.local", "本地分支"),
    ("branch.remote", "远端分支"),
    ("branch.none", "没有分支"),
    ("branch.create", "创建分支"),
    ("branch.name", "分支名称"),
    ("branch.checkout", "检出分支"),
    ("branch.checkout_remote", "检出远端分支"),
    ("branch.delete", "删除分支"),
    ("branch.force_delete", "强制删除"),
    ("branch.confirm_delete", "删除这个分支？"),
    ("remote.title", "远端仓库"),
    ("remote.none", "没有远端仓库"),
    ("worktree.title", "工作区"),
    ("worktree.clean", "干净"),
    ("worktree.clean_detail", "没有待处理的文件变更。"),
    ("nav.history", "历史"),
    ("worktree.stage_all", "全部暂存"),
    ("worktree.unstage_all", "全部取消暂存"),
    ("worktree.staged", "已暂存"),
    ("worktree.unstaged", "未暂存"),
    ("worktree.stage_file", "暂存文件"),
    ("worktree.unstage_file", "取消暂存"),
    ("worktree.discard", "丢弃更改"),
    ("worktree.view_tree", "树形展示"),
    ("worktree.view_flat", "完整路径"),
    ("worktree.add_gitignore", "添加到 .gitignore"),
    ("worktree.resolve_conflict", "解决冲突"),
    ("worktree.resolve_conflicts", "解决冲突"),
    ("worktree.conflicts.title", "冲突"),
    ("worktree.conflicts.detail", "选择要解决的冲突文件。"),
    ("worktree.conflicts.empty", "没有冲突文件"),
    ("worktree.accept_yours", "接受本地"),
    ("worktree.accept_theirs", "接受远端"),
    ("worktree.merge", "合并..."),
    ("stash.title", "贮藏"),
    ("stash.none", "没有贮藏"),
    ("stash.create", "贮藏更改"),
    ("stash.message", "贮藏信息"),
    ("stash.apply", "应用贮藏"),
    ("stash.pop", "弹出贮藏"),
    ("stash.drop", "删除贮藏"),
    ("stash.confirm_drop", "删除这个贮藏？"),
    ("tag.title", "标签"),
    ("tag.none", "没有标签"),
    ("tag.create", "创建标签"),
    ("tag.name", "标签名称"),
    ("tag.checkout", "检出标签"),
    ("tag.push", "推送"),
    ("tag.push_after_create", "创建后推送"),
    ("tag.remote", "远端"),
    ("tag.delete", "删除标签"),
    ("tag.confirm_delete", "删除这个标签？"),
    ("commit.details", "提交详情"),
    ("commit.none", "没有提交。"),
    ("commit.changed_files", "变更文件"),
    ("commit.loading_files", "正在加载文件"),
    ("commit.select_to_load_files", "选择提交后加载文件。"),
    ("commit.diff", "差异"),
    ("commit.hash", "哈希"),
    ("commit.author", "作者"),
    ("commit.when", "时间"),
    ("commit.parents", "父提交"),
    ("commit.panel", "提交"),
    ("commit.message", "提交信息"),
    ("commit.button", "提交已暂存更改"),
    ("commit.button.short", "提交"),
    ("commit.push_immediately", "立即推送"),
    ("commit.amend", "修改最后一次提交"),
    ("commit.history", "提交信息历史"),
    ("commit.history_empty", "没有提交信息历史"),
    ("commit.options", "提交选项..."),
    ("commit.no_verify", "绕过提交钩子"),
    ("commit.gpg_sign", "签名提交"),
    ("commit.staged_files", "个已暂存文件"),
    ("commit.no_changes", "没有记录文件变更。"),
    ("commit.select_file", "选择一个变更文件。"),
    ("commit.search", "搜索提交"),
    ("commit.no_matches", "没有匹配的提交"),
    ("commit.no_commits", "还没有提交"),
    ("commit.stats_loaded", "个提交已加载"),
    ("commit.stats_lanes", "条图谱泳道"),
    ("commit.stats_visible", "个可见"),
    (
        "commit.no_commits_hint",
        "创建第一次提交后，图谱会显示在这里。",
    ),
    ("dialog.cancel", "取消"),
    ("dialog.ok", "确定"),
    ("dialog.create", "创建"),
    ("dialog.checkout", "检出"),
    ("dialog.discard", "丢弃"),
    ("dialog.close", "关闭"),
    ("dialog.error.title", "Git 错误"),
    ("dialog.error.message", "Git 命令返回了错误。"),
    ("menu.copy_hash", "复制完整 hash"),
    ("menu.copy_short_hash", "复制短 hash"),
    ("menu.checkout_commit", "检出此提交"),
    ("menu.create_branch", "从这里创建分支"),
    ("menu.create_tag", "从这里创建标签"),
    ("menu.cherry_pick", "拣选此提交"),
    ("menu.revert", "还原此提交"),
    ("menu.reset", "重置当前分支到这里"),
    ("menu.compare_worktree", "与工作区比较"),
    ("menu.open_remote", "在远端打开提交"),
    ("commit.confirm_cherry_pick", "拣选这个提交？"),
    ("commit.confirm_revert", "还原这个提交？"),
    ("commit.confirm_reset", "重置当前分支到这个提交？"),
    ("commit.create_from", "从提交创建"),
    ("commit.tag_commit", "标记提交"),
    ("commit.checkout_confirm", "检出提交"),
    ("commit.detached_warning", "这会让仓库进入分离 HEAD 状态。"),
    ("reset.soft", "软重置"),
    ("reset.mixed", "混合重置"),
    ("reset.hard", "硬重置"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_labels_are_not_mojibake() {
        assert_eq!(Language::Chinese.code(), "\u{4e2d}\u{6587}");
        assert_eq!(t(Language::Chinese, "action.push"), "\u{63a8}\u{9001}");
        assert_eq!(
            t(Language::Chinese, "commit.details"),
            "\u{63d0}\u{4ea4}\u{8be6}\u{60c5}"
        );
        assert_eq!(t(Language::Chinese, "dialog.ok"), "\u{786e}\u{5b9a}");
        assert_eq!(
            t(Language::Chinese, "repo.source.clone"),
            "\u{514b}\u{9686}"
        );
        assert_eq!(t(Language::Chinese, "repo.source.add"), "\u{6dfb}\u{52a0}");
        assert_eq!(
            t(Language::Chinese, "repo.source.create"),
            "\u{521b}\u{5efa}"
        );
        assert_eq!(
            t(Language::Chinese, "repo.source.invalid"),
            "\u{65e0}\u{6548}\u{8fde}\u{63a5}"
        );
        assert_eq!(
            t(Language::Chinese, "repo.command_mode"),
            "\u{547d}\u{4ee4}\u{884c}\u{6a21}\u{5f0f}"
        );
        assert_eq!(t(Language::Chinese, "repo.remote"), "\u{8fdc}\u{7aef}");
        assert_eq!(
            t(Language::Chinese, "repo.source.remote"),
            "\u{8fdc}\u{7aef}"
        );
        assert_eq!(t(Language::Chinese, "common.remote"), "\u{8fdc}\u{7aef}");
        assert_eq!(
            t(Language::Chinese, "branch.remote"),
            "\u{8fdc}\u{7aef}\u{5206}\u{652f}"
        );
        assert_eq!(
            t(Language::Chinese, "remote.title"),
            "\u{8fdc}\u{7aef}\u{5206}\u{652f}"
        );
        assert_eq!(
            t(Language::Chinese, "repo.remote.missing"),
            "\u{672a}\u{914d}\u{7f6e}\u{8fdc}\u{7aef} URL"
        );
    }
}
