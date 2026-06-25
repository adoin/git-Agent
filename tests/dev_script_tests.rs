use std::fs;

#[test]
fn dev_script_stops_running_app_before_building_bins() {
    let script_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("dev.ps1");
    let script = fs::read_to_string(script_path).expect("dev.ps1 should be readable");
    let restart_start = script
        .find("function Restart-DevApp")
        .expect("Restart-DevApp should exist");
    let restart_body = &script[restart_start..];
    let stop_index = restart_body
        .find("Stop-DevBinaries")
        .expect("Restart-DevApp should stop running debug binaries");
    let build_index = restart_body
        .find("Build-Bins")
        .expect("Restart-DevApp should build bins");

    assert!(
        stop_index < build_index,
        "Windows locks a running exe, so dev.ps1 must stop git-agent.exe before cargo build --bins"
    );

    let stop_bins_start = script
        .find("function Stop-DevBinaries")
        .expect("Stop-DevBinaries should exist");
    let stop_bins_end = script[stop_bins_start..]
        .find("function Build-Bins")
        .expect("Stop-DevBinaries should end before Build-Bins")
        + stop_bins_start;
    let stop_bins_body = &script[stop_bins_start..stop_bins_end];

    assert!(stop_bins_body.contains("Stop-MainWindow"));
    assert!(stop_bins_body.contains("\"git-agent-merge\""));
}
