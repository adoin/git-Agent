param(
    [int]$DebounceMs = 700
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $root
$logDir = Join-Path $root "target"
$stdoutLog = Join-Path $logDir "dev-watch.out.log"
$stderrLog = Join-Path $logDir "dev-watch.err.log"
$appExe = Join-Path $root "target\debug\git-agent.exe"
New-Item -ItemType Directory -Force -Path $logDir | Out-Null

function Stop-GitAgent {
    if ($script:appProcess -and -not $script:appProcess.HasExited) {
        Stop-Process -Id $script:appProcess.Id -Force -ErrorAction SilentlyContinue
    }
    Get-Process -Name "git-agent" -ErrorAction SilentlyContinue | Stop-Process -Force
}

function Start-GitAgent {
    Stop-GitAgent
    "[dev] $(Get-Date -Format o) cargo build" | Add-Content -Path $stdoutLog
    Push-Location $root
    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        & cargo build >> $stdoutLog 2>> $stderrLog
        $buildExitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
        Pop-Location
    }

    if ($buildExitCode -ne 0) {
        "[dev] $(Get-Date -Format o) build failed: $buildExitCode" | Add-Content -Path $stderrLog
        return $null
    }

    "[dev] $(Get-Date -Format o) start $appExe" | Add-Content -Path $stdoutLog
    $process = Start-Process `
        -FilePath $appExe `
        -WorkingDirectory $root `
        -PassThru
    return $process
}

$watcher = New-Object System.IO.FileSystemWatcher
$watcher.Path = $root
$watcher.IncludeSubdirectories = $true
$watcher.EnableRaisingEvents = $true
$watcher.Filter = "*.*"

$lastRestart = Get-Date "2000-01-01"
$script:appProcess = Start-GitAgent

try {
    Write-Host "[dev] watching src/, assets/, Cargo.toml, Cargo.lock. Ctrl+C to stop." -ForegroundColor Green
    while ($true) {
        $change = $watcher.WaitForChanged("Changed, Created, Deleted, Renamed", 1000)
        if ($change.TimedOut) {
            if ($script:appProcess -and $script:appProcess.HasExited) {
                Write-Host "[dev] app exited; waiting for next file change." -ForegroundColor Yellow
            }
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
        $script:appProcess = Start-GitAgent
    }
}
finally {
    $watcher.Dispose()
    Stop-GitAgent
}
