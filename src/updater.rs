use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use semver::Version;
use serde::Deserialize;

pub const REPOSITORY_URL: &str = "https://github.com/adoin/git-Agent";
pub const RELEASE_PAGE_URL: &str = "https://github.com/adoin/git-Agent/releases";
const LATEST_RELEASE_API: &str = "https://api.github.com/repos/adoin/git-Agent/releases/latest";
const TRUSTED_DOWNLOAD_PREFIX: &str = "https://github.com/adoin/git-Agent/releases/download/";
const USER_AGENT: &str = "Git-Agent-Updater";

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
    let response = ureq::get(LATEST_RELEASE_API)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|error| anyhow!("GitHub release request failed: {error}"))?;
    let release: GithubRelease = response
        .into_json()
        .context("GitHub release response was not valid JSON")?;
    release_from_response(current_version, release, std::env::consts::OS)
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
    let expected = match os {
        "windows" => None,
        "linux" => Some("git-agent-linux-installer.tar.gz"),
        "macos" => Some("git-agent-macos-installer.tar.gz"),
        _ => return None,
    };
    if let Some(expected) = expected {
        return assets.iter().find(|asset| asset.name == expected);
    }
    assets
        .iter()
        .find(|asset| asset.name.starts_with("GitAgentSetup-") && asset.name.ends_with(".exe"))
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
    let valid_platform_name = match std::env::consts::OS {
        "windows" => asset.name.starts_with("GitAgentSetup-") && asset.name.ends_with(".exe"),
        "linux" => asset.name == "git-agent-linux-installer.tar.gz",
        "macos" => asset.name == "git-agent-macos-installer.tar.gz",
        _ => false,
    };
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn install_downloaded_asset(path: &Path, directory: &Path) -> Result<InstallOutcome> {
    let extract_status = Command::new("tar")
        .arg("-xzf")
        .arg(path)
        .arg("-C")
        .arg(directory)
        .status()
        .context("Unable to start tar for update extraction")?;
    if !extract_status.success() {
        bail!("Unable to extract update package");
    }

    let package_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".tar.gz"))
        .context("Update package name is invalid")?;
    let package_directory = directory.join(package_name);
    let installer = package_directory.join("install.sh");
    let install_status = Command::new("sh")
        .arg(&installer)
        .current_dir(&package_directory)
        .status()
        .with_context(|| format!("Unable to start installer: {}", installer.display()))?;
    if !install_status.success() {
        bail!("Update installer returned an error");
    }
    Ok(InstallOutcome::Installed)
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn install_downloaded_asset(_path: &Path, _directory: &Path) -> Result<InstallOutcome> {
    bail!("Automatic updates are not supported on this operating system")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release_json() -> GithubRelease {
        serde_json::from_str(
            r#"{
                "tag_name":"v1.2.0",
                "name":"Git Agent v1.2.0",
                "body":"Release notes",
                "html_url":"https://github.com/adoin/git-Agent/releases/tag/v1.2.0",
                "assets":[
                    {"name":"git-agent-linux-installer.tar.gz","browser_download_url":"https://github.com/adoin/git-Agent/releases/download/v1.2.0/git-agent-linux-installer.tar.gz","size":10},
                    {"name":"git-agent-macos-installer.tar.gz","browser_download_url":"https://github.com/adoin/git-Agent/releases/download/v1.2.0/git-agent-macos-installer.tar.gz","size":20},
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
        assert_eq!(
            linux.asset.unwrap().name,
            "git-agent-linux-installer.tar.gz"
        );

        let macos = release_from_response("1.3.0", release_json(), "macos").unwrap();
        assert!(!macos.is_newer);
        assert_eq!(
            macos.asset.unwrap().name,
            "git-agent-macos-installer.tar.gz"
        );
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
