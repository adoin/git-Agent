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
    ("diff.revert_hunk", "\u{56de}\u{6eda}\u{533a}\u{5757}"),
    (
        "diff.revert_select_line",
        "\u{8bf7}\u{5148}\u{9009}\u{62e9}\u{8981}\u{56de}\u{6eda}\u{7684}\u{5dee}\u{5f02}\u{884c}",
    ),
    ("settings.appearance", "\u{5916}\u{89c2}"),
    ("settings.theme", "\u{4e3b}\u{9898}"),
    ("settings.language", "\u{8bed}\u{8a00}"),
    (
        "settings.ssh_configuration",
        "SSH \u{5ba2}\u{6237}\u{7aef}\u{914d}\u{7f6e}",
    ),
    ("settings.ssh_client", "SSH \u{5ba2}\u{6237}\u{7aef}"),
    ("settings.ssh_key", "SSH \u{5bc6}\u{94a5}"),
    (
        "settings.ssh_choose_key",
        "\u{9009}\u{62e9} SSH \u{5bc6}\u{94a5}",
    ),
    (
        "settings.ssh_executable",
        "\u{5ba2}\u{6237}\u{7aef}\u{7a0b}\u{5e8f}",
    ),
    (
        "settings.ssh_executable_placeholder",
        "\u{7559}\u{7a7a}\u{65f6}\u{6309}\u{6240}\u{9009}\u{5ba2}\u{6237}\u{7aef}\u{81ea}\u{52a8}\u{67e5}\u{627e}",
    ),
    (
        "settings.ssh_choose_executable",
        "\u{9009}\u{62e9}\u{5ba2}\u{6237}\u{7aef}\u{7a0b}\u{5e8f}",
    ),
    (
        "settings.ssh_auto_detected",
        "\u{81ea}\u{52a8}\u{68c0}\u{6d4b}",
    ),
    (
        "settings.ssh_not_found",
        "\u{672a}\u{627e}\u{5230}\u{5ba2}\u{6237}\u{7aef}\u{7a0b}\u{5e8f}",
    ),
    (
        "ssh.agent_started",
        "SSH \u{52a9}\u{624b}\u{5df2}\u{542f}\u{52a8}",
    ),
    ("ssh.status.title", "SSH \u{52a9}\u{624b}"),
    ("ssh.status.client", "\u{5ba2}\u{6237}\u{7aef}"),
    ("ssh.status.state", "\u{72b6}\u{6001}"),
    ("ssh.status.running", "\u{6b63}\u{5728}\u{8fd0}\u{884c}"),
    (
        "ssh.status.starting",
        "\u{6b63}\u{5728}\u{542f}\u{52a8} SSH \u{52a9}\u{624b}",
    ),
    (
        "ssh.status.refreshing",
        "\u{6b63}\u{5728}\u{5237}\u{65b0}\u{72b6}\u{6001}",
    ),
    (
        "ssh.status.adding_key",
        "\u{6b63}\u{5728}\u{6dfb}\u{52a0} SSH \u{5bc6}\u{94a5}",
    ),
    ("ssh.status.backend", "\u{52a9}\u{624b}\u{540e}\u{7aef}"),
    (
        "ssh.status.loaded_keys",
        "\u{5df2}\u{52a0}\u{8f7d}\u{5bc6}\u{94a5}",
    ),
    (
        "ssh.status.no_keys",
        "\u{672a}\u{52a0}\u{8f7d}\u{4efb}\u{4f55} SSH \u{5bc6}\u{94a5}",
    ),
    (
        "ssh.status.external_key_list",
        "\u{5bc6}\u{94a5}\u{5217}\u{8868}\u{7531} Pageant \u{7ba1}\u{7406}\u{3002}",
    ),
    (
        "ssh.status.openssh_background",
        "OpenSSH ssh-agent \u{4f5c}\u{4e3a}\u{540e}\u{53f0}\u{670d}\u{52a1}\u{8fd0}\u{884c}\u{ff0c}\u{4e0d}\u{4f1a}\u{6253}\u{5f00}\u{72ec}\u{7acb}\u{7a97}\u{53e3}\u{3002}",
    ),
    (
        "ssh.status.putty_tray",
        "Pageant \u{5728}\u{7cfb}\u{7edf}\u{6258}\u{76d8}\u{8fd0}\u{884c}\u{ff0c}\u{53ef}\u{5728} Pageant \u{4e2d}\u{67e5}\u{770b}\u{548c}\u{7ba1}\u{7406}\u{5bc6}\u{94a5}\u{3002}",
    ),
    ("ssh.status.refresh", "\u{5237}\u{65b0}"),
    ("ssh.status.add_key", "\u{6dfb}\u{52a0}\u{5bc6}\u{94a5}"),
    (
        "ssh.load_key.title",
        "\u{52a0}\u{8f7d} SSH \u{5bc6}\u{94a5}\u{ff1f}",
    ),
    (
        "ssh.load_key.message",
        "\u{8981}\u{73b0}\u{5728}\u{52a0}\u{8f7d}\u{4e00}\u{4e2a} SSH \u{5bc6}\u{94a5}\u{5417}\u{ff1f}\u{9009}\u{62e9}\u{201c}\u{5426}\u{201d}\u{540e}\u{ff0c}\u{4ecd}\u{53ef}\u{7a0d}\u{540e}\u{901a}\u{8fc7}\u{201c}\u{5de5}\u{5177} > \u{6dfb}\u{52a0} SSH \u{5bc6}\u{94a5}\u{201d}\u{52a0}\u{8f7d}\u{3002}",
    ),
    ("ssh.load_key.yes", "\u{662f}"),
    ("ssh.load_key.no", "\u{5426}"),
    (
        "ssh.key_added",
        "SSH \u{5bc6}\u{94a5}\u{5df2}\u{4ea4}\u{7ed9}\u{52a9}\u{624b}",
    ),
    (
        "ssh.task_stopped",
        "SSH \u{5de5}\u{5177}\u{4efb}\u{52a1}\u{610f}\u{5916}\u{505c}\u{6b62}",
    ),
    (
        "ssh.openssh_agent_not_found",
        "\u{672a}\u{627e}\u{5230} OpenSSH ssh-add\u{3002}\u{8bf7}\u{5728}\u{201c}\u{9009}\u{9879} > Git\u{201d}\u{4e2d}\u{9009}\u{62e9}\u{6709}\u{6548}\u{7684} OpenSSH \u{5ba2}\u{6237}\u{7aef}\u{7a0b}\u{5e8f}\u{3002}",
    ),
    (
        "ssh.openssh_agent_service_missing",
        "\u{672a}\u{627e}\u{5230} Windows OpenSSH ssh-agent \u{670d}\u{52a1}\u{3002}\u{8bf7}\u{5148}\u{5b89}\u{88c5} Windows OpenSSH \u{5ba2}\u{6237}\u{7aef}\u{529f}\u{80fd}\u{3002}",
    ),
    (
        "ssh.openssh_agent_elevation_failed",
        "\u{65e0}\u{6cd5}\u{542f}\u{7528} Windows OpenSSH ssh-agent \u{670d}\u{52a1}\u{3002}\u{8bf7}\u{5141}\u{8bb8}\u{7ba1}\u{7406}\u{5458}\u{6743}\u{9650}\u{8bf7}\u{6c42}\u{ff0c}\u{7136}\u{540e}\u{91cd}\u{8bd5}\u{3002}",
    ),
    (
        "ssh.openssh_agent_connect_failed",
        "Windows OpenSSH ssh-agent \u{670d}\u{52a1}\u{5df2}\u{542f}\u{52a8}\u{ff0c}\u{4f46} ssh-add \u{65e0}\u{6cd5}\u{8fde}\u{63a5}\u{3002}",
    ),
    (
        "ssh.openssh_installed",
        "OpenSSH \u{5ba2}\u{6237}\u{7aef}\u{5df2}\u{5b89}\u{88c5}",
    ),
    (
        "ssh.openssh_install.title",
        "\u{672a}\u{627e}\u{5230} OpenSSH",
    ),
    (
        "ssh.openssh_install.message",
        "\u{5f53}\u{524d}\u{9009}\u{62e9} OpenSSH\u{ff0c}\u{4f46}\u{7cfb}\u{7edf}\u{4e2d}\u{672a}\u{627e}\u{5230}\u{53ef}\u{7528}\u{7684} ssh \u{5ba2}\u{6237}\u{7aef}\u{7a0b}\u{5e8f}\u{3002}",
    ),
    (
        "ssh.openssh_install.windows_hint",
        "Windows \u{5c06}\u{901a}\u{8fc7}\u{7cfb}\u{7edf}\u{53ef}\u{9009}\u{529f}\u{80fd}\u{5b89}\u{88c5} OpenSSH Client\u{3002}\u{9700}\u{8981}\u{7ba1}\u{7406}\u{5458}\u{6743}\u{9650}\u{548c}\u{7f51}\u{7edc}\u{8fde}\u{63a5}\u{3002}",
    ),
    (
        "ssh.openssh_install.unix_hint",
        "macOS \u{901a}\u{5e38}\u{81ea}\u{5e26} OpenSSH\u{ff1b}Linux \u{8bf7}\u{901a}\u{8fc7}\u{7cfb}\u{7edf}\u{5305}\u{7ba1}\u{7406}\u{5668}\u{5b89}\u{88c5} openssh-client\u{ff0c}\u{6216}\u{624b}\u{52a8}\u{6307}\u{5b9a}\u{5ba2}\u{6237}\u{7aef}\u{7a0b}\u{5e8f}\u{3002}",
    ),
    ("ssh.openssh_install.install", "\u{5b89}\u{88c5} OpenSSH"),
    (
        "ssh.openssh_install.installing",
        "\u{6b63}\u{5728}\u{5b89}\u{88c5} OpenSSH\u{3002}\u{8bf7}\u{5728}\u{7cfb}\u{7edf}\u{63d0}\u{793a}\u{4e2d}\u{5141}\u{8bb8}\u{7ba1}\u{7406}\u{5458}\u{6743}\u{9650}\u{3002}",
    ),
    (
        "ssh.openssh_install.guide",
        "\u{6253}\u{5f00}\u{5b89}\u{88c5}\u{6307}\u{5357}",
    ),
    (
        "ssh.openssh_install.recheck",
        "\u{91cd}\u{65b0}\u{68c0}\u{6d4b}",
    ),
    (
        "ssh.openssh_install.detected",
        "\u{5df2}\u{68c0}\u{6d4b}\u{5230} OpenSSH \u{5ba2}\u{6237}\u{7aef}",
    ),
    (
        "ssh.openssh_install.use_putty",
        "\u{4f7f}\u{7528} PuTTY / Plink",
    ),
    (
        "ssh.openssh_install.failed",
        "\u{65e0}\u{6cd5}\u{5b89}\u{88c5} Windows OpenSSH \u{5ba2}\u{6237}\u{7aef}\u{3002}\u{8bf7}\u{68c0}\u{67e5}\u{7f51}\u{7edc}\u{8fde}\u{63a5}\u{548c} Windows \u{53ef}\u{9009}\u{529f}\u{80fd}\u{670d}\u{52a1}\u{ff0c}\u{6216}\u{6253}\u{5f00}\u{5b89}\u{88c5}\u{6307}\u{5357}\u{624b}\u{52a8}\u{5b89}\u{88c5}\u{3002}",
    ),
    (
        "ssh.openssh_install.guided_only",
        "\u{5f53}\u{524d}\u{7cfb}\u{7edf}\u{4e0d}\u{652f}\u{6301}\u{81ea}\u{52a8}\u{5b89}\u{88c5} OpenSSH\u{3002}\u{8bf7}\u{6309}\u{5b89}\u{88c5}\u{6307}\u{5357}\u{64cd}\u{4f5c}\u{ff0c}\u{6216}\u{5728}\u{201c}\u{9009}\u{9879} > Git\u{201d}\u{4e2d}\u{6307}\u{5b9a}\u{5ba2}\u{6237}\u{7aef}\u{7a0b}\u{5e8f}\u{3002}",
    ),
    (
        "ssh.install.title",
        "\u{672a}\u{627e}\u{5230} PuTTY / Plink",
    ),
    (
        "ssh.install.message",
        "\u{5f53}\u{524d}\u{9009}\u{62e9} PuTTY / Plink\u{ff0c}\u{4f46}\u{7cfb}\u{7edf}\u{4e2d}\u{7f3a}\u{5c11} Plink \u{6216} Pageant\u{3002}PuTTY \u{514d}\u{8d39}\u{4e14}\u{5f00}\u{6e90}\u{3002}\u{8bf7}\u{5b89}\u{88c5}\u{5b8c}\u{6574}\u{7684} PuTTY\u{ff0c}\u{6216}\u{5207}\u{6362}\u{5230} OpenSSH\u{ff1b}\u{4e5f}\u{53ef}\u{4ee5}\u{5728}\u{201c}\u{9009}\u{9879} > Git\u{201d}\u{4e2d}\u{6307}\u{5b9a}\u{5df2}\u{5b89}\u{88c5}\u{7684}\u{5ba2}\u{6237}\u{7aef}\u{7a0b}\u{5e8f}\u{3002}",
    ),
    (
        "ssh.install.auto_detect_hint",
        "\u{7559}\u{7a7a}\u{65f6}\u{ff0c}\u{672c}\u{7a0b}\u{5e8f}\u{4f1a}\u{6309}\u{5f53}\u{524d}\u{64cd}\u{4f5c}\u{7cfb}\u{7edf}\u{548c} SSH \u{5ba2}\u{6237}\u{7aef}\u{7c7b}\u{578b}\u{81ea}\u{52a8}\u{67e5}\u{627e}\u{3002}\u{624b}\u{52a8}\u{6307}\u{5b9a}\u{540e}\u{ff0c}\u{5c06}\u{4f18}\u{5148}\u{4f7f}\u{7528}\u{8be5}\u{6587}\u{4ef6}\u{3002}",
    ),
    (
        "ssh.install.download",
        "\u{6253}\u{5f00}\u{5b98}\u{65b9}\u{4e0b}\u{8f7d}",
    ),
    ("ssh.install.use_openssh", "\u{4f7f}\u{7528} OpenSSH"),
    (
        "ssh.install.open_settings",
        "\u{6253}\u{5f00} SSH \u{8bbe}\u{7f6e}",
    ),
    ("ssh.install.later", "\u{7a0d}\u{540e}"),
    ("about.title", "\u{5173}\u{4e8e} Git Agent"),
    ("about.version", "\u{7248}\u{672c}"),
    ("about.repository", "\u{9879}\u{76ee}\u{4e3b}\u{9875}"),
    ("update.title", "\u{68c0}\u{67e5}\u{66f4}\u{65b0}"),
    ("update.current_version", "\u{5f53}\u{524d}\u{7248}\u{672c}"),
    ("update.latest_version", "\u{6700}\u{65b0}\u{7248}\u{672c}"),
    (
        "update.checking",
        "\u{6b63}\u{5728}\u{68c0}\u{67e5} GitHub Release",
    ),
    (
        "update.installing",
        "\u{6b63}\u{5728}\u{4e0b}\u{8f7d}\u{5e76}\u{542f}\u{52a8}\u{5b89}\u{88c5}",
    ),
    (
        "update.available",
        "\u{6709}\u{53ef}\u{7528}\u{66f4}\u{65b0}",
    ),
    (
        "update.up_to_date",
        "\u{5f53}\u{524d}\u{5df2}\u{662f}\u{6700}\u{65b0}\u{7248}\u{672c}",
    ),
    ("update.package", "\u{5b89}\u{88c5}\u{5305}"),
    (
        "update.no_asset",
        "\u{672a}\u{627e}\u{5230}\u{9002}\u{7528}\u{4e8e}\u{5f53}\u{524d}\u{7cfb}\u{7edf}\u{7684}\u{5b89}\u{88c5}\u{5305}",
    ),
    (
        "update.download_install",
        "\u{4e0b}\u{8f7d}\u{5e76}\u{5b89}\u{88c5}",
    ),
    (
        "update.open_release",
        "\u{6253}\u{5f00}\u{53d1}\u{5e03}\u{9875}\u{9762}",
    ),
    ("update.retry", "\u{91cd}\u{8bd5}"),
    (
        "update.failed",
        "\u{68c0}\u{67e5}\u{6216}\u{5b89}\u{88c5}\u{66f4}\u{65b0}\u{5931}\u{8d25}",
    ),
    (
        "update.installed",
        "\u{66f4}\u{65b0}\u{5df2}\u{5b89}\u{88c5}",
    ),
    (
        "update.restart_required",
        "\u{8bf7}\u{91cd}\u{542f} Git Agent \u{4ee5}\u{4f7f}\u{7528}\u{65b0}\u{7248}\u{672c}",
    ),
    (
        "update.stopped",
        "\u{66f4}\u{65b0}\u{4efb}\u{52a1}\u{610f}\u{5916}\u{505c}\u{6b62}",
    ),
    ("menu.copy", "\u{590d}\u{5236}"),
    ("action.clone_new", "\u{514b}\u{9686}/\u{65b0}\u{5efa}"),
    (
        "menu.interactive_rebase_children",
        "\u{4ea4}\u{4e92}\u{5f0f}\u{53d8}\u{57fa}\u{6b64}\u{63d0}\u{4ea4}\u{4e4b}\u{540e}\u{7684}\u{63d0}\u{4ea4}...",
    ),
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
    ("menu.compare", "\u{6bd4}\u{8f83}"),
    (
        "menu.external_diff",
        "\u{5916}\u{90e8}\u{5dee}\u{5f02}\u{5bf9}\u{6bd4}",
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
        "pull_request.title",
        "\u{521b}\u{5efa}\u{62c9}\u{53d6}\u{8bf7}\u{6c42}",
    ),
    (
        "pull_request.remote",
        "\u{901a}\u{8fc7}\u{8fdc}\u{7aef}\u{63d0}\u{4ea4}:",
    ),
    (
        "pull_request.local_branch",
        "\u{672c}\u{5730}\u{5206}\u{652f}",
    ),
    (
        "pull_request.remote_branch",
        "\u{8fdc}\u{7aef}\u{5206}\u{652f}",
    ),
    (
        "pull_request.remote_branch_placeholder",
        "\u{8bf7}\u{8f93}\u{5165}\u{8fdc}\u{7aef}\u{5206}\u{652f}",
    ),
    (
        "pull_request.hint",
        "\u{5728}\u{521b}\u{5efa}\u{62c9}\u{53d6}\u{8bf7}\u{6c42}\u{4e4b}\u{524d}\u{7684}\u{6700}\u{540e}\u{4e00}\u{6b21}\u{63d0}\u{4ea4}\u{5c06}\u{88ab}\u{63a8}\u{9001}",
    ),
    (
        "pull_request.submit",
        "\u{5728}\u{7f51}\u{4e0a}\u{521b}\u{5efa}\u{62c9}\u{53d6}\u{8bf7}\u{6c42}",
    ),
    (
        "pull_request.error.remote_invalid",
        "\u{8bf7}\u{9009}\u{62e9}\u{6709}\u{6548}\u{8fdc}\u{7aef}",
    ),
    (
        "pull_request.error.local_branch_invalid",
        "\u{8bf7}\u{9009}\u{62e9}\u{6709}\u{6548}\u{672c}\u{5730}\u{5206}\u{652f}",
    ),
    (
        "pull_request.error.remote_branch_invalid",
        "\u{8bf7}\u{8f93}\u{5165}\u{6709}\u{6548}\u{8fdc}\u{7aef}\u{5206}\u{652f}",
    ),
    (
        "git_flow.initialize.title",
        "\u{521d}\u{59cb}\u{5316} Git \u{5de5}\u{4f5c}\u{6d41}",
    ),
    (
        "git_flow.initialize.detail",
        "Git Flow 会保存 Production/Develop 分支和 Feature/Release/Hotfix 前缀配置。若 Develop 分支不存在，会从 Production 分支创建；Feature/Release/Hotfix 分支会在对应开始动作时创建。",
    ),
    ("git_flow.production_branch", "Production \u{5206}\u{652f}"),
    ("git_flow.development_branch", "Develop \u{5206}\u{652f}"),
    ("git_flow.feature_prefix", "Feature \u{524d}\u{7f00}"),
    ("git_flow.release_prefix", "Release \u{524d}\u{7f00}"),
    ("git_flow.hotfix_prefix", "Hotfix \u{524d}\u{7f00}"),
    ("git_flow.version_tag_prefix", "Tag \u{524d}\u{7f00}"),
    (
        "git_flow.use_defaults",
        "\u{4f7f}\u{7528}\u{9ed8}\u{8ba4}\u{8bbe}\u{7f6e}",
    ),
    ("git_flow.initialize.submit", "\u{521d}\u{59cb}\u{5316}"),
    (
        "git_flow.other_action.title",
        "\u{9009}\u{62e9} Git Flow \u{64cd}\u{4f5c}",
    ),
    ("git_flow.name", "\u{540d}\u{79f0}"),
    ("git_flow.feature_name", "\u{529f}\u{80fd}\u{540d}\u{79f0}"),
    (
        "git_flow.release_name",
        "\u{53d1}\u{5e03}\u{7248}\u{672c}\u{540d}",
    ),
    (
        "git_flow.hotfix_name",
        "\u{4fee}\u{590d}\u{8865}\u{4e01}\u{540d}",
    ),
    ("git_flow.start_from", "\u{5f00}\u{59cb}\u{4e8e}:"),
    ("git_flow.branch_name", "\u{5206}\u{652f}\u{540d}"),
    (
        "git_flow.branch_preview",
        "\u{5c06}\u{521b}\u{5efa}\u{5206}\u{652f}:",
    ),
    ("git_flow.preview", "\u{9884}\u{89c8}"),
    (
        "git_flow.preview.create_branch",
        "\u{521b}\u{5efa}\u{65b0}\u{5206}\u{652f}",
    ),
    (
        "git_flow.preview.missing_start",
        "\u{672a}\u{9009}\u{62e9}\u{5f00}\u{59cb}\u{70b9}",
    ),
    ("git_flow.preview.merge_prefix", "\u{5c06}"),
    ("git_flow.preview.merge_suffix", "\u{5408}\u{5e76}\u{5230}"),
    (
        "git_flow.preview.latest_feature",
        "\u{6700}\u{65b0}\u{7684}\u{529f}\u{80fd}\u{5206}\u{652f}",
    ),
    (
        "git_flow.preview.latest_release",
        "\u{6700}\u{65b0}\u{7684}\u{53d1}\u{5e03}\u{7248}\u{672c}\u{5206}\u{652f}",
    ),
    (
        "git_flow.preview.latest_hotfix",
        "\u{6700}\u{65b0}\u{7684}\u{4fee}\u{590d}\u{8865}\u{4e01}\u{5206}\u{652f}",
    ),
    (
        "git_flow.start_feature.title",
        "\u{5efa}\u{7acb}\u{65b0}\u{7684}\u{529f}\u{80fd}",
    ),
    (
        "git_flow.finish_feature.title",
        "\u{5b8c}\u{6210}\u{529f}\u{80fd}",
    ),
    (
        "git_flow.start_release.title",
        "\u{5efa}\u{7acb}\u{65b0}\u{7684}\u{53d1}\u{5e03}\u{7248}\u{672c}",
    ),
    (
        "git_flow.finish_release.title",
        "\u{5b8c}\u{6210}\u{53d1}\u{5e03}\u{7248}\u{672c}",
    ),
    (
        "git_flow.start_hotfix.title",
        "\u{5efa}\u{7acb}\u{65b0}\u{7684}\u{4fee}\u{590d}\u{8865}\u{4e01}",
    ),
    (
        "git_flow.finish_hotfix.title",
        "\u{5b8c}\u{6210}\u{4fee}\u{590d}\u{8865}\u{4e01}",
    ),
    (
        "git_flow.start.detail",
        "\u{4ece}\u{914d}\u{7f6e}\u{7684}\u{57fa}\u{7840}\u{5206}\u{652f}\u{521b}\u{5efa}\u{5e76}\u{68c0}\u{51fa}\u{65b0}\u{5206}\u{652f}\u{3002}",
    ),
    (
        "git_flow.finish_feature.detail",
        "\u{5c06}\u{529f}\u{80fd}\u{5206}\u{652f}\u{5408}\u{5e76}\u{56de} develop \u{5e76}\u{5220}\u{9664}\u{529f}\u{80fd}\u{5206}\u{652f}\u{3002}",
    ),
    (
        "git_flow.finish_release.detail",
        "\u{5c06}\u{53d1}\u{5e03}\u{5206}\u{652f}\u{5408}\u{5e76}\u{5230} production \u{548c} develop\u{ff0c}\u{5e76}\u{521b}\u{5efa} tag\u{3002}",
    ),
    (
        "git_flow.finish_hotfix.detail",
        "\u{5c06}\u{4fee}\u{590d}\u{5206}\u{652f}\u{5408}\u{5e76}\u{5230} production \u{548c} develop\u{ff0c}\u{5e76}\u{521b}\u{5efa} tag\u{3002}",
    ),
    ("git_flow.start", "\u{5f00}\u{59cb}"),
    ("git_flow.finish", "\u{5b8c}\u{6210}"),
    (
        "git_flow.finish.rebase_development",
        "\u{5728}\u{5f00}\u{53d1}\u{5206}\u{652f}\u{4e0a}\u{8fdb}\u{884c}\u{53d8}\u{57fa}",
    ),
    ("git_flow.finish.after", "\u{5b8c}\u{6210}\u{540e}:"),
    (
        "git_flow.finish.delete_branch",
        "\u{5220}\u{9664}\u{5206}\u{652f}",
    ),
    (
        "git_flow.finish.force_delete",
        "\u{5f3a}\u{5236}\u{5220}\u{9664}",
    ),
    (
        "git_flow.finish.tag_message",
        "\u{6b64}\u{4fe1}\u{606f}\u{7684}\u{6807}\u{7b7e}:",
    ),
    (
        "git_flow.finish.tag_message_placeholder",
        "\u{8bf7}\u{8f93}\u{5165}\u{6807}\u{7b7e}\u{4fe1}\u{606f}",
    ),
    (
        "git_flow.finish.push_remote",
        "\u{63a8}\u{9001}\u{53d8}\u{66f4}\u{5230}\u{8fdc}\u{7aef}\u{4ed3}\u{5e93}",
    ),
    (
        "git_flow.error.fix_inputs",
        "\u{8bf7}\u{4fee}\u{6b63}\u{4ee5}\u{4e0b}\u{8f93}\u{5165}:",
    ),
    (
        "git_flow.error.required",
        "\u{5fc5}\u{586b}\u{9879}\u{4e0d}\u{80fd}\u{4e3a}\u{7a7a}",
    ),
    (
        "git_flow.error.branch_invalid",
        "\u{5206}\u{652f}\u{540d}\u{4e0d}\u{5408}\u{6cd5}",
    ),
    (
        "git_flow.error.branch_same",
        "production \u{548c} develop \u{5206}\u{652f}\u{4e0d}\u{80fd}\u{76f8}\u{540c}",
    ),
    (
        "git_flow.error.branch_exists",
        "\u{5206}\u{652f}\u{5df2}\u{5b58}\u{5728}",
    ),
    (
        "git_flow.error.branch_prefix",
        "\u{5206}\u{652f}\u{524d}\u{7f00}\u{4e0d}\u{5339}\u{914d}",
    ),
    (
        "git_flow.error.branch_missing",
        "\u{5206}\u{652f}\u{4e0d}\u{5b58}\u{5728}",
    ),
    (
        "git_flow.error.start_point_missing",
        "\u{8bf7}\u{9009}\u{62e9}\u{6709}\u{6548}\u{7684}\u{5f00}\u{59cb}\u{70b9}",
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
        "benchmark.title",
        "\u{4ed3}\u{5e93}\u{6027}\u{80fd}\u{57fa}\u{51c6}\u{6d4b}\u{8bd5}",
    ),
    (
        "benchmark.running",
        "\u{6b63}\u{5728}\u{6d4b}\u{8bd5}\u{4ed3}\u{5e93}\u{6027}\u{80fd}...",
    ),
    (
        "benchmark.step.branches",
        "\u{6b63}\u{5728}\u{8bfb}\u{53d6}\u{672c}\u{5730}\u{5206}\u{652f}",
    ),
    (
        "benchmark.step.remote_branches",
        "\u{6b63}\u{5728}\u{8bfb}\u{53d6}\u{8fdc}\u{7aef}\u{5206}\u{652f}",
    ),
    (
        "benchmark.step.tracking",
        "\u{6b63}\u{5728}\u{8bfb}\u{53d6}\u{8ddf}\u{8e2a}\u{5206}\u{652f}",
    ),
    (
        "benchmark.step.summary",
        "\u{6b63}\u{5728}\u{8bfb}\u{53d6}\u{4ed3}\u{5e93}\u{6458}\u{8981}",
    ),
    (
        "benchmark.step.tags",
        "\u{6b63}\u{5728}\u{8bfb}\u{53d6}\u{6807}\u{7b7e}",
    ),
    (
        "benchmark.step.commit_labels",
        "\u{6b63}\u{5728}\u{8bfb}\u{53d6}\u{63d0}\u{4ea4}\u{6807}\u{8bb0}",
    ),
    (
        "benchmark.step.stashes",
        "\u{6b63}\u{5728}\u{8bfb}\u{53d6}\u{8d2e}\u{85cf}",
    ),
    (
        "benchmark.step.logs",
        "\u{6b63}\u{5728}\u{8bfb}\u{53d6}\u{63d0}\u{4ea4}\u{5386}\u{53f2}",
    ),
    (
        "benchmark.step.commit_details",
        "\u{6b63}\u{5728}\u{8bfb}\u{53d6}\u{63d0}\u{4ea4}\u{8be6}\u{60c5}",
    ),
    (
        "benchmark.step.file_status",
        "\u{6b63}\u{5728}\u{8bfb}\u{53d6}\u{6587}\u{4ef6}\u{72b6}\u{6001}",
    ),
    (
        "benchmark.step.remotes",
        "\u{6b63}\u{5728}\u{8bfb}\u{53d6}\u{8fdc}\u{7aef}\u{4ed3}\u{5e93}",
    ),
    (
        "benchmark.step.files",
        "\u{6b63}\u{5728}\u{7edf}\u{8ba1}\u{6587}\u{4ef6}\u{6570}",
    ),
    (
        "benchmark.step.system",
        "\u{6b63}\u{5728}\u{8bfb}\u{53d6}\u{7cfb}\u{7edf}\u{4fe1}\u{606f}",
    ),
    (
        "benchmark.save_title",
        "\u{4fdd}\u{5b58} Benchmark \u{6587}\u{4ef6}",
    ),
    (
        "benchmark.saved",
        "\u{57fa}\u{51c6}\u{6d4b}\u{8bd5}\u{5df2}\u{4fdd}\u{5b58}",
    ),
    (
        "benchmark.save_failed",
        "\u{4fdd}\u{5b58}\u{57fa}\u{51c6}\u{6d4b}\u{8bd5}\u{5931}\u{8d25}",
    ),
    (
        "benchmark.cancelled",
        "\u{5df2}\u{53d6}\u{6d88}\u{4fdd}\u{5b58}\u{57fa}\u{51c6}\u{6d4b}\u{8bd5}",
    ),
    (
        "benchmark.stopped",
        "\u{4ed3}\u{5e93}\u{57fa}\u{51c6}\u{6d4b}\u{8bd5}\u{5df2}\u{505c}\u{6b62}",
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
    (
        "branch.confirm_checkout_title",
        "\u{786e}\u{8ba4}\u{5206}\u{652f}\u{5207}\u{6362}",
    ),
    (
        "branch.confirm_checkout",
        "\u{786e}\u{5b9a}\u{5c06}\u{4f60}\u{7684}\u{5de5}\u{4f5c}\u{526f}\u{672c}\u{5207}\u{6362}\u{4e3a}",
    ),
    (
        "branch.discard_before_checkout",
        "\u{6e05}\u{9664}\u{ff08}\u{4e22}\u{5f03}\u{6240}\u{6709}\u{66f4}\u{6539}\u{ff09}",
    ),
    (
        "worktree.discard_all_confirm",
        "\u{4e22}\u{5f03}\u{6240}\u{6709}\u{672a}\u{63d0}\u{4ea4}\u{7684}\u{66f4}\u{6539}\u{ff1f}",
    ),
    (
        "worktree.discard_all_warning",
        "\u{8fd9}\u{4f1a}\u{91cd}\u{7f6e}\u{5df2}\u{8ddf}\u{8e2a}\u{66f4}\u{6539}\u{5e76}\u{5220}\u{9664}\u{672a}\u{8ddf}\u{8e2a}\u{6587}\u{4ef6}\u{3002}",
    ),
    (
        "worktree.discard_untracked_warning",
        "\u{8fd9}\u{4f1a}\u{5220}\u{9664}\u{672a}\u{8ddf}\u{8e2a}\u{7684}\u{6587}\u{4ef6}\u{6216}\u{76ee}\u{5f55}\u{3002}",
    ),
    (
        "worktree.discard_tracked_warning",
        "\u{8fd9}\u{4f1a}\u{4ece} HEAD \u{6062}\u{590d}\u{8be5}\u{8def}\u{5f84}\u{3002}",
    ),
    ("patch.create.title", "\u{521b}\u{5efa}\u{8865}\u{4e01}"),
    (
        "patch.create.worktree_tab",
        "\u{5de5}\u{4f5c}\u{526f}\u{672c}\u{7684}\u{53d8}\u{66f4}",
    ),
    (
        "patch.create.history_tab",
        "\u{65e5}\u{5fd7} / \u{5386}\u{53f2}",
    ),
    (
        "patch.create.output_path",
        "\u{8865}\u{4e01}\u{6587}\u{4ef6}",
    ),
    (
        "patch.create.empty_worktree",
        "\u{6ca1}\u{6709}\u{53ef}\u{521b}\u{5efa}\u{8865}\u{4e01}\u{7684}\u{5de5}\u{4f5c}\u{533a}\u{53d8}\u{66f4}\u{3002}",
    ),
    (
        "patch.create.history_empty",
        "\u{9009}\u{62e9}\u{63d0}\u{4ea4}\u{4ee5}\u{521b}\u{5efa}\u{8865}\u{4e01}\u{3002}",
    ),
    (
        "patch.create.changed_files",
        "\u{53d8}\u{66f4}\u{6587}\u{4ef6}",
    ),
    ("patch.create.browse", "\u{6d4f}\u{89c8}"),
    (
        "patch.create.separate_files",
        "\u{4e3a}\u{6bcf}\u{4e2a}\u{63d0}\u{4ea4}\u{521b}\u{5efa}\u{5355}\u{72ec}\u{7684}\u{8865}\u{4e01}\u{6587}\u{4ef6}",
    ),
    ("patch.create.submit", "\u{521b}\u{5efa}\u{8865}\u{4e01}"),
    (
        "patch.create.no_selection",
        "\u{8bf7}\u{81f3}\u{5c11}\u{9009}\u{62e9}\u{4e00}\u{9879}\u{3002}",
    ),
    (
        "patch.create.invalid_output",
        "\u{8bf7}\u{9009}\u{62e9}\u{6709}\u{6548}\u{7684}\u{8865}\u{4e01}\u{6587}\u{4ef6}\u{8def}\u{5f84}\u{3002}",
    ),
    (
        "patch.create.running",
        "\u{6b63}\u{5728}\u{521b}\u{5efa}\u{8865}\u{4e01}...",
    ),
    (
        "patch.create.success",
        "\u{5df2}\u{521b}\u{5efa}\u{8865}\u{4e01}\u{3002}",
    ),
    (
        "patch.create.disconnected",
        "\u{8865}\u{4e01}\u{4efb}\u{52a1}\u{610f}\u{5916}\u{505c}\u{6b62}\u{3002}",
    ),
    (
        "patch.create.overwrite_title",
        "\u{8986}\u{76d6}\u{5df2}\u{6709}\u{8865}\u{4e01}\u{ff1f}",
    ),
    (
        "patch.create.overwrite_message",
        "\u{4ee5}\u{4e0b}\u{6587}\u{4ef6}\u{5df2}\u{5b58}\u{5728}\u{3002}\u{7ee7}\u{7eed}\u{5c06}\u{8986}\u{76d6}\u{5b83}\u{4eec}\u{3002}",
    ),
    (
        "patch.create.overwrite_confirm",
        "\u{8986}\u{76d6}\u{5e76}\u{521b}\u{5efa}",
    ),
    (
        "patch.apply.failed_hint",
        "\u{65e0}\u{6cd5}\u{5c06}\u{8865}\u{4e01}\u{5e94}\u{7528}\u{5230}\u{5f53}\u{524d}\u{5206}\u{652f}\u{3002}\u{5f53}\u{524d}\u{5206}\u{652f}\u{53ef}\u{80fd}\u{4e0d}\u{5305}\u{542b}\u{521b}\u{5efa}\u{8865}\u{4e01}\u{65f6}\u{7684}\u{57fa}\u{7ebf}\u{6587}\u{4ef6}\u{6216}\u{5185}\u{5bb9}\u{3002}",
    ),
    (
        "stash.staged_files",
        "\u{5df2}\u{6682}\u{5b58}\u{6587}\u{4ef6} / \u{9009}\u{4e2d}\u{7684}\u{6587}\u{4ef6}",
    ),
    (
        "stash.keep_staged",
        "\u{4fdd}\u{7559}\u{6682}\u{5b58}\u{7684}\u{66f4}\u{6539}",
    ),
    (
        "stash.include_untracked",
        "\u{672a}\u{8ddf}\u{8e2a}\u{7684}\u{6587}\u{4ef6}",
    ),
    ("stash.include_ignored", "\u{6240}\u{6709}"),
    ("checkout.title", "\u{68c0}\u{51fa}"),
    (
        "checkout.existing",
        "\u{68c0}\u{51fa}\u{73b0}\u{6709}\u{7684}",
    ),
    (
        "checkout.new_branch",
        "\u{68c0}\u{51fa}\u{65b0}\u{5206}\u{652f}",
    ),
    (
        "checkout.existing_commit",
        "\u{9009}\u{62e9}\u{8981}\u{68c0}\u{51fa}\u{7684}\u{63d0}\u{4ea4}",
    ),
    (
        "checkout.remote_branch",
        "\u{68c0}\u{51fa}\u{8fdc}\u{7aef}\u{5206}\u{652f}:",
    ),
    (
        "checkout.local_branch",
        "\u{65b0}\u{7684}\u{672c}\u{5730}\u{5206}\u{652f}\u{540d}:",
    ),
    (
        "checkout.select_remote_branch",
        "\u{9009}\u{62e9}\u{8fdc}\u{7aef}\u{5206}\u{652f}",
    ),
    (
        "checkout.track_remote",
        "\u{672c}\u{5730}\u{5206}\u{652f}\u{5e94}\u{8ddf}\u{8e2a}\u{8fdc}\u{7aef}\u{5206}\u{652f}",
    ),
    (
        "checkout.detached_confirm_title",
        "\u{8b66}\u{544a}\u{ff1a}\u{6b63}\u{5728}\u{521b}\u{5efa}\u{5206}\u{79bb}\u{7684} HEAD",
    ),
    (
        "checkout.detached_target",
        "\u{68c0}\u{51fa}\u{63d0}\u{4ea4}",
    ),
    (
        "checkout.detached_warning_detail",
        "\u{68c0}\u{51fa}\u{8be5}\u{63d0}\u{4ea4}\u{4f1a}\u{521b}\u{5efa}\u{4e00}\u{4e2a}\u{5206}\u{79bb}\u{7684} HEAD\u{ff0c}\u{4f60}\u{5c06}\u{4e0d}\u{5728}\u{4efb}\u{4f55}\u{5206}\u{652f}\u{4e0a}\u{3002}\u{53ef}\u{7528}\u{4e8e}\u{4e34}\u{65f6} commit \u{9a8c}\u{8bc1}\u{ff1b}\u{82e5}\u{9700}\u{8981}\u{957f}\u{671f}\u{4fdd}\u{5b58}\u{ff0c}\u{8bf7}\u{4ece}\u{68c0}\u{51fa}\u{65b0}\u{5206}\u{652f}\u{521b}\u{5efa}\u{672c}\u{5730}\u{5206}\u{652f}\u{3002}",
    ),
    (
        "checkout.error.fix_inputs",
        "\u{8bf7}\u{7ea0}\u{6b63}\u{4ee5}\u{4e0b}\u{8f93}\u{5165}\u{9519}\u{8bef}:",
    ),
    (
        "checkout.error.local_branch_invalid",
        "\u{672c}\u{5730}\u{5206}\u{652f}\u{540d}\u{65e0}\u{6548}",
    ),
    (
        "checkout.error.remote_branch_invalid",
        "\u{9009}\u{4e2d}\u{7684}\u{8fdc}\u{7aef}\u{5206}\u{652f}\u{540d}\u{65e0}\u{6548}",
    ),
    (
        "checkout.error.local_branch_exists",
        "\u{672c}\u{5730}\u{5206}\u{652f}\u{5df2}\u{5b58}\u{5728}",
    ),
    (
        "interactive_rebase.title",
        "\u{4ea4}\u{4e92}\u{5f0f}\u{53d8}\u{57fa}",
    ),
    (
        "interactive_rebase.select_commit",
        "\u{9009}\u{62e9}\u{8981}\u{53d8}\u{57fa}\u{7684}\u{63d0}\u{4ea4}",
    ),
    (
        "interactive_rebase.selected_commit",
        "\u{5df2}\u{9009}\u{63d0}\u{4ea4}:",
    ),
    (
        "interactive_rebase.base_commit",
        "\u{57fa}\u{70b9}\u{63d0}\u{4ea4}:",
    ),
    (
        "interactive_rebase.published_warning",
        "\u{6b64}\u{64cd}\u{4f5c}\u{4f1a}\u{91cd}\u{5199}\u{5df2}\u{7ecf}\u{63a8}\u{9001}\u{5230}\u{8fdc}\u{7aef}\u{7684}\u{5386}\u{53f2}\u{3002}\u{5b8c}\u{6210}\u{540e}\u{9700}\u{8981} force-with-lease \u{63a8}\u{9001}\u{ff0c}\u{5176}\u{4ed6}\u{4eba}\u{82e5}\u{62c9}\u{8fc7}\u{6b64}\u{5206}\u{652f}\u{4f1a}\u{53d7}\u{5f71}\u{54cd}\u{3002}",
    ),
    (
        "interactive_rebase.confirm_published",
        "\u{6211}\u{786e}\u{8ba4}\u{8981}\u{91cd}\u{5199}\u{5df2}\u{63a8}\u{9001}\u{5386}\u{53f2}",
    ),
    ("interactive_rebase.todo_action", "\u{52a8}\u{4f5c}:"),
    ("interactive_rebase.todo.pick", "\u{4fdd}\u{7559}"),
    (
        "interactive_rebase.todo.reword",
        "\u{6539}\u{5199}\u{4fe1}\u{606f}",
    ),
    (
        "interactive_rebase.todo.edit",
        "\u{7f16}\u{8f91}\u{63d0}\u{4ea4}",
    ),
    (
        "interactive_rebase.todo.none",
        "\u{9009}\u{62e9}\u{52a8}\u{4f5c}",
    ),
    (
        "interactive_rebase.todo.squash",
        "\u{5408}\u{5e76}\u{5230}\u{4e0a}\u{4e00}\u{4e2a}",
    ),
    (
        "interactive_rebase.todo.fixup",
        "\u{5408}\u{5e76}\u{4e14}\u{4e22}\u{5f03}\u{63d0}\u{4ea4}\u{4fe1}\u{606f}",
    ),
    (
        "interactive_rebase.autosquash_target",
        "\u{81ea}\u{52a8}\u{76ee}\u{6807}\u{ff1a}",
    ),
    ("interactive_rebase.todo.drop", "\u{5220}\u{9664}"),
    (
        "interactive_rebase.reword.commit",
        "\u{6539}\u{5199}\u{63d0}\u{4ea4}:",
    ),
    (
        "interactive_rebase.reword.title",
        "\u{6539}\u{5199}\u{63d0}\u{4ea4}\u{4fe1}\u{606f}",
    ),
    (
        "interactive_rebase.reword.hint",
        "\u{8bf7}\u{8f93}\u{5165}\u{63d0}\u{4ea4}\u{4fe1}\u{606f}",
    ),
    ("interactive_rebase.reword.author", "\u{4f5c}\u{8005}"),
    ("interactive_rebase.reword.email", "\u{90ae}\u{7bb1}"),
    (
        "interactive_rebase.reword.author_hint",
        "\u{8bf7}\u{8f93}\u{5165}\u{4f5c}\u{8005}\u{540d}",
    ),
    (
        "interactive_rebase.reword.email_hint",
        "\u{8bf7}\u{8f93}\u{5165}\u{90ae}\u{7bb1}",
    ),
    (
        "interactive_rebase.reword.edit",
        "\u{7f16}\u{8f91}\u{6539}\u{5199}\u{4fe1}\u{606f}",
    ),
    (
        "interactive_rebase.reword.done",
        "\u{4fdd}\u{5b58}\u{6539}\u{5199}",
    ),
    (
        "interactive_rebase.reword.cancel",
        "\u{53d6}\u{6d88}\u{6539}\u{5199}",
    ),
    (
        "interactive_rebase.plan",
        "\u{53d8}\u{57fa}\u{8ba1}\u{5212}",
    ),
    (
        "interactive_rebase.plan_hint",
        "\u{62d6}\u{52a8} :: \u{8c03}\u{6574}\u{63d0}\u{4ea4}\u{987a}\u{5e8f}",
    ),
    (
        "interactive_rebase.preview",
        "\u{8ba1}\u{5212}\u{9884}\u{89c8}",
    ),
    (
        "interactive_rebase.plan_position",
        "\u{8ba1}\u{5212}\u{987a}\u{5e8f}",
    ),
    (
        "interactive_rebase.preview_target",
        "\u{5408}\u{5e76}\u{76ee}\u{6807}",
    ),
    (
        "interactive_rebase.preview_previous",
        "\u{524d}\u{4e00}\u{6761}",
    ),
    (
        "interactive_rebase.preview_next",
        "\u{540e}\u{4e00}\u{6761}",
    ),
    (
        "interactive_rebase.preview_effect.pick",
        "\u{5c06}\u{4fdd}\u{7559}\u{5e76}\u{91cd}\u{653e}\u{6b64}\u{63d0}\u{4ea4}",
    ),
    (
        "interactive_rebase.preview_effect.reword",
        "\u{5c06}\u{6539}\u{5199}\u{6b64}\u{63d0}\u{4ea4}\u{4fe1}\u{606f}",
    ),
    (
        "interactive_rebase.preview_effect.edit",
        "\u{5e94}\u{7528}\u{6b64}\u{63d0}\u{4ea4}\u{540e}\u{6682}\u{505c}",
    ),
    (
        "interactive_rebase.preview_effect.squash",
        "\u{5c06}\u{5408}\u{5e76}\u{5230}\u{76ee}\u{6807}\u{5e76}\u{4fdd}\u{7559}\u{63d0}\u{4ea4}\u{4fe1}\u{606f}",
    ),
    (
        "interactive_rebase.preview_effect.fixup",
        "\u{5c06}\u{5408}\u{5e76}\u{5230}\u{76ee}\u{6807}\u{5e76}\u{4e22}\u{5f03}\u{6b64}\u{63d0}\u{4ea4}\u{4fe1}\u{606f}",
    ),
    (
        "interactive_rebase.preview_effect.drop",
        "\u{5c06}\u{4e22}\u{5f03}\u{6b64}\u{63d0}\u{4ea4}\u{53ca}\u{5176}\u{53d8}\u{66f4}",
    ),
    (
        "interactive_rebase.graph_drag_hint",
        "\u{62d6}\u{52a8}\u{63d0}\u{4ea4}\u{5230}\u{7956}\u{5148}\u{63d0}\u{4ea4}\u{ff0c}\u{6253}\u{5f00}\u{53d8}\u{57fa}\u{8ba1}\u{5212}",
    ),
    (
        "interactive_rebase.graph_drag_started",
        "\u{5df2}\u{6253}\u{5f00}\u{4ea4}\u{4e92}\u{5f0f}\u{53d8}\u{57fa}\u{8ba1}\u{5212}",
    ),
    (
        "interactive_rebase.graph_drag_invalid",
        "\u{53ea}\u{80fd}\u{5c06}\u{975e}\u{5408}\u{5e76}\u{63d0}\u{4ea4}\u{62d6}\u{5230}\u{5176}\u{7956}\u{5148}\u{63d0}\u{4ea4}\u{4e0a}",
    ),
    ("interactive_rebase.move_up", "\u{4e0a}\u{79fb}"),
    ("interactive_rebase.move_down", "\u{4e0b}\u{79fb}"),
    ("interactive_rebase.reset", "\u{91cd}\u{7f6e}"),
    ("interactive_rebase.selected_count", "\u{5df2}\u{9009}"),
    (
        "interactive_rebase.drop_selected",
        "\u{5220}\u{9664}\u{9009}\u{4e2d}",
    ),
    (
        "interactive_rebase.squash_selected",
        "\u{5408}\u{5e76}\u{9009}\u{4e2d}\u{5230}\u{4e0a}\u{4e00}\u{4e2a}",
    ),
    (
        "interactive_rebase.squash_target",
        "\u{5408}\u{5e76}\u{76ee}\u{6807}:",
    ),
    (
        "interactive_rebase.squash_to_target",
        "\u{5408}\u{5e76}\u{9009}\u{4e2d}\u{5230}\u{76ee}\u{6807}",
    ),
    (
        "interactive_rebase.squash_to_target_applied",
        "\u{5408}\u{5e76}\u{9009}\u{4e2d}\u{5230}",
    ),
    (
        "interactive_rebase.reset_selected",
        "\u{91cd}\u{7f6e}\u{9009}\u{4e2d}",
    ),
    (
        "interactive_rebase.merge_commit_disabled",
        "\u{5408}\u{5e76}\u{63d0}\u{4ea4}\u{6682}\u{4e0d}\u{652f}\u{6301}\u{4ea4}\u{4e92}\u{5f0f}\u{53d8}\u{57fa}\u{64cd}\u{4f5c}",
    ),
    (
        "interactive_rebase.squash_previous",
        "\u{7528}\u{6b64}\u{524d}\u{7684} squash",
    ),
    ("interactive_rebase.drop_commit", "\u{5220}\u{9664}"),
    (
        "interactive_rebase.error.dirty",
        "git rebase -i --autosquash\nerror: cannot rebase: You have unstaged changes.\nerror: Please commit or stash them.",
    ),
    (
        "interactive_rebase.error.index_dirty",
        "git rebase -i --autosquash\nerror: cannot rebase: Your index contains uncommitted changes.\nerror: Please commit or stash them.",
    ),
    (
        "interactive_rebase.error.in_progress",
        "\u{5f53}\u{524d}\u{4ed3}\u{5e93}\u{5df2}\u{6709}\u{672a}\u{5b8c}\u{6210}\u{7684} rebase\u{3002}\n\u{8bf7}\u{5148}\u{5904}\u{7406}\u{5f53}\u{524d} rebase\u{ff1a}git rebase --continue / --abort / --skip\u{ff0c}\u{7136}\u{540e}\u{518d}\u{91cd}\u{65b0}\u{6253}\u{5f00}\u{4ea4}\u{4e92}\u{5f0f}\u{53d8}\u{57fa}\u{3002}",
    ),
    (
        "interactive_rebase.error.detached",
        "\u{5f53}\u{524d}\u{5904}\u{4e8e}\u{5206}\u{79bb} HEAD\u{ff0c}\u{8bf7}\u{5148}\u{68c0}\u{51fa}\u{6216}\u{521b}\u{5efa}\u{672c}\u{5730}\u{5206}\u{652f}\u{540e}\u{518d}\u{4ea4}\u{4e92}\u{5f0f}\u{53d8}\u{57fa}\u{3002}",
    ),
    (
        "interactive_rebase.error.no_commits",
        "\u{5f53}\u{524d}\u{5206}\u{652f}\u{6ca1}\u{6709}\u{53ef}\u{7528}\u{4e8e}\u{4ea4}\u{4e92}\u{5f0f}\u{53d8}\u{57fa}\u{7684}\u{63d0}\u{4ea4}",
    ),
    (
        "interactive_rebase.error.no_children",
        "\u{6240}\u{9009}\u{63d0}\u{4ea4}\u{4e4b}\u{540e}\u{6ca1}\u{6709}\u{53ef}\u{7528}\u{4e8e}\u{4ea4}\u{4e92}\u{5f0f}\u{53d8}\u{57fa}\u{7684}\u{63d0}\u{4ea4}",
    ),
    (
        "interactive_rebase.error.confirm_published",
        "\u{8bf7}\u{5148}\u{786e}\u{8ba4}\u{91cd}\u{5199}\u{5df2}\u{63a8}\u{9001}\u{5386}\u{53f2}",
    ),
    (
        "interactive_rebase.error.first_squash",
        "\u{6700}\u{65e9}\u{7684}\u{63d0}\u{4ea4}\u{4e0d}\u{80fd} squash",
    ),
    (
        "interactive_rebase.error.no_changes",
        "\u{8bf7}\u{5148}\u{9009}\u{62e9}\u{8981}\u{6267}\u{884c}\u{7684}\u{53d8}\u{57fa}\u{52a8}\u{4f5c}",
    ),
    (
        "interactive_rebase.error.reword_unconfirmed",
        "\u{8bf7}\u{5148}\u{4fdd}\u{5b58}\u{6539}\u{5199}\u{4fe1}\u{606f}",
    ),
    (
        "interactive_rebase.in_progress.title",
        "\u{53d8}\u{57fa}\u{8fdb}\u{884c}\u{4e2d}",
    ),
    (
        "interactive_rebase.in_progress.detail",
        "\u{5f53}\u{524d}\u{4ed3}\u{5e93}\u{6b63}\u{5728}\u{5904}\u{7406} git rebase\u{3002}\u{89e3}\u{51b3}\u{51b2}\u{7a81}\u{540e}\u{53ef}\u{7ee7}\u{7eed}\u{ff0c}\u{4e5f}\u{53ef}\u{8df3}\u{8fc7}\u{5f53}\u{524d}\u{63d0}\u{4ea4}\u{6216}\u{4e2d}\u{6b62}\u{53d8}\u{57fa}\u{3002}",
    ),
    (
        "interactive_rebase.in_progress.conflicts",
        "\u{5f53}\u{524d}\u{6709}\u{51b2}\u{7a81}\u{6587}\u{4ef6}\u{ff0c}\u{9700}\u{5148}\u{89e3}\u{51b2}\u{5e76}\u{6682}\u{5b58}\u{3002}",
    ),
    (
        "interactive_rebase.in_progress.ready",
        "\u{51b2}\u{7a81}\u{5df2}\u{89e3}\u{51b3}\u{ff0c}\u{53ef}\u{4ee5}\u{7ee7}\u{7eed}\u{53d8}\u{57fa}\u{3002}",
    ),
    (
        "interactive_rebase.in_progress.edit_ready",
        "\u{5df2}\u{6682}\u{505c}\u{5728}\u{7f16}\u{8f91}\u{6b65}\u{9aa4}\u{3002}\u{6682}\u{5b58}\u{6539}\u{52a8}\u{540e}\u{4fee}\u{8ba2}\u{5f53}\u{524d}\u{63d0}\u{4ea4}\u{ff0c}\u{518d}\u{7ee7}\u{7eed}\u{53d8}\u{57fa}\u{3002}",
    ),
    (
        "interactive_rebase.in_progress.continue",
        "\u{7ee7}\u{7eed}",
    ),
    (
        "interactive_rebase.in_progress.skip",
        "\u{8df3}\u{8fc7}\u{5f53}\u{524d}\u{63d0}\u{4ea4}",
    ),
    (
        "interactive_rebase.in_progress.abort",
        "\u{4e2d}\u{6b62}\u{53d8}\u{57fa}",
    ),
    (
        "interactive_rebase.in_progress.amend",
        "\u{4fee}\u{8ba2}\u{5f53}\u{524d}\u{63d0}\u{4ea4}",
    ),
    (
        "submodule.title",
        "\u{6dfb}\u{52a0}\u{5b50}\u{6a21}\u{5757}...",
    ),
    ("submodule.source", "\u{6e90}\u{8def}\u{5f84} / URL:"),
    ("submodule.repo_type", "\u{4ed3}\u{5e93}\u{7c7b}\u{578b}:"),
    (
        "submodule.local_path",
        "\u{672c}\u{5730}\u{76f8}\u{5173}\u{8def}\u{5f84}:",
    ),
    ("submodule.source_branch", "\u{6e90}\u{5206}\u{652f}:"),
    (
        "submodule.recursive",
        "\u{9012}\u{5f52}\u{5b50}\u{6a21}\u{5757}",
    ),
    (
        "subtree.title",
        "\u{6dfb}\u{52a0}/\u{94fe}\u{63a5}\u{5b50}\u{6811}",
    ),
    ("subtree.source", "\u{6e90}\u{8def}\u{5f84} / URL:"),
    ("subtree.repo_type", "\u{4ed3}\u{5e93}\u{7c7b}\u{578b}:"),
    ("subtree.ref_name", "\u{5206}\u{652f} / \u{63d0}\u{4ea4}:"),
    (
        "subtree.local_path",
        "\u{672c}\u{5730}\u{76f8}\u{5173}\u{8def}\u{5f84}:",
    ),
    ("subtree.squash", "squash \u{63d0}\u{4ea4}?"),
    (
        "subtree.error.ref_required",
        "\u{5206}\u{652f}/\u{63d0}\u{4ea4}\u{4e0d}\u{80fd}\u{4e3a}\u{7a7a}",
    ),
    (
        "dependency.advanced_options",
        "\u{9ad8}\u{7ea7}\u{9009}\u{9879}",
    ),
    (
        "dependency.repo_type_missing",
        "\u{672a}\u{63d0}\u{4f9b}\u{8def}\u{5f84}\u{6216} URL",
    ),
    ("dependency.repo_type_git", "Git \u{4ed3}\u{5e93}"),
    (
        "dependency.repo_type_local",
        "\u{672c}\u{5730}\u{8def}\u{5f84}",
    ),
    (
        "dependency.error.source_required",
        "\u{672a}\u{63d0}\u{4f9b}\u{8def}\u{5f84}\u{6216} URL",
    ),
    (
        "dependency.error.local_path_required",
        "\u{672c}\u{5730}\u{76f8}\u{5bf9}\u{8def}\u{5f84}\u{4e0d}\u{80fd}\u{4e3a}\u{7a7a}",
    ),
    (
        "dependency.error.local_path_relative",
        "\u{672c}\u{5730}\u{8def}\u{5f84}\u{5fc5}\u{987b}\u{662f}\u{4ed3}\u{5e93}\u{5185}\u{76f8}\u{5bf9}\u{8def}\u{5f84}",
    ),
    (
        "lfs.init.title",
        "\u{4e3a}\u{4ed3}\u{5e93}\u{521d}\u{59cb}\u{5316} Git LFS",
    ),
    ("lfs.intro.heading", "Git LFS"),
    (
        "lfs.intro.body",
        "Git LFS \u{4f1a}\u{628a}\u{89c6}\u{9891}\u{3001}\u{8bbe}\u{8ba1}\u{56fe}\u{3001}\u{6e38}\u{620f}\u{8d44}\u{6e90}\u{3001}\u{4e8c}\u{8fdb}\u{5236}\u{5305}\u{7b49}\u{5927}\u{6587}\u{4ef6}\u{4fdd}\u{5b58}\u{5230}\u{5927}\u{6587}\u{4ef6}\u{5b58}\u{50a8}\u{ff0c}\u{4ed3}\u{5e93}\u{91cc}\u{53ea}\u{4fdd}\u{7559}\u{5f88}\u{5c0f}\u{7684}\u{6307}\u{9488}\u{6587}\u{4ef6}\u{3002}\u{8fd9}\u{6837}\u{53ef}\u{4ee5}\u{51cf}\u{5c11}\u{4ed3}\u{5e93}\u{4f53}\u{79ef}\u{548c}\u{62c9}\u{53d6}\u{3001}\u{63a8}\u{9001}\u{7684}\u{5361}\u{987f}\u{3002}",
    ),
    (
        "lfs.intro.note",
        "\u{5f00}\u{59cb}\u{540e}\u{9700}\u{8981}\u{9009}\u{62e9}\u{8981}\u{7531} Git LFS \u{8ddf}\u{8e2a}\u{7684}\u{6587}\u{4ef6}\u{7c7b}\u{578b}\u{ff0c}\u{4f8b}\u{5982} *.psd\u{3001}*.mp4 \u{6216} *.zip\u{3002}",
    ),
    ("lfs.start", "\u{5f00}\u{59cb}\u{4f7f}\u{7528} Git LFS"),
    (
        "lfs.track.title",
        "Git LFS: \u{9009}\u{62e9}\u{8ddf}\u{8e2a}\u{7684}\u{6587}\u{4ef6}",
    ),
    (
        "lfs.patterns_label",
        "\u{5728} Git LFS \u{4e2d}\u{88ab}\u{8ffd}\u{8e2a}\u{7684}\u{6587}\u{4ef6}\u{7c7b}\u{578b}",
    ),
    (
        "lfs.pattern_help",
        "\u{4f60}\u{53ef}\u{4ee5}\u{7a0d}\u{540e}\u{518d}\u{4ece}\u{2018}\u{4ed3}\u{5e93} > Git LFS\u{2019}\u{4e0b}\u{7684}\u{83dc}\u{5355}\u{52a0}\u{5165}\u{8be5}\u{5217}\u{8868}\u{3002}",
    ),
    ("lfs.add", "\u{6dfb}\u{52a0}"),
    ("lfs.remove", "\u{79fb}\u{9664}"),
    ("lfs.track_files", "\u{8ddf}\u{8e2a}\u{6587}\u{4ef6}"),
    (
        "lfs.pattern_empty",
        "\u{5c1a}\u{672a}\u{6dfb}\u{52a0}\u{8ddf}\u{8e2a}\u{7c7b}\u{578b}",
    ),
    ("lfs.pattern_placeholder", "\u{4f8b}\u{5982} *.psd"),
    (
        "lfs.error.pattern_required",
        "\u{8ddf}\u{8e2a}\u{7c7b}\u{578b}\u{4e0d}\u{80fd}\u{4e3a}\u{7a7a}",
    ),
    (
        "lfs.error.duplicate_pattern",
        "\u{8ddf}\u{8e2a}\u{7c7b}\u{578b}\u{4e0d}\u{80fd}\u{91cd}\u{590d}",
    ),
    (
        "merge.title",
        "\u{9009}\u{62e9}\u{4e00}\u{4e2a}\u{63d0}\u{4ea4}\u{5408}\u{5e76}\u{5230}\u{5f53}\u{524d}\u{5206}\u{652f}",
    ),
    (
        "merge.select_commit",
        "\u{9009}\u{62e9}\u{8981}\u{5408}\u{5e76}\u{7684}\u{63d0}\u{4ea4}",
    ),
    ("merge.selected_commit", "\u{5df2}\u{9009}\u{63d0}\u{4ea4}:"),
    (
        "merge.commit_immediately",
        "\u{7acb}\u{5373}\u{63d0}\u{4ea4}\u{5408}\u{5e76}\u{ff08}\u{5982}\u{679c}\u{6ca1}\u{6709}\u{51b2}\u{7a81}\u{ff09}",
    ),
    (
        "merge.include_messages",
        "\u{5305}\u{62ec}\u{88ab}\u{5408}\u{5e76}\u{63d0}\u{4ea4}\u{7684}\u{4fe1}\u{606f}\u{5185}\u{5bb9}",
    ),
    (
        "merge.force_merge_commit",
        "\u{65e0}\u{8bba}\u{5feb}\u{8fdb}\u{66f4}\u{65b0}\u{662f}\u{5426}\u{53ef}\u{4ee5}\u{88ab}\u{6267}\u{884c}\u{90fd}\u{521b}\u{5efa}\u{4e00}\u{4e2a}\u{65b0}\u{7684}\u{63d0}\u{4ea4}",
    ),
    (
        "merge.rebase",
        "\u{7528}\u{53d8}\u{57fa}\u{4ee3}\u{66ff}\u{5408}\u{5e76}\u{ff08}\u{8b66}\u{544a}\u{ff1a}\u{8bf7}\u{786e}\u{4fdd}\u{4f60}\u{8fd8}\u{6ca1}\u{6709}\u{63a8}\u{9001}\u{60a8}\u{7684}\u{53d8}\u{66f4}\u{ff09}",
    ),
    (
        "merge.detect_renames",
        "\u{68c0}\u{6d4b}\u{76f8}\u{4f3c}\u{7684}\u{91cd}\u{547d}\u{540d}",
    ),
    ("archive.title", "\u{5b58}\u{6863}"),
    ("archive.output_path", "\u{5b58}\u{6863}\u{6587}\u{4ef6}:"),
    (
        "archive.folder_prefix",
        "\u{6587}\u{4ef6}\u{5939}\u{524d}\u{7f00}:",
    ),
    ("archive.target", "\u{63d0}\u{4ea4}:"),
    (
        "archive.worktree",
        "\u{5de5}\u{4f5c}\u{526f}\u{672c}\u{7248}\u{672c}",
    ),
    (
        "archive.commit",
        "\u{6307}\u{5b9a}\u{7684}\u{63d0}\u{4ea4}:",
    ),
    (
        "archive.error.output_required",
        "\u{5b58}\u{6863}\u{6587}\u{4ef6}\u{4e0d}\u{80fd}\u{4e3a}\u{7a7a}",
    ),
    (
        "archive.error.commit_required",
        "\u{6307}\u{5b9a}\u{7684}\u{63d0}\u{4ea4}\u{4e0d}\u{80fd}\u{4e3a}\u{7a7a}",
    ),
    (
        "branch.pull_tracked",
        "\u{62c9}\u{53d6}\u{ff08}\u{5df2}\u{8ddf}\u{8e2a}\u{ff09}",
    ),
    (
        "branch.push_tracked",
        "\u{63a8}\u{9001}\u{5230}\u{ff08}\u{5df2}\u{8ddf}\u{8e2a}\u{ff09}",
    ),
    ("branch.push_to", "\u{63a8}\u{9001}\u{5230}"),
    (
        "branch.track_remote",
        "\u{8ddf}\u{8e2a}\u{8fdc}\u{7a0b}\u{5206}\u{652f}",
    ),
    ("branch.no_remote_tracking", "\u{ff08}\u{65e0}\u{ff09}"),
    ("branch.no_remotes", "\u{65e0}\u{8fdc}\u{7aef}"),
    (
        "branch.compare_with_current",
        "\u{4e0e}\u{5f53}\u{524d}\u{5bf9}\u{6bd4}",
    ),
    (
        "branch.rename_title",
        "\u{91cd}\u{547d}\u{540d}\u{5206}\u{652f}",
    ),
    ("branch.new_name", "\u{65b0}\u{540d}\u{79f0}"),
    (
        "branch.create_pull_request",
        "\u{521b}\u{5efa}\u{62c9}\u{53d6}\u{8bf7}\u{6c42}...",
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
        "settings.repository_workspaces",
        "\u{5de5}\u{4f5c}\u{7a7a}\u{95f4}",
    ),
    (
        "settings.repository_workspaces_hint",
        "\u{672c}\u{5730}\u{4ed3}\u{5e93}\u{5217}\u{8868}\u{4f1a}\u{626b}\u{63cf}\u{8fd9}\u{4e9b}\u{6587}\u{4ef6}\u{5939}\u{7684}\u{4e0b}\u{4e00}\u{7ea7}\u{3002}",
    ),
    (
        "settings.repository_workspaces_default",
        "\u{9ed8}\u{8ba4}\u{ff1a}",
    ),
    (
        "settings.repository_workspaces_empty",
        "\u{672a}\u{914d}\u{7f6e}\u{5de5}\u{4f5c}\u{7a7a}\u{95f4}",
    ),
    (
        "settings.repository_workspace_add",
        "\u{6dfb}\u{52a0}\u{5de5}\u{4f5c}\u{7a7a}\u{95f4}",
    ),
    ("settings.auto_refresh", "\u{81ea}\u{52a8}\u{5237}\u{65b0}"),
    (
        "settings.refresh_active_repo_seconds",
        "\u{5f53}\u{524d}\u{4ed3}\u{5e93}\u{ff08}\u{79d2}\u{ff09}",
    ),
    (
        "settings.refresh_inactive_repo_seconds",
        "\u{7a97}\u{53e3}\u{5176}\u{4ed6}\u{4ed3}\u{5e93}\u{ff08}\u{79d2}\u{ff09}",
    ),
    (
        "settings.tab_empty",
        "\u{6b64}\u{5206}\u{7c7b}\u{6682}\u{65e0}\u{914d}\u{7f6e}\u{3002}",
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
    ("pull.title", "\u{62c9}\u{53d6}"),
    ("pull.remote", "\u{4ece}\u{8fdc}\u{7aef}\u{62c9}\u{53d6}"),
    (
        "pull.remote_branch",
        "\u{8981}\u{62c9}\u{53d6}\u{7684}\u{8fdc}\u{7aef}\u{5206}\u{652f}",
    ),
    (
        "pull.local_branch",
        "\u{62c9}\u{53d6}\u{5230}\u{672c}\u{5730}\u{7684}\u{5206}\u{652f}",
    ),
    ("pull.options", "\u{9009}\u{9879}"),
    (
        "pull.commit_merge",
        "\u{7acb}\u{5373}\u{63d0}\u{4ea4}\u{5408}\u{5e76}\u{7684}\u{6539}\u{52a8}",
    ),
    (
        "pull.include_tags",
        "\u{5305}\u{62ec}\u{88ab}\u{5408}\u{5e76}\u{63d0}\u{4ea4}\u{7684}\u{6807}\u{7b7e}\u{5185}\u{5bb9}",
    ),
    (
        "pull.force_merge_commit",
        "\u{65e0}\u{8bba}\u{5feb}\u{8fdb}\u{66f4}\u{65b0}\u{662f}\u{5426}\u{53ef}\u{4ee5}\u{88ab}\u{6267}\u{884c}\u{90fd}\u{521b}\u{5efa}\u{4e00}\u{4e2a}\u{65b0}\u{7684}\u{63d0}\u{4ea4}",
    ),
    (
        "pull.rebase",
        "\u{7528}\u{53d8}\u{57fa}\u{4ee3}\u{66ff}\u{5408}\u{5e76}",
    ),
    ("pull.refresh", "\u{5237}\u{65b0}"),
    ("fetch.title", "\u{83b7}\u{53d6}"),
    (
        "fetch.all_remotes",
        "\u{4ece}\u{5168}\u{90e8}\u{8fdc}\u{7aef}\u{83b7}\u{53d6}\u{66f4}\u{65b0}",
    ),
    (
        "fetch.prune_tracking",
        "\u{5220}\u{9664}\u{6240}\u{6709}\u{8fdc}\u{7aef}\u{73b0}\u{5df2}\u{4e0d}\u{5b58}\u{5728}\u{7684}\u{8ddf}\u{8e2a}\u{5206}\u{652f} (tracking)",
    ),
    (
        "fetch.tags",
        "\u{83b7}\u{53d6}\u{6240}\u{6709}\u{6807}\u{7b7e}",
    ),
    (
        "fetch.force_tags",
        "\u{8986}\u{76d6}\u{672c}\u{5730}\u{6807}\u{7b7e} (--force)",
    ),
    ("push.title", "\u{63a8}\u{9001}"),
    ("push.remote", "\u{63a8}\u{9001}\u{5230}\u{4ed3}\u{5e93}"),
    (
        "push.branches",
        "\u{8981}\u{63a8}\u{9001}\u{7684}\u{5206}\u{652f}",
    ),
    ("push.select", "\u{662f}\u{5426}\u{63a8}\u{9001}"),
    ("push.local_branch", "\u{672c}\u{5730}\u{5206}\u{652f}"),
    ("push.remote_branch", "\u{8fdc}\u{7aef}\u{5206}\u{652f}"),
    ("push.track", "\u{8ddf}\u{8e2a}?"),
    ("push.select_all", "\u{9009}\u{4e2d}\u{6240}\u{6709}"),
    (
        "push.push_tags",
        "\u{63a8}\u{9001}\u{6240}\u{6709}\u{6807}\u{7b7e}",
    ),
    (
        "push.force",
        "\u{5b89}\u{5168}\u{5f3a}\u{5236}\u{63a8}\u{9001} (--force-with-lease)",
    ),
    (
        "push.detached_error",
        "\u{5f53}\u{524d}\u{5904}\u{4e8e}\u{5206}\u{79bb} HEAD\u{ff0c}\u{65e0}\u{6cd5}\u{63a8}\u{9001}\u{5230}\u{8fdc}\u{7aef}\u{5206}\u{652f}\u{3002}\u{8bf7}\u{5148}\u{68c0}\u{51fa}\u{6216}\u{521b}\u{5efa}\u{672c}\u{5730}\u{5206}\u{652f}\u{3002}",
    ),
    (
        "push.force_confirm.title",
        "\u{786e}\u{8ba4}\u{5b89}\u{5168}\u{5f3a}\u{63a8}",
    ),
    (
        "push.force_confirm.message",
        "\u{8fd9}\u{4f1a}\u{4f7f}\u{7528} --force-with-lease \u{8986}\u{76d6}\u{8fdc}\u{7aef}\u{5206}\u{652f}\u{3002}\u{8bf7}\u{786e}\u{8ba4}\u{8fdc}\u{7aef}\u{6ca1}\u{6709}\u{522b}\u{4eba}\u{65b0}\u{7684}\u{63d0}\u{4ea4}\u{3002}",
    ),
    (
        "push.force_confirm.submit",
        "\u{786e}\u{8ba4}\u{5b89}\u{5168}\u{5f3a}\u{63a8}",
    ),
    (
        "rewrite_prompt.title",
        "\u{5386}\u{53f2}\u{5df2}\u{91cd}\u{5199}",
    ),
    (
        "rewrite_prompt.message",
        "\u{4ea4}\u{4e92}\u{5f0f}\u{53d8}\u{57fa}\u{5df2}\u{5b8c}\u{6210}\u{ff0c}\u{5f53}\u{524d}\u{5206}\u{652f}\u{4e0e}\u{8fdc}\u{7aef}\u{5386}\u{53f2}\u{5df2}\u{7ecf}\u{5206}\u{53c9}\u{3002}\u{8bf7}\u{6253}\u{5f00}\u{63a8}\u{9001}\u{9009}\u{9879}\u{ff0c}\u{786e}\u{8ba4}\u{540e}\u{4f7f}\u{7528} --force-with-lease \u{5b89}\u{5168}\u{8986}\u{76d6}\u{8fdc}\u{7aef}\u{3002}",
    ),
    (
        "rewrite_prompt.open_push",
        "\u{6253}\u{5f00}\u{63a8}\u{9001}\u{9009}\u{9879}",
    ),
    ("rewrite_prompt.later", "\u{7a0d}\u{540e}\u{5904}\u{7406}"),
    ("credentials.github_login", "\u{767b}\u{5f55} GitHub"),
    (
        "credentials.github_login_running",
        "\u{6b63}\u{5728}\u{767b}\u{5f55} GitHub...",
    ),
    (
        "credentials.github_login_done",
        "GitHub \u{767b}\u{5f55}\u{5df2}\u{5b8c}\u{6210}",
    ),
    (
        "credentials.github_login_done_message",
        "GitHub \u{767b}\u{5f55}\u{5df2}\u{5b8c}\u{6210}\u{3002}\u{5982}\u{679c}\u{6ca1}\u{6709}\u{53ef}\u{81ea}\u{52a8}\u{91cd}\u{8bd5}\u{7684} Git \u{64cd}\u{4f5c}\u{ff0c}\u{8bf7}\u{91cd}\u{65b0}\u{6253}\u{5f00}\u{63a8}\u{9001}\u{5e76}\u{518d}\u{6b21}\u{786e}\u{8ba4}\u{3002}",
    ),
    (
        "credentials.github_retry_failed",
        "\u{767b}\u{5f55}\u{540e}\u{81ea}\u{52a8}\u{91cd}\u{8bd5}\u{4ecd}\u{7136}\u{5931}\u{8d25}\u{3002}\u{8bf7}\u{68c0}\u{67e5}\u{5f53}\u{524d} HTTPS \u{51ed}\u{636e}\u{3001}GitHub \u{8d26}\u{53f7}\u{5199}\u{6743}\u{9650}\u{ff0c}\u{6216}\u{8005}\u{6539}\u{7528} SSH remote\u{3002}",
    ),
    ("blame.title", "\u{6309}\u{884c}\u{5ba1}\u{9605}"),
    (
        "blame.loading",
        "\u{6b63}\u{5728}\u{52a0}\u{8f7d}\u{6309}\u{884c}\u{5ba1}\u{9605}...",
    ),
    (
        "blame.empty",
        "\u{6ca1}\u{6709}\u{6309}\u{884c}\u{5ba1}\u{9605}\u{7ed3}\u{679c}",
    ),
    ("blame.path", "\u{6587}\u{4ef6}"),
    ("blame.line", "\u{884c}"),
    ("blame.commit", "\u{63d0}\u{4ea4}"),
    ("blame.author", "\u{4f5c}\u{8005}"),
    ("blame.summary", "\u{4fe1}\u{606f}"),
    ("blame.content", "\u{5185}\u{5bb9}"),
    (
        "undo.toast.ready",
        "\u{5df2}\u{5b8c}\u{6210} {action}\u{ff0c}\u{53ef}\u{64a4}\u{9500}",
    ),
    (
        "undo.toast.completed",
        "\u{5df2}\u{64a4}\u{9500} {action}\u{ff0c}\u{53ef}\u{91cd}\u{505a}",
    ),
    (
        "redo.toast.completed",
        "\u{5df2}\u{91cd}\u{505a} {action}\u{ff0c}\u{53ef}\u{518d}\u{6b21}\u{64a4}\u{9500}",
    ),
];

const EN: &[(&str, &str)] = &[
    ("app.title", "Git Agent"),
    ("app.subtitle", "fast visual Git client"),
    ("action.clone_new", "Clone/New"),
    ("action.open", "Open"),
    ("action.refresh", "Refresh"),
    ("action.fetch", "Fetch"),
    ("action.pull", "Pull"),
    ("action.push", "Push"),
    ("undo.toast.ready", "{action} completed. Undo is available."),
    ("undo.toast.completed", "Undid {action}. Redo is available."),
    ("redo.toast.completed", "Redid {action}. Undo is available."),
    ("pull.title", "Pull"),
    ("pull.remote", "Pull from remote"),
    ("pull.remote_branch", "Remote branch"),
    ("pull.local_branch", "Pull into local branch"),
    ("pull.options", "Options"),
    ("pull.commit_merge", "Commit merged changes immediately"),
    ("pull.include_tags", "Include tags from merged commits"),
    (
        "pull.force_merge_commit",
        "Create a merge commit even when fast-forward is possible",
    ),
    ("pull.rebase", "Rebase instead of merge"),
    ("pull.refresh", "Refresh"),
    ("fetch.title", "Fetch"),
    ("fetch.all_remotes", "Fetch from all remotes"),
    (
        "fetch.prune_tracking",
        "Prune tracking branches that no longer exist on the remote (tracking)",
    ),
    ("fetch.tags", "Fetch all tags"),
    ("fetch.force_tags", "Overwrite local tags (--force)"),
    ("push.title", "Push"),
    ("push.remote", "Push to repository"),
    ("push.branches", "Branches to push"),
    ("push.select", "Push?"),
    ("push.local_branch", "Local branch"),
    ("push.remote_branch", "Remote branch"),
    ("push.track", "Track?"),
    ("push.select_all", "Select all"),
    ("push.push_tags", "Push all tags"),
    ("push.force", "Safe force push (--force-with-lease)"),
    (
        "push.detached_error",
        "Current repository is in detached HEAD and cannot be pushed to a remote branch. Check out or create a local branch first.",
    ),
    ("push.force_confirm.title", "Confirm Safe Force Push"),
    (
        "push.force_confirm.message",
        "This uses --force-with-lease to update the remote branch. Confirm the remote does not contain someone else's new commits.",
    ),
    ("push.force_confirm.submit", "Confirm Safe Force Push"),
    ("pull_request.title", "Create Pull Request"),
    ("pull_request.remote", "Submit through remote:"),
    ("pull_request.local_branch", "Local branch"),
    ("pull_request.remote_branch", "Remote branch"),
    (
        "pull_request.remote_branch_placeholder",
        "Enter remote branch",
    ),
    (
        "pull_request.hint",
        "The latest commit will be pushed before creating the pull request",
    ),
    ("pull_request.submit", "Create Pull Request Online"),
    ("pull_request.error.remote_invalid", "Select a valid remote"),
    (
        "pull_request.error.local_branch_invalid",
        "Select a valid local branch",
    ),
    (
        "pull_request.error.remote_branch_invalid",
        "Enter a valid remote branch",
    ),
    ("rewrite_prompt.title", "History Rewritten"),
    (
        "rewrite_prompt.message",
        "Interactive rebase completed and the current branch now diverges from the remote history. Open push options and use --force-with-lease after confirming.",
    ),
    ("rewrite_prompt.open_push", "Open Push Options"),
    ("rewrite_prompt.later", "Later"),
    ("settings.title", "Settings"),
    ("options.title", "Options"),
    ("settings.appearance", "Appearance"),
    ("settings.theme", "Theme"),
    ("settings.ssh_configuration", "SSH Client Configuration"),
    ("settings.ssh_client", "SSH client"),
    ("settings.ssh_key", "SSH key"),
    ("settings.ssh_choose_key", "Choose SSH key"),
    ("settings.ssh_executable", "Client executable"),
    (
        "settings.ssh_executable_placeholder",
        "Leave empty to auto-detect the selected client",
    ),
    ("settings.ssh_choose_executable", "Choose client executable"),
    ("settings.ssh_auto_detected", "Auto-detected"),
    ("settings.ssh_not_found", "Client executable not found"),
    ("ssh.agent_started", "SSH agent started"),
    ("ssh.status.title", "SSH Agent"),
    ("ssh.status.client", "Client"),
    ("ssh.status.state", "Status"),
    ("ssh.status.running", "Running"),
    ("ssh.status.starting", "Starting SSH agent"),
    ("ssh.status.refreshing", "Refreshing status"),
    ("ssh.status.adding_key", "Adding SSH key"),
    ("ssh.status.backend", "Agent backend"),
    ("ssh.status.loaded_keys", "Loaded keys"),
    ("ssh.status.no_keys", "No SSH keys are loaded"),
    (
        "ssh.status.external_key_list",
        "The key list is managed by Pageant.",
    ),
    (
        "ssh.status.openssh_background",
        "OpenSSH ssh-agent runs as a background service and does not open a separate window.",
    ),
    (
        "ssh.status.putty_tray",
        "Pageant runs in the system tray; open Pageant to inspect and manage keys.",
    ),
    ("ssh.status.refresh", "Refresh"),
    ("ssh.status.add_key", "Add Key"),
    ("ssh.load_key.title", "Load SSH Key?"),
    (
        "ssh.load_key.message",
        "Do you want to load an SSH key now? Choose No to load one later from Tools > Add SSH Key.",
    ),
    ("ssh.load_key.yes", "Yes"),
    ("ssh.load_key.no", "No"),
    ("ssh.key_added", "SSH key sent to agent"),
    ("ssh.task_stopped", "SSH tool task stopped unexpectedly"),
    (
        "ssh.openssh_agent_not_found",
        "OpenSSH ssh-add was not found. Choose a valid OpenSSH client executable in Options > Git.",
    ),
    (
        "ssh.openssh_agent_service_missing",
        "The Windows OpenSSH ssh-agent service was not found. Install the Windows OpenSSH Client feature first.",
    ),
    (
        "ssh.openssh_agent_elevation_failed",
        "Unable to enable the Windows OpenSSH ssh-agent service. Allow the administrator permission request, then try again.",
    ),
    (
        "ssh.openssh_agent_connect_failed",
        "The Windows OpenSSH ssh-agent service started, but ssh-add cannot connect.",
    ),
    ("ssh.openssh_installed", "OpenSSH client installed"),
    ("ssh.openssh_install.title", "OpenSSH Not Found"),
    (
        "ssh.openssh_install.message",
        "OpenSSH is selected, but no usable ssh client executable was found on this system.",
    ),
    (
        "ssh.openssh_install.windows_hint",
        "Windows will install OpenSSH Client as an optional system feature. Administrator permission and a network connection are required.",
    ),
    (
        "ssh.openssh_install.unix_hint",
        "OpenSSH normally ships with macOS. On Linux, install openssh-client with the system package manager, or choose the client executable manually.",
    ),
    ("ssh.openssh_install.install", "Install OpenSSH"),
    (
        "ssh.openssh_install.installing",
        "Installing OpenSSH. Allow the administrator permission request from the system.",
    ),
    ("ssh.openssh_install.guide", "Open Installation Guide"),
    ("ssh.openssh_install.recheck", "Check Again"),
    ("ssh.openssh_install.detected", "OpenSSH client detected"),
    ("ssh.openssh_install.use_putty", "Use PuTTY / Plink"),
    (
        "ssh.openssh_install.failed",
        "Unable to install Windows OpenSSH Client. Check the network connection and Windows optional feature service, or follow the installation guide manually.",
    ),
    (
        "ssh.openssh_install.guided_only",
        "Automatic OpenSSH installation is not supported on this system. Follow the installation guide, or choose a client executable in Options > Git.",
    ),
    ("ssh.install.title", "PuTTY / Plink Not Found"),
    (
        "ssh.install.message",
        "PuTTY / Plink is selected, but Plink or Pageant is missing. PuTTY is free and open source. Install the complete PuTTY package, switch to OpenSSH, or choose an installed client executable in Options > Git.",
    ),
    (
        "ssh.install.auto_detect_hint",
        "When empty, Git Agent detects the executable for the selected SSH client and operating system. A manually selected file takes priority.",
    ),
    ("ssh.install.download", "Open Official Download"),
    ("ssh.install.use_openssh", "Use OpenSSH"),
    ("ssh.install.open_settings", "Open SSH Settings"),
    ("ssh.install.later", "Later"),
    ("about.title", "About Git Agent"),
    ("about.version", "Version"),
    ("about.repository", "Project home"),
    ("update.title", "Check for Updates"),
    ("update.current_version", "Current version"),
    ("update.latest_version", "Latest version"),
    ("update.checking", "Checking GitHub Releases"),
    (
        "update.installing",
        "Downloading and starting the installer",
    ),
    ("update.available", "An update is available"),
    ("update.up_to_date", "Git Agent is up to date"),
    ("update.package", "Installer"),
    (
        "update.no_asset",
        "No installer is available for this operating system",
    ),
    ("update.download_install", "Download and Install"),
    ("update.open_release", "Open Release Page"),
    ("update.retry", "Retry"),
    ("update.failed", "Unable to check or install the update"),
    ("update.installed", "The update is installed"),
    (
        "update.restart_required",
        "Restart Git Agent to use the new version",
    ),
    ("update.stopped", "The update task stopped unexpectedly"),
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
    ("settings.repository_workspaces", "Workspaces"),
    (
        "settings.repository_workspaces_hint",
        "Local repositories scan one level below these folders.",
    ),
    ("settings.repository_workspaces_default", "Default:"),
    (
        "settings.repository_workspaces_empty",
        "No workspaces configured",
    ),
    ("settings.repository_workspace_add", "Add workspace"),
    ("settings.auto_refresh", "Auto Refresh"),
    (
        "settings.refresh_active_repo_seconds",
        "Current repo (seconds)",
    ),
    (
        "settings.refresh_inactive_repo_seconds",
        "Other repos (seconds)",
    ),
    ("settings.tab_empty", "No settings in this category yet."),
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
    ("diff.revert_hunk", "Revert hunk"),
    (
        "diff.revert_select_line",
        "Select a diff line to revert its hunk",
    ),
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
    ("git_flow.initialize.title", "Initialize Git Workflow"),
    (
        "git_flow.initialize.detail",
        "Git Flow saves Production/Develop branch and Feature/Release/Hotfix prefix settings. If the Develop branch does not exist, it is created from Production; Feature/Release/Hotfix branches are created by their start actions.",
    ),
    ("git_flow.production_branch", "Production branch"),
    ("git_flow.development_branch", "Develop branch"),
    ("git_flow.feature_prefix", "Feature prefix"),
    ("git_flow.release_prefix", "Release prefix"),
    ("git_flow.hotfix_prefix", "Hotfix prefix"),
    ("git_flow.version_tag_prefix", "Tag prefix"),
    ("git_flow.use_defaults", "Use defaults"),
    ("git_flow.initialize.submit", "Initialize"),
    ("git_flow.other_action.title", "Choose Git Flow Action"),
    ("git_flow.name", "Name"),
    ("git_flow.feature_name", "Feature name"),
    ("git_flow.release_name", "Release name"),
    ("git_flow.hotfix_name", "Hotfix name"),
    ("git_flow.start_from", "Start from:"),
    ("git_flow.branch_name", "Branch name"),
    ("git_flow.branch_preview", "Branch to create:"),
    ("git_flow.preview", "Preview"),
    ("git_flow.preview.create_branch", "Create branch"),
    ("git_flow.preview.missing_start", "No start point"),
    ("git_flow.preview.merge_prefix", "Merge"),
    ("git_flow.preview.merge_suffix", "into"),
    ("git_flow.preview.latest_feature", "Latest feature branch"),
    ("git_flow.preview.latest_release", "Latest release branch"),
    ("git_flow.preview.latest_hotfix", "Latest hotfix branch"),
    ("git_flow.start_feature.title", "Start New Feature"),
    ("git_flow.finish_feature.title", "Finish Feature"),
    ("git_flow.start_release.title", "Start New Release"),
    ("git_flow.finish_release.title", "Finish Release"),
    ("git_flow.start_hotfix.title", "Start New Hotfix"),
    ("git_flow.finish_hotfix.title", "Finish Hotfix"),
    (
        "git_flow.start.detail",
        "Create and checkout a new branch from the configured base branch.",
    ),
    (
        "git_flow.finish_feature.detail",
        "Merge the feature branch back into develop and delete the feature branch.",
    ),
    (
        "git_flow.finish_release.detail",
        "Merge the release branch into production and develop, then create a tag.",
    ),
    (
        "git_flow.finish_hotfix.detail",
        "Merge the hotfix branch into production and develop, then create a tag.",
    ),
    ("git_flow.start", "Start"),
    ("git_flow.finish", "Finish"),
    (
        "git_flow.finish.rebase_development",
        "Rebase on the development branch",
    ),
    ("git_flow.finish.after", "After finish:"),
    ("git_flow.finish.delete_branch", "Delete branch"),
    ("git_flow.finish.force_delete", "Force delete"),
    ("git_flow.finish.tag_message", "Tag this message:"),
    (
        "git_flow.finish.tag_message_placeholder",
        "Enter tag message",
    ),
    ("git_flow.finish.push_remote", "Push changes to remote"),
    ("git_flow.error.fix_inputs", "Fix these inputs:"),
    ("git_flow.error.required", "Required fields cannot be empty"),
    ("git_flow.error.branch_invalid", "Branch name is invalid"),
    (
        "git_flow.error.branch_same",
        "Production and develop branches cannot be the same",
    ),
    ("git_flow.error.branch_exists", "Branch already exists"),
    (
        "git_flow.error.branch_prefix",
        "Branch prefix does not match this action",
    ),
    ("git_flow.error.branch_missing", "Branch does not exist"),
    (
        "git_flow.error.start_point_missing",
        "Choose a valid start point",
    ),
    ("repo.command_mode.failed", "Failed to open command mode"),
    (
        "repo.resource_manager.failed",
        "Failed to open resource manager",
    ),
    ("repo.remote.missing", "No remote URL configured"),
    ("repo.remote.failed", "Failed to open remote URL"),
    ("benchmark.title", "Benchmark Repository Performance"),
    ("benchmark.running", "Testing repository performance..."),
    ("benchmark.step.branches", "Reading local branches"),
    ("benchmark.step.remote_branches", "Reading remote branches"),
    ("benchmark.step.tracking", "Reading tracking branches"),
    ("benchmark.step.summary", "Reading repository summary"),
    ("benchmark.step.tags", "Reading tags"),
    ("benchmark.step.commit_labels", "Reading commit labels"),
    ("benchmark.step.stashes", "Reading stashes"),
    ("benchmark.step.logs", "Reading commit history"),
    ("benchmark.step.commit_details", "Reading commit details"),
    ("benchmark.step.file_status", "Reading file status"),
    ("benchmark.step.remotes", "Reading remote repositories"),
    ("benchmark.step.files", "Counting files"),
    ("benchmark.step.system", "Reading system information"),
    ("benchmark.save_title", "Save Benchmark File"),
    ("benchmark.saved", "Benchmark saved"),
    ("benchmark.save_failed", "Failed to save benchmark"),
    ("benchmark.cancelled", "Benchmark save cancelled"),
    ("benchmark.stopped", "Repository benchmark stopped"),
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
    ("branch.confirm_checkout_title", "Confirm branch switch"),
    ("branch.confirm_checkout", "Switch working copy to"),
    (
        "branch.discard_before_checkout",
        "Clear (discard all changes)",
    ),
    ("checkout.title", "Checkout"),
    ("checkout.existing", "Checkout existing"),
    ("checkout.new_branch", "Checkout new branch"),
    ("checkout.existing_commit", "Select commit to checkout"),
    ("checkout.remote_branch", "Remote branch to checkout:"),
    ("checkout.local_branch", "New local branch name:"),
    ("checkout.select_remote_branch", "Select remote branch"),
    (
        "checkout.track_remote",
        "Local branch should track remote branch",
    ),
    (
        "checkout.detached_confirm_title",
        "Warning: creating detached HEAD",
    ),
    ("checkout.detached_target", "Checkout commit"),
    (
        "checkout.detached_warning_detail",
        "Checking out this commit creates a detached HEAD. You will not be on any branch. Use it for temporary commit verification; create a new branch if you need to keep the work.",
    ),
    (
        "checkout.error.fix_inputs",
        "Please correct the following input errors:",
    ),
    (
        "checkout.error.local_branch_invalid",
        "Local branch name is invalid",
    ),
    (
        "checkout.error.remote_branch_invalid",
        "Selected remote branch name is invalid",
    ),
    (
        "checkout.error.local_branch_exists",
        "Local branch already exists",
    ),
    ("interactive_rebase.title", "Interactive Rebase"),
    (
        "interactive_rebase.select_commit",
        "Select commits to rebase",
    ),
    ("interactive_rebase.selected_commit", "Selected commit:"),
    ("interactive_rebase.base_commit", "Base commit:"),
    (
        "interactive_rebase.published_warning",
        "This operation rewrites history that has already been pushed to the remote. After it completes, you will need a force-with-lease push, and anyone who pulled this branch can be affected.",
    ),
    (
        "interactive_rebase.confirm_published",
        "I understand this rewrites pushed history",
    ),
    ("interactive_rebase.todo_action", "Action:"),
    ("interactive_rebase.todo.pick", "Pick"),
    ("interactive_rebase.todo.reword", "Reword message"),
    ("interactive_rebase.todo.edit", "Edit commit"),
    ("interactive_rebase.todo.none", "Choose action"),
    ("interactive_rebase.todo.squash", "Squash into previous"),
    ("interactive_rebase.todo.fixup", "Fixup (discard message)"),
    ("interactive_rebase.autosquash_target", "Autosquash target:"),
    ("interactive_rebase.todo.drop", "Drop"),
    ("interactive_rebase.reword.commit", "Reword commit:"),
    ("interactive_rebase.reword.title", "Reword commit message"),
    ("interactive_rebase.reword.hint", "Enter commit message"),
    ("interactive_rebase.reword.author", "Author"),
    ("interactive_rebase.reword.email", "Email"),
    ("interactive_rebase.reword.author_hint", "Enter author name"),
    ("interactive_rebase.reword.email_hint", "Enter author email"),
    ("interactive_rebase.reword.edit", "Edit reword message"),
    ("interactive_rebase.reword.done", "Save reword"),
    ("interactive_rebase.reword.cancel", "Cancel reword"),
    ("interactive_rebase.plan", "Rebase plan"),
    ("interactive_rebase.plan_hint", "Drag :: to reorder commits"),
    ("interactive_rebase.preview", "Plan preview"),
    ("interactive_rebase.plan_position", "Plan position"),
    ("interactive_rebase.preview_target", "Fold target"),
    ("interactive_rebase.preview_previous", "Previous"),
    ("interactive_rebase.preview_next", "Next"),
    (
        "interactive_rebase.preview_effect.pick",
        "This commit will be replayed unchanged",
    ),
    (
        "interactive_rebase.preview_effect.reword",
        "This commit message and author will be rewritten",
    ),
    (
        "interactive_rebase.preview_effect.edit",
        "Rebase will pause after applying this commit",
    ),
    (
        "interactive_rebase.preview_effect.squash",
        "This commit will be folded into its target and keep its message",
    ),
    (
        "interactive_rebase.preview_effect.fixup",
        "This commit will be folded into its target and discard its message",
    ),
    (
        "interactive_rebase.preview_effect.drop",
        "This commit and its changes will be removed",
    ),
    (
        "interactive_rebase.graph_drag_hint",
        "Drag a commit onto an ancestor to open a rebase plan",
    ),
    (
        "interactive_rebase.graph_drag_started",
        "Interactive rebase plan opened",
    ),
    (
        "interactive_rebase.graph_drag_invalid",
        "Drag a non-merge commit onto one of its ancestors",
    ),
    ("interactive_rebase.move_up", "Move up"),
    ("interactive_rebase.move_down", "Move down"),
    ("interactive_rebase.reset", "Reset"),
    ("interactive_rebase.selected_count", "Selected"),
    ("interactive_rebase.drop_selected", "Drop selected"),
    (
        "interactive_rebase.squash_selected",
        "Squash selected into previous",
    ),
    ("interactive_rebase.squash_target", "Squash target:"),
    (
        "interactive_rebase.squash_to_target",
        "Squash selected into target",
    ),
    (
        "interactive_rebase.squash_to_target_applied",
        "Squash selected into",
    ),
    ("interactive_rebase.reset_selected", "Reset selected"),
    (
        "interactive_rebase.merge_commit_disabled",
        "Merge commits are shown for context and cannot be edited by this interactive rebase.",
    ),
    ("interactive_rebase.squash_previous", "Squash with previous"),
    ("interactive_rebase.drop_commit", "Drop"),
    (
        "interactive_rebase.error.dirty",
        "git rebase -i --autosquash\nerror: cannot rebase: You have unstaged changes.\nerror: Please commit or stash them.",
    ),
    (
        "interactive_rebase.error.index_dirty",
        "git rebase -i --autosquash\nerror: cannot rebase: Your index contains uncommitted changes.\nerror: Please commit or stash them.",
    ),
    (
        "interactive_rebase.error.in_progress",
        "A rebase is already in progress.\nRun git rebase --continue, --abort, or --skip before starting another interactive rebase.",
    ),
    (
        "interactive_rebase.error.detached",
        "Current repository is in detached HEAD. Check out or create a local branch before interactive rebase.",
    ),
    (
        "interactive_rebase.error.no_commits",
        "Current branch has no commits available for interactive rebase",
    ),
    (
        "interactive_rebase.error.no_children",
        "Selected commit has no later commits available for interactive rebase",
    ),
    (
        "interactive_rebase.error.confirm_published",
        "Confirm pushed-history rewrite first",
    ),
    (
        "interactive_rebase.error.first_squash",
        "Oldest commit cannot be squash",
    ),
    (
        "interactive_rebase.error.no_changes",
        "Choose at least one rebase action first",
    ),
    (
        "interactive_rebase.error.reword_unconfirmed",
        "Save the reworded message before continuing",
    ),
    ("interactive_rebase.in_progress.title", "Rebase in progress"),
    (
        "interactive_rebase.in_progress.detail",
        "This repository is currently running git rebase. Resolve conflicts, then continue, skip the current commit, or abort the rebase.",
    ),
    (
        "interactive_rebase.in_progress.conflicts",
        "Conflicted files must be resolved and staged before continuing.",
    ),
    (
        "interactive_rebase.in_progress.ready",
        "Conflicts are resolved. Continue the rebase.",
    ),
    (
        "interactive_rebase.in_progress.edit_ready",
        "Paused at an edit step. Stage changes, amend the current commit, then continue.",
    ),
    ("interactive_rebase.in_progress.continue", "Continue"),
    ("interactive_rebase.in_progress.skip", "Skip current commit"),
    ("interactive_rebase.in_progress.abort", "Abort rebase"),
    (
        "interactive_rebase.in_progress.amend",
        "Amend current commit",
    ),
    ("submodule.title", "Add Submodule..."),
    ("submodule.source", "Source path / URL:"),
    ("submodule.repo_type", "Repository type:"),
    ("submodule.local_path", "Local relative path:"),
    ("submodule.source_branch", "Source branch:"),
    ("submodule.recursive", "Recursive submodules"),
    ("subtree.title", "Add/Link Subtree"),
    ("subtree.source", "Source path / URL:"),
    ("subtree.repo_type", "Repository type:"),
    ("subtree.ref_name", "Branch / Commit:"),
    ("subtree.local_path", "Local relative path:"),
    ("subtree.squash", "Squash commits?"),
    ("subtree.error.ref_required", "Branch / commit is required"),
    ("dependency.advanced_options", "Advanced options"),
    ("dependency.repo_type_missing", "No path or URL provided"),
    ("dependency.repo_type_git", "Git repository"),
    ("dependency.repo_type_local", "Local path"),
    (
        "dependency.error.source_required",
        "No path or URL provided",
    ),
    (
        "dependency.error.local_path_required",
        "Local relative path is required",
    ),
    (
        "dependency.error.local_path_relative",
        "Local path must be a relative path inside the repository",
    ),
    ("lfs.init.title", "Initialize Git LFS for repository"),
    ("lfs.intro.heading", "Git LFS"),
    (
        "lfs.intro.body",
        "Git LFS stores large files such as videos, designs, game assets, and binary packages in large-file storage while keeping small pointer files in the Git repository. This keeps clones, pulls, and pushes lighter.",
    ),
    (
        "lfs.intro.note",
        "Next choose file patterns Git LFS should track, such as *.psd, *.mp4, or *.zip.",
    ),
    ("lfs.start", "Start using Git LFS"),
    ("lfs.track.title", "Git LFS: Select tracked files"),
    ("lfs.patterns_label", "File types tracked in Git LFS"),
    (
        "lfs.pattern_help",
        "You can add more patterns later from Repository > Git LFS.",
    ),
    ("lfs.add", "Add"),
    ("lfs.remove", "Remove"),
    ("lfs.track_files", "Track files"),
    ("lfs.pattern_empty", "No tracked patterns yet"),
    ("lfs.pattern_placeholder", "Example: *.psd"),
    ("lfs.error.pattern_required", "Tracked pattern is required"),
    (
        "lfs.error.duplicate_pattern",
        "Tracked patterns cannot repeat",
    ),
    (
        "merge.title",
        "Select a commit to merge into the current branch",
    ),
    ("merge.select_commit", "Select commit to merge"),
    ("merge.selected_commit", "Selected commit:"),
    (
        "merge.commit_immediately",
        "Commit merged changes immediately (if there are no conflicts)",
    ),
    (
        "merge.include_messages",
        "Include messages from merged commits",
    ),
    (
        "merge.force_merge_commit",
        "Create a new commit even if fast-forward is possible",
    ),
    (
        "merge.rebase",
        "Rebase instead of merge (warning: make sure you have not pushed your changes)",
    ),
    ("merge.detect_renames", "Detect similar renames"),
    ("archive.title", "Archive"),
    ("archive.output_path", "Archive file:"),
    ("archive.folder_prefix", "Folder prefix:"),
    ("archive.target", "Commit:"),
    ("archive.worktree", "Working copy version"),
    ("archive.commit", "Specified commit:"),
    ("archive.error.output_required", "Archive file is required"),
    (
        "archive.error.commit_required",
        "Specified commit is required",
    ),
    ("branch.pull_tracked", "Pull (tracked)"),
    ("branch.push_tracked", "Push to (tracked)"),
    ("branch.push_to", "Push to"),
    ("branch.track_remote", "Track remote branch"),
    ("branch.no_remote_tracking", "(None)"),
    ("branch.no_remotes", "No remotes"),
    ("branch.compare_with_current", "Compare with current"),
    ("branch.rename_title", "Rename branch"),
    ("branch.new_name", "New name"),
    ("branch.create_pull_request", "Create pull request..."),
    ("remote.title", "Remote Branches"),
    ("remote.none", "No remote repositories"),
    ("remote.no_branches", "No fetched remote branches"),
    ("worktree.title", "Workspace"),
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
    (
        "worktree.discard_all_confirm",
        "Discard all uncommitted changes?",
    ),
    (
        "worktree.discard_all_warning",
        "This resets tracked changes and deletes untracked files.",
    ),
    (
        "worktree.discard_untracked_warning",
        "This will delete the untracked file or directory.",
    ),
    (
        "worktree.discard_tracked_warning",
        "This will restore the path from HEAD.",
    ),
    ("patch.create.title", "Create Patch"),
    ("patch.create.worktree_tab", "Working Copy Changes"),
    ("patch.create.history_tab", "Log / History"),
    ("patch.create.output_path", "Patch file"),
    (
        "patch.create.empty_worktree",
        "There are no working-copy changes to include.",
    ),
    (
        "patch.create.history_empty",
        "Select commits to create a patch.",
    ),
    ("patch.create.changed_files", "Changed files"),
    ("patch.create.browse", "Browse"),
    (
        "patch.create.separate_files",
        "Create a separate patch file per commit",
    ),
    ("patch.create.submit", "Create Patch"),
    ("patch.create.no_selection", "Select at least one item."),
    (
        "patch.create.invalid_output",
        "Select a valid patch file path.",
    ),
    ("patch.create.running", "Creating patch..."),
    ("patch.create.success", "Patch created."),
    (
        "patch.create.disconnected",
        "The patch task stopped unexpectedly.",
    ),
    (
        "patch.create.overwrite_title",
        "Overwrite existing patches?",
    ),
    (
        "patch.create.overwrite_message",
        "The following files already exist. Continuing will overwrite them.",
    ),
    ("patch.create.overwrite_confirm", "Overwrite and Create"),
    (
        "patch.apply.failed_hint",
        "Could not apply the patch to the current branch. The branch may not contain the files or baseline content used to create the patch.",
    ),
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
    ("stash.staged_files", "Staged files / selected files"),
    ("stash.keep_staged", "Keep staged changes"),
    ("stash.include_untracked", "Untracked files"),
    ("stash.include_ignored", "All"),
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
    ("credentials.github_login", "Log in to GitHub"),
    (
        "credentials.github_login_running",
        "Logging in to GitHub...",
    ),
    ("credentials.github_login_done", "GitHub login completed"),
    (
        "credentials.github_login_done_message",
        "GitHub login completed. If there is no Git operation that can be retried automatically, open push again and confirm it.",
    ),
    (
        "credentials.github_retry_failed",
        "Automatic retry after login still failed. Check the current HTTPS credential, GitHub account write permission, or switch this remote to SSH.",
    ),
    ("blame.title", "Blame"),
    ("blame.loading", "Loading blame..."),
    ("blame.empty", "No blame rows"),
    ("blame.path", "File"),
    ("blame.line", "Line"),
    ("blame.commit", "Commit"),
    ("blame.author", "Author"),
    ("blame.summary", "Summary"),
    ("blame.content", "Content"),
    ("menu.copy_hash", "Copy commit hash"),
    ("menu.copy_short_hash", "Copy short hash"),
    ("menu.copy", "Copy"),
    ("menu.checkout_commit", "Checkout this commit"),
    ("menu.create_branch", "Create branch here"),
    ("menu.create_tag", "Create tag here"),
    ("menu.cherry_pick", "Cherry-pick commit"),
    (
        "menu.interactive_rebase_children",
        "Rebase children of this commit interactively...",
    ),
    ("menu.revert", "Revert commit"),
    ("menu.reset", "Reset current branch to here"),
    ("menu.compare", "Compare"),
    ("menu.compare_worktree", "Compare with working tree"),
    ("menu.external_diff", "External diff"),
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
            t(Language::Chinese, "blame.title"),
            "\u{6309}\u{884c}\u{5ba1}\u{9605}"
        );
        assert_eq!(
            t(Language::Chinese, "worktree.discard_untracked_warning"),
            "\u{8fd9}\u{4f1a}\u{5220}\u{9664}\u{672a}\u{8ddf}\u{8e2a}\u{7684}\u{6587}\u{4ef6}\u{6216}\u{76ee}\u{5f55}\u{3002}"
        );
        assert_eq!(
            t(Language::Chinese, "worktree.discard_tracked_warning"),
            "\u{8fd9}\u{4f1a}\u{4ece} HEAD \u{6062}\u{590d}\u{8be5}\u{8def}\u{5f84}\u{3002}"
        );
        assert_eq!(t(Language::English, "blame.title"), "Blame");
        assert_eq!(
            t(Language::Chinese, "undo.toast.ready"),
            "\u{5df2}\u{5b8c}\u{6210} {action}\u{ff0c}\u{53ef}\u{64a4}\u{9500}"
        );
        assert_eq!(
            t(Language::English, "undo.toast.ready"),
            "{action} completed. Undo is available."
        );
        assert_eq!(
            t(Language::English, "worktree.discard_untracked_warning"),
            "This will delete the untracked file or directory."
        );
        for key in [
            "patch.create.title",
            "patch.create.worktree_tab",
            "patch.create.history_tab",
            "patch.create.output_path",
            "patch.create.empty_worktree",
            "patch.create.separate_files",
            "patch.create.submit",
            "patch.create.no_selection",
            "patch.create.invalid_output",
            "patch.create.running",
            "patch.create.success",
            "patch.create.disconnected",
            "patch.create.overwrite_title",
            "patch.create.overwrite_message",
            "patch.create.overwrite_confirm",
            "patch.apply.failed_hint",
        ] {
            assert_ne!(t(Language::Chinese, key), key, "missing Chinese {key}");
            assert_ne!(t(Language::English, key), key, "missing English {key}");
        }
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

    #[test]
    fn git_flow_finish_release_labels_are_translated() {
        for (key, zh, en) in [
            (
                "git_flow.release_name",
                "\u{53d1}\u{5e03}\u{7248}\u{672c}\u{540d}",
                "Release name",
            ),
            (
                "git_flow.hotfix_name",
                "\u{4fee}\u{590d}\u{8865}\u{4e01}\u{540d}",
                "Hotfix name",
            ),
            (
                "git_flow.finish.tag_message",
                "\u{6b64}\u{4fe1}\u{606f}\u{7684}\u{6807}\u{7b7e}:",
                "Tag this message:",
            ),
            (
                "git_flow.finish.tag_message_placeholder",
                "\u{8bf7}\u{8f93}\u{5165}\u{6807}\u{7b7e}\u{4fe1}\u{606f}",
                "Enter tag message",
            ),
            (
                "git_flow.finish.push_remote",
                "\u{63a8}\u{9001}\u{53d8}\u{66f4}\u{5230}\u{8fdc}\u{7aef}\u{4ed3}\u{5e93}",
                "Push changes to remote",
            ),
            (
                "git_flow.preview.latest_release",
                "\u{6700}\u{65b0}\u{7684}\u{53d1}\u{5e03}\u{7248}\u{672c}\u{5206}\u{652f}",
                "Latest release branch",
            ),
            (
                "git_flow.preview.latest_hotfix",
                "\u{6700}\u{65b0}\u{7684}\u{4fee}\u{590d}\u{8865}\u{4e01}\u{5206}\u{652f}",
                "Latest hotfix branch",
            ),
        ] {
            assert_eq!(t(Language::Chinese, key), zh, "{key}");
            assert_eq!(t(Language::English, key), en, "{key}");
        }
    }
}
