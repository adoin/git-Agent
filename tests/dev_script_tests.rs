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

    let stop_main_start = script
        .find("function Stop-MainWindow")
        .expect("Stop-MainWindow should exist");
    let stop_main_end = script[stop_main_start..]
        .find("function Stop-DevBinaries")
        .expect("Stop-MainWindow should end before Stop-DevBinaries")
        + stop_main_start;
    let stop_main_body = &script[stop_main_start..stop_main_end];
    assert!(
        stop_main_body.contains("Write-DevLog \"stop git-agent pid=$($script:mainProcess.Id)\"")
    );
    assert!(stop_main_body.contains("Write-DevLog \"stop stray git-agent pid=$($_.Id)\""));
}

#[test]
fn dev_script_logs_started_app_pid_and_exit_code() {
    let script_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("dev.ps1");
    let script = fs::read_to_string(script_path).expect("dev.ps1 should be readable");

    assert!(script.contains("function Test-MainWindowExit"));

    let start_start = script
        .find("function Start-MainWindow")
        .expect("Start-MainWindow should exist");
    let start_end = script[start_start..]
        .find("function Restart-DevApp")
        .expect("Start-MainWindow should end before Restart-DevApp")
        + start_start;
    let start_body = &script[start_start..start_end];
    assert!(start_body.contains("Write-DevLog \"started git-agent pid=$($process.Id)\""));

    let exit_start = script
        .find("function Test-MainWindowExit")
        .expect("Test-MainWindowExit should exist");
    let exit_end = script[exit_start..]
        .find("function Restart-DevApp")
        .expect("Test-MainWindowExit should end before Restart-DevApp")
        + exit_start;
    let exit_body = &script[exit_start..exit_end];
    assert!(exit_body.contains("Write-DevLog \"git-agent exited pid=$processId exit=$exitCode\""));

    let loop_start = script
        .find("while ($true)")
        .expect("watch loop should exist");
    let loop_body = &script[loop_start..];
    assert!(loop_body.contains("Test-MainWindowExit"));
}
