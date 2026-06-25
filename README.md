# Git Agent

Git Agent is a desktop Git helper built with Rust and egui.

## Build

```powershell
cargo build --release
```

Release binaries are produced in `target/release/`.

## GitHub Actions

Pushing to `main` runs the `Build` workflow for Linux, macOS, and Windows. Each job runs tests, builds release binaries, and uploads one installer package for that platform.

Each package includes:

- `git-agent`
- `git-agent-merge`
- `install.sh` on Linux/macOS or `install.ps1` on Windows

Run the installer script from the downloaded package to install both executables.

The workflow keeps only the latest 3 build runs and sets uploaded installer artifacts to expire after 3 days, which helps limit GitHub Actions storage usage.

## Support

If this project helps you, you can support it on 爱发电:

https://ifdian.net/a/adoin
