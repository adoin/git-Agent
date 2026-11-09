use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use semver::Version;
use serde::Deserialize;

pub const REPOSITORY_URL: &str = "https://github.com/adoin/git-Agent";
pub const RELEASE_PAGE_URL: &str = "https://github.com/adoin/git-Agent/releases";
const LATEST_RELEASE_API: &str = "https://api.github.com/repos/adoin/git-Agent/releases/latest";
const TRUSTED_DOWNLOAD_PREFIX: &str = "https://github.com/adoin/git-Agent/releases/download/";
const USER_AGENT: &str = "Git-Agent-Updater";
const GH_AUTH_TIMEOUT: Duration = Duration::from_secs(10);
const GH_API_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_GH_OUTPUT_BYTES: u64 = 2 * 1024 * 1024;
const GH_AUTH_ARGS: &[&str] = &["auth", "status", "--active", "--hostname", "github.com"];
const GH_RELEASE_ARGS: &[&str] = &[
    "api",
    "--hostname",
    "github.com",
    "--method",
    "GET",
    "repos/adoin/git-Agent/releases/latest",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseAsset {
    pub name: String,
    pub download_url: String,
    pub size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateRelease {
    pub tag: String,
    pub name: String,
    pub notes: String,
    pub page_url: String,
    pub is_newer: bool,
    pub asset: Option<ReleaseAsset>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallOutcome {
    InstallerLaunched,
    Installed,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    name: String,
    body: Option<String>,
    html_url: String,
    assets: Vec<GithubAsset>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

pub fn check_latest_release(current_version: &str) -> Result<UpdateRelease> {
    check_release_with_sources(
        current_version,
        std::env::consts::OS,
        latest_release_via_rest,
        latest_release_via_gh,
    )
}

fn check_release_with_sources(
    current_version: &str,
    os: &str,
    primary: impl FnOnce() -> Result<GithubRelease>,
    fallback: impl FnOnce() -> Result<GithubRelease>,
) -> Result<UpdateRelease> {
    parse_version(current_version)?;
    match primary().and_then(|release| release_from_response(current_version, release, os)) {
        Ok(release) => Ok(release),
        Err(primary_error) => fallback()
            .and_then(|release| release_from_response(current_version, release, os))
            .map_err(|fallback_error| {
                anyhow!("{primary_error:#}\nGitHub CLI fallback failed: {fallback_error:#}")
            }),
    }
}

fn latest_release_via_rest() -> Result<GithubRelease> {
    let response = ureq::get(LATEST_RELEASE_API)
        .timeout(std::time::Duration::from_secs(20))
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|error| anyhow!("GitHub release request failed: {error}"))?;
    response
        .into_json()
        .context("GitHub release response was not valid JSON")
}

fn latest_release_via_gh() -> Result<GithubRelease> {
    latest_release_via_gh_using(|args, timeout| {
        run_update_cli(&mut gh_update_command(args), timeout)
    })
}

#[derive(Debug)]
struct CliOutput {
    success: bool,
    stdout: Vec<u8>,
}

fn latest_release_via_gh_using(
    mut run: impl FnMut(&[&str], Duration) -> Result<CliOutput>,
) -> Result<GithubRelease> {
    // Spawning auth status checks availability and the active github.com login together.
    // Never invoke login, extract a token, or forward account/debug output to the UI.
    let auth = run(GH_AUTH_ARGS, GH_AUTH_TIMEOUT)
        .context("GitHub CLI is unavailable or its login check could not complete")?;
    if !auth.success {
        bail!("GitHub CLI has no usable active github.com login");
    }
    let response = run(GH_RELEASE_ARGS, GH_API_TIMEOUT)
        .context("GitHub CLI release request could not complete")?;
    if !response.success {
        bail!("GitHub CLI release request failed");
    }
    serde_json::from_slice(&response.stdout)
        .context("GitHub CLI release response was not valid JSON")
}

fn gh_update_command(args: &[&str]) -> Command {
    let mut command = Command::new("gh");
    command
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env("GH_NO_EXTENSION_UPDATE_NOTIFIER", "1")
        .env_remove("GH_DEBUG");
    command
}

fn run_update_cli(command: &mut Command, timeout: Duration) -> Result<CliOutput> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Unable to start the update CLI command")?;
    let stdout = child.stdout.take().expect("piped CLI stdout");
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        // Drain during execution: large release notes must not fill the pipe and
        // deadlock the child. Bound memory even for an unexpected CLI response.
        let mut bytes = Vec::new();
        let result = stdout
            .take(MAX_GH_OUTPUT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = sender.send(result);
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => {
                thread::sleep(Duration::from_millis(25));
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                match result {
                    Err(error) => return Err(error).context("Unable to wait for update CLI"),
                    _ => bail!("GitHub CLI command timed out"),
                }
            }
        }
    };
    let stdout = receiver
        .recv_timeout(Duration::from_secs(1))
        .context("GitHub CLI output reader did not complete")?
        .context("Unable to read GitHub CLI output")?;
    if stdout.len() as u64 > MAX_GH_OUTPUT_BYTES {
        bail!("GitHub CLI response exceeded the size limit");
    }
    Ok(CliOutput {
        success: status.success(),
        stdout,
    })
}

pub fn download_and_install(asset: &ReleaseAsset) -> Result<InstallOutcome> {
    validate_release_asset(asset)?;
    let directory = update_temp_directory()?;
    fs::create_dir_all(&directory)
        .with_context(|| format!("Unable to create update directory: {}", directory.display()))?;
    let archive_path = directory.join(&asset.name);
    download_asset(asset, &archive_path)?;
    install_downloaded_asset(&archive_path, &directory)
}

fn release_from_response(
    current_version: &str,
    release: GithubRelease,
    os: &str,
) -> Result<UpdateRelease> {
    let current = parse_version(current_version)?;
    let latest = parse_version(&release.tag_name)?;
    Ok(UpdateRelease {
        tag: release.tag_name,
        name: release.name,
        notes: release.body.unwrap_or_default().trim().to_owned(),
        page_url: release.html_url,
        is_newer: latest > current,
        asset: platform_asset(&release.assets, os).map(|asset| ReleaseAsset {
            name: asset.name.clone(),
            download_url: asset.browser_download_url.clone(),
            size: asset.size,
        }),
    })
}

fn parse_version(raw: &str) -> Result<Version> {
    let normalized = raw
        .trim()
        .strip_prefix('v')
        .or_else(|| raw.trim().strip_prefix('V'))
        .unwrap_or(raw.trim());
    Version::parse(normalized).with_context(|| format!("Invalid version: {raw}"))
}

fn platform_asset<'a>(assets: &'a [GithubAsset], os: &str) -> Option<&'a GithubAsset> {
    if !matches!(os, "windows" | "linux" | "macos") {
        return None;
    }
    assets
        .iter()
        .find(|asset| asset_name_matches_os(&asset.name, os))
}

fn asset_name_matches_os(name: &str, os: &str) -> bool {
    match os {
        "windows" => name.starts_with("GitAgentSetup-") && name.ends_with(".exe"),
        "linux" => name.starts_with("GitAgent_") && name.ends_with("_amd64.deb"),
        "macos" => name.starts_with("GitAgent-") && name.ends_with("-macOS.dmg"),
        _ => false,
    }
}

fn validate_release_asset(asset: &ReleaseAsset) -> Result<()> {
    if !asset.download_url.starts_with(TRUSTED_DOWNLOAD_PREFIX) {
        bail!("Update asset is not hosted by the official Git Agent release");
    }
    let file_name = Path::new(&asset.name)
        .file_name()
        .and_then(|name| name.to_str());
    if file_name != Some(asset.name.as_str()) || asset.name.is_empty() {
        bail!("Update asset has an invalid file name");
    }
    let valid_platform_name = asset_name_matches_os(&asset.name, std::env::consts::OS);
    if !valid_platform_name {
        bail!("Update asset does not match the current operating system");
    }
    Ok(())
}

fn update_temp_directory() -> Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System clock is earlier than the Unix epoch")?
        .as_millis();
    Ok(std::env::temp_dir().join(format!(
        "git-agent-update-{}-{timestamp}",
        std::process::id()
    )))
}

fn download_asset(asset: &ReleaseAsset, destination: &Path) -> Result<()> {
    let response = ureq::get(&asset.download_url)
        .set("Accept", "application/octet-stream")
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|error| anyhow!("Update download failed: {error}"))?;
    let mut reader = response.into_reader();
    let mut file = File::create(destination)
        .with_context(|| format!("Unable to create update file: {}", destination.display()))?;
    let bytes = io::copy(&mut reader, &mut file).context("Unable to save update package")?;
    file.sync_all().context("Unable to finish update package")?;
    if bytes == 0 {
        bail!("Downloaded update package is empty");
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn install_downloaded_asset(path: &Path, _directory: &Path) -> Result<InstallOutcome> {
    Command::new(path)
        .spawn()
        .with_context(|| format!("Unable to launch installer: {}", path.display()))?;
    Ok(InstallOutcome::InstallerLaunched)
}

#[cfg(target_os = "macos")]
fn install_downloaded_asset(path: &Path, _directory: &Path) -> Result<InstallOutcome> {
    Command::new("open")
        .arg(path)
        .spawn()
        .with_context(|| format!("Unable to open disk image: {}", path.display()))?;
    Ok(InstallOutcome::InstallerLaunched)
}

#[cfg(target_os = "linux")]
fn install_downloaded_asset(path: &Path, _directory: &Path) -> Result<InstallOutcome> {
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .or_else(|_| Command::new("gio").arg("open").arg(path).spawn())
        .with_context(|| {
            format!(
                "Unable to open system package installer: {}",
                path.display()
            )
        })?;
    Ok(InstallOutcome::InstallerLaunched)
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn install_downloaded_asset(_path: &Path, _directory: &Path) -> Result<InstallOutcome> {
    bail!("Automatic updates are not supported on this operating system")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli_response(success: bool, stdout: &[u8]) -> CliOutput {
        CliOutput {
            success,
            stdout: stdout.to_vec(),
        }
    }

    #[test]
    fn rest_success_does_not_probe_or_invoke_gh() {
        let release = check_release_with_sources(
            "1.1.0",
            "windows",
            || Ok(release_json()),
            || panic!("a successful REST request must not invoke gh"),
        )
        .unwrap();
        assert_eq!(release.tag, "v1.2.0");
    }

    #[test]
    fn rest_failure_uses_authenticated_cli_and_the_same_release_parser() {
        let mut calls = Vec::new();
        let payload = serde_json::to_vec(&serde_json::json!({
            "tag_name":"v1.2.0", "name":"Release", "body":"Notes",
            "html_url":"https://github.com/adoin/git-Agent/releases/tag/v1.2.0",
            "assets":[{"name":"GitAgentSetup-v1.2.0.exe", "size":30,
                "browser_download_url":"https://github.com/adoin/git-Agent/releases/download/v1.2.0/GitAgentSetup-v1.2.0.exe"}]
        })).unwrap();
        let release = check_release_with_sources(
            "1.1.0",
            "windows",
            || bail!("REST returned 403"),
            || {
                latest_release_via_gh_using(|args, timeout| {
                    calls.push((
                        args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>(),
                        timeout,
                    ));
                    Ok(cli_response(
                        true,
                        if args == GH_AUTH_ARGS { b"" } else { &payload },
                    ))
                })
            },
        )
        .unwrap();
        assert!(release.is_newer);
        assert_eq!(release.asset.unwrap().name, "GitAgentSetup-v1.2.0.exe");
        assert_eq!(
            calls,
            vec![
                (
                    GH_AUTH_ARGS
                        .iter()
                        .map(|arg| (*arg).to_owned())
                        .collect::<Vec<_>>(),
                    GH_AUTH_TIMEOUT
                ),
                (
                    GH_RELEASE_ARGS
                        .iter()
                        .map(|arg| (*arg).to_owned())
                        .collect::<Vec<_>>(),
                    GH_API_TIMEOUT
                )
            ]
        );
    }

    #[test]
    fn missing_cli_or_invalid_login_never_attempts_the_release_request() {
        for missing in [true, false] {
            let mut calls = 0;
            assert!(
                latest_release_via_gh_using(|args, _| {
                    calls += 1;
                    assert_eq!(args, GH_AUTH_ARGS);
                    if missing {
                        bail!("not found")
                    }
                    Ok(cli_response(false, b""))
                })
                .is_err()
            );
            assert_eq!(calls, 1);
        }
    }

    #[test]
    fn cli_errors_or_invalid_json_keep_both_failure_reasons() {
        for output in [
            cli_response(false, b"private diagnostic must not be displayed"),
            cli_response(true, b"not JSON"),
        ] {
            let mut output = Some(output);
            let error = check_release_with_sources(
                "1.1.0",
                "windows",
                || bail!("REST 403"),
                || {
                    latest_release_via_gh_using(|args, _| {
                        Ok(if args == GH_AUTH_ARGS {
                            cli_response(true, b"")
                        } else {
                            output.take().unwrap()
                        })
                    })
                },
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains("REST 403"));
            assert!(error.contains("GitHub CLI"));
            assert!(!error.contains("private diagnostic"));
        }
    }

    #[test]
    fn cli_authentication_and_api_timeouts_remain_bounded_failures() {
        for auth_timeout in [true, false] {
            let error = latest_release_via_gh_using(|args, _| {
                if auth_timeout || args == GH_RELEASE_ARGS {
                    bail!("command timed out")
                }
                Ok(cli_response(true, b""))
            })
            .unwrap_err();
            assert!(format!("{error:#}").contains("timed out"));
        }
    }

    #[test]
    fn invalid_current_version_never_sends_network_requests() {
        assert!(
            check_release_with_sources(
                "invalid",
                "windows",
                || panic!("must validate before REST"),
                || panic!("must validate before CLI")
            )
            .is_err()
        );
    }

    #[test]
    fn gh_update_commands_disable_prompts_and_do_not_extract_credentials() {
        for args in [GH_AUTH_ARGS, GH_RELEASE_ARGS] {
            let command = gh_update_command(args);
            assert_eq!(command.get_program(), "gh");
            let values = command
                .get_args()
                .map(|arg| arg.to_string_lossy())
                .collect::<Vec<_>>();
            assert!(
                !values
                    .iter()
                    .any(|arg| arg == "login" || arg == "token" || arg == "--show-token")
            );
            assert!(
                command
                    .get_envs()
                    .any(|(key, value)| key == "GH_PROMPT_DISABLED"
                        && value == Some(std::ffi::OsStr::new("1")))
            );
        }
    }

    fn fixture_command(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "updater::tests::cli_process_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("GIT_AGENT_UPDATER_CLI_FIXTURE", mode);
        command
    }

    #[test]
    fn cli_process_runner_drains_pipes_and_stops_an_unresponsive_child() {
        let output =
            run_update_cli(&mut fixture_command("large"), Duration::from_secs(10)).unwrap();
        assert!(output.success);
        assert!(output.stdout.len() > 128 * 1024);
        let started = Instant::now();
        let error =
            run_update_cli(&mut fixture_command("hang"), Duration::from_millis(150)).unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn cli_process_runner_rejects_oversized_output() {
        let error =
            run_update_cli(&mut fixture_command("oversized"), Duration::from_secs(10)).unwrap_err();
        assert!(error.to_string().contains("size limit"));
    }

    #[test]
    #[ignore = "subprocess fixture invoked only by the bounded-runner tests"]
    fn cli_process_fixture() {
        use std::io::Write;
        match std::env::var("GIT_AGENT_UPDATER_CLI_FIXTURE")
            .unwrap()
            .as_str()
        {
            "hang" => thread::sleep(Duration::from_secs(30)),
            "large" => {
                let _ = io::stdout().lock().write_all(&vec![b'x'; 256 * 1024]);
            }
            "oversized" => {
                let _ = io::stdout()
                    .lock()
                    .write_all(&vec![b'x'; MAX_GH_OUTPUT_BYTES as usize + 4096]);
            }
            _ => panic!("unknown fixture"),
        }
    }

    #[test]
    #[ignore = "requires a locally installed and authenticated GitHub CLI"]
    fn live_authenticated_gh_fallback() {
        let release = check_release_with_sources(
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            || bail!("simulated REST 403"),
            latest_release_via_gh,
        )
        .unwrap();
        assert!(release.asset.is_some());
        println!("Authenticated gh fallback selected {}", release.tag);
    }

    fn release_json() -> GithubRelease {
        serde_json::from_str(
            r#"{
                "tag_name":"v1.2.0",
                "name":"Git Agent v1.2.0",
                "body":"Release notes",
                "html_url":"https://github.com/adoin/git-Agent/releases/tag/v1.2.0",
                "assets":[
                    {"name":"GitAgent_1.2.0_amd64.deb","browser_download_url":"https://github.com/adoin/git-Agent/releases/download/v1.2.0/GitAgent_1.2.0_amd64.deb","size":10},
                    {"name":"GitAgent-1.2.0-macOS.dmg","browser_download_url":"https://github.com/adoin/git-Agent/releases/download/v1.2.0/GitAgent-1.2.0-macOS.dmg","size":20},
                    {"name":"GitAgentSetup-v1.2.0.exe","browser_download_url":"https://github.com/adoin/git-Agent/releases/download/v1.2.0/GitAgentSetup-v1.2.0.exe","size":30}
                ]
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn semantic_versions_ignore_v_prefix_and_prerelease_ordering() {
        assert!(parse_version("v1.2.0").unwrap() > parse_version("1.1.9").unwrap());
        assert!(parse_version("1.2.0").unwrap() > parse_version("1.2.0-beta.1").unwrap());
        assert!(parse_version("not-a-version").is_err());
    }

    #[test]
    fn release_result_selects_platform_asset_and_compares_current_version() {
        let windows = release_from_response("1.1.0", release_json(), "windows").unwrap();
        assert!(windows.is_newer);
        assert_eq!(windows.asset.unwrap().name, "GitAgentSetup-v1.2.0.exe");

        let linux = release_from_response("1.2.0", release_json(), "linux").unwrap();
        assert!(!linux.is_newer);
        assert_eq!(linux.asset.unwrap().name, "GitAgent_1.2.0_amd64.deb");

        let macos = release_from_response("1.3.0", release_json(), "macos").unwrap();
        assert!(!macos.is_newer);
        assert_eq!(macos.asset.unwrap().name, "GitAgent-1.2.0-macOS.dmg");
        assert!(asset_name_matches_os("GitAgent-1.2.0-macOS.dmg", "macos"));
        assert!(asset_name_matches_os("GitAgent_1.2.0_amd64.deb", "linux"));
        assert!(!asset_name_matches_os(
            "git-agent-linux-installer.tar.gz",
            "linux"
        ));
    }

    #[test]
    fn update_asset_rejects_untrusted_downloads_and_path_names() {
        let untrusted = ReleaseAsset {
            name: "GitAgentSetup-v1.2.0.exe".to_owned(),
            download_url: "https://example.com/GitAgentSetup-v1.2.0.exe".to_owned(),
            size: 10,
        };
        assert!(validate_release_asset(&untrusted).is_err());

        let invalid_name = ReleaseAsset {
            name: "../GitAgentSetup-v1.2.0.exe".to_owned(),
            download_url: "https://github.com/adoin/git-Agent/releases/download/v1.2.0/GitAgentSetup-v1.2.0.exe".to_owned(),
            size: 10,
        };
        assert!(validate_release_asset(&invalid_name).is_err());
    }
}
