param(
    [int]$DebounceMs = 700,
    [switch]$KeepExisting,
    [switch]$LayoutDebug
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $root

$stateDir = Join-Path $root "target\dev-watch"
$pidFile = Join-Path $stateDir "runner.pid"
$stdoutLog = Join-Path $stateDir "dev-watch.out.log"
$stderrLog = Join-Path $stateDir "dev-watch.err.log"
$mainExe = Join-Path $root "target\debug\git-agent.exe"
$mergeExe = Join-Path $root "target\debug\git-agent-merge.exe"

New-Item -ItemType Directory -Force -Path $stateDir | Out-Null

function Write-DevLog {
    param([string]$Message)
    $line = "[dev] $(Get-Date -Format o) $Message"
    Write-Host $line
    $line | Add-Content -Path $stdoutLog
}

function Stop-ProcessTree {
    param([int]$ProcessId)

    $children = Get-CimInstance Win32_Process -Filter "ParentProcessId=$ProcessId" -ErrorAction SilentlyContinue
    foreach ($child in $children) {
        Stop-ProcessTree -ProcessId $child.ProcessId
    }

    $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    if ($process) {
        Stop-Process -Id $ProcessId -Force -ErrorAction SilentlyContinue
    }
}

function Stop-ExistingRunner {
    if ($KeepExisting -or -not (Test-Path $pidFile)) {
        return
    }

    $oldPidText = Get-Content -Path $pidFile -ErrorAction SilentlyContinue | Select-Object -First 1
    [int]$oldPid = 0
    if ([int]::TryParse($oldPidText, [ref]$oldPid) -and $oldPid -gt 0 -and $oldPid -ne $PID) {
        Write-DevLog "stop old dev runner pid=$oldPid"
        Stop-ProcessTree -ProcessId $oldPid
    }

    Remove-Item -Path $pidFile -Force -ErrorAction SilentlyContinue
}

function Stop-MainWindow {
    if ($script:mainProcess -and -not $script:mainProcess.HasExited) {
        Stop-Process -Id $script:mainProcess.Id -Force -ErrorAction SilentlyContinue
    }

    Get-Process -Name "git-agent" -ErrorAction SilentlyContinue |
        Stop-Process -Force -ErrorAction SilentlyContinue
}

function Stop-DevBinaries {
    Stop-MainWindow

    Get-Process -Name "git-agent-merge" -ErrorAction SilentlyContinue |
        Stop-Process -Force -ErrorAction SilentlyContinue
}

function Build-Bins {
    Write-DevLog "cargo build --bins"
    Push-Location $root
    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        & cargo build --bins >> $stdoutLog 2>> $stderrLog
        $buildExitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
        Pop-Location
    }

    if ($buildExitCode -ne 0) {
        $line = "[dev] $(Get-Date -Format o) build failed: exit $buildExitCode"
        Write-Host $line -ForegroundColor Red
        $line | Add-Content -Path $stderrLog
        return $false
    }

    Write-DevLog "built main=$mainExe"
    Write-DevLog "built merge=$mergeExe"
    return $true
}

function Start-MainWindow {
    if (-not (Test-Path $mainExe)) {
        Write-DevLog "main exe missing; skip start"
        return $null
    }

    if ($LayoutDebug) {
        $env:GIT_AGENT_LAYOUT_DEBUG = "1"
        Write-DevLog "layout debug enabled"
    }
    else {
        Remove-Item Env:\GIT_AGENT_LAYOUT_DEBUG -ErrorAction SilentlyContinue
    }

    Write-DevLog "start $mainExe"
    return Start-Process `
        -FilePath $mainExe `
        -WorkingDirectory $root `
        -PassThru
}

function Restart-DevApp {
    Stop-DevBinaries
    if (Build-Bins) {
        $script:mainProcess = Start-MainWindow
    }
}

Stop-ExistingRunner
Set-Content -Path $pidFile -Value $PID

$watcher = New-Object System.IO.FileSystemWatcher
$watcher.Path = $root
$watcher.IncludeSubdirectories = $true
$watcher.EnableRaisingEvents = $true
$watcher.Filter = "*.*"

$lastRestart = Get-Date "2000-01-01"
$script:mainProcess = $null

try {
    Restart-DevApp
    Write-Host "[dev] watching src/, assets/, Cargo.toml, Cargo.lock. Ctrl+C to stop." -ForegroundColor Green
    Write-Host "[dev] logs: $stdoutLog ; $stderrLog" -ForegroundColor DarkGray

    while ($true) {
        $change = $watcher.WaitForChanged("Changed, Created, Deleted, Renamed", 1000)
        if ($change.TimedOut) {
            continue
        }

        $path = $change.Name -replace "/", "\"
        $isWatched =
            $path -like "src\*" -or
            $path -like "assets\*" -or
            $path -eq "Cargo.toml" -or
            $path -eq "Cargo.lock"

        if (-not $isWatched) {
            continue
        }

        $now = Get-Date
        if (($now - $lastRestart).TotalMilliseconds -lt $DebounceMs) {
            continue
        }

        $lastRestart = $now
        Write-Host "[dev] change: $path" -ForegroundColor DarkGray
        Restart-DevApp
    }
}
finally {
    $watcher.Dispose()
    Stop-MainWindow

    $currentPidText = Get-Content -Path $pidFile -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($currentPidText -eq "$PID") {
        Remove-Item -Path $pidFile -Force -ErrorAction SilentlyContinue
    }
}
