param(
    [string]$Application = "$PSScriptRoot\..\src-tauri\target\debug\PulseBridge Visuals.exe"
)
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$Application = (Resolve-Path $Application).Path
$root = Join-Path $PSScriptRoot "..\src-tauri\target\windows-startup-smoke"
# This opt-in hook exists only in debug builds. Each process uses isolated logs
# and default settings; no saved user settings or diagnostic history are changed.
try {
    foreach ($delay in @(0, 13000)) {
        $directory = Join-Path $root "delay-$delay"
        New-Item -ItemType Directory -Force $directory | Out-Null
        $env:PULSEBRIDGE_SMOKE_AUTOSTART = "1"
        $env:PULSEBRIDGE_SMOKE_GPU_DELAY_MS = "$delay"
        $env:PULSEBRIDGE_SMOKE_LOG_DIR = $directory
        $process = Start-Process -FilePath $Application -PassThru
        if (-not $process.WaitForExit(90000)) {
            $process.Kill()
            throw "Native application did not finish Start/first-frame/Stop within 90 seconds (injected delay $delay ms)."
        }
        $process.WaitForExit()
        $log = Join-Path $directory "pulsebridge.log"
        if (-not (Test-Path $log)) { throw "Native application wrote no diagnostic log" }
        $events = @(Get-Content $log | ForEach-Object { $_ | ConvertFrom-Json })
        $session = ($events | Where-Object code -eq "APP_RUNTIME_START" | Select-Object -Last 1).sessionId
        $events = @($events | Where-Object sessionId -eq $session)
        $events | Where-Object { $_.event -match "renderer|smoke|performance.start|performance.stop" } | ConvertTo-Json -Depth 8
        if ($process.ExitCode -ne 0) { throw "Native application exited $($process.ExitCode)" }
        foreach ($code in @("GPU_SCENE_COMPILED", "GPU_FIRST_FRAME_PRESENTED", "FULLSCREEN_REVEALED", "SMOKE_AUTOSTART_PASS")) {
            if (-not ($events | Where-Object code -eq $code)) { throw "Missing native startup event $code" }
        }
        if ($events | Where-Object { $_.code -eq "WORKER_STOP_TIMEOUT" -or $_.code -eq "GPU_STARTUP_TIMEOUT" }) {
            throw "Native startup or worker cleanup timed out"
        }
        Write-Host "Native Windows Start/first-frame/Stop passed with $delay ms injected delay."
    }
} finally {
    Remove-Item Env:PULSEBRIDGE_SMOKE_AUTOSTART -ErrorAction SilentlyContinue
    Remove-Item Env:PULSEBRIDGE_SMOKE_GPU_DELAY_MS -ErrorAction SilentlyContinue
    Remove-Item Env:PULSEBRIDGE_SMOKE_LOG_DIR -ErrorAction SilentlyContinue
}
