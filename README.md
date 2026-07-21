# Git Agent

Git Agent is a desktop Git helper built with Rust and egui.

## Build

```powershell
cargo build --release
```

Release binaries are produced in `target/release/`.

## Local Development

Start the local watcher and desktop app from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\dev.ps1
```

## GitHub Actions

Pushing a `v*` tag runs the `Build` workflow for Linux, macOS, and Windows. Each job runs tests, builds release binaries, and uploads one installer package for that platform.

Linux and macOS releases are published as tarballs with `git-agent`, `git-agent-merge`, and `install.sh`.

Windows releases are published as `GitAgentSetup-<version>.exe`. The setup wizard lets you choose the install path and installs both executables.

User data is stored relative to the executable in `data/`, for example:

```text
<install path>/data/config.json
<install path>/data/tabs.json
<install path>/data/layout.json
<install path>/data/stores/<repository-hash>/snapshot.json
<install path>/data/stores/<repository-hash>/commit-options.json
```

The workflow keeps only the latest 3 build runs and sets uploaded installer artifacts to expire after 3 days, which helps limit GitHub Actions storage usage.

## Release

Create and push a version tag to publish a GitHub Release with Linux, macOS, and Windows installer packages:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

The release assets are generated automatically from the workflow build outputs.

## Support

If this project helps you, you can support it on 爱发电:

https://ifdian.net/a/adoin
