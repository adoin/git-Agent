#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() -> eframe::Result<()> {
    git_agent::merge_tool::MergeToolApp::run_from_env()
}
