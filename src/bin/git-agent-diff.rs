#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() -> eframe::Result<()> {
    git_agent::diff_tool::DiffToolApp::run_from_env()
}
