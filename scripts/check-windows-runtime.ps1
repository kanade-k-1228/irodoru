param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [switch]$Gpu,
    [switch]$EmbeddedCpu
)
# Run the app's runtime probe (a tiny Add graph on the selected ONNX Runtime)
# from a copy of only the executable, so it cannot rely on app-local DLLs.
$ErrorActionPreference = 'Stop'
if ($Gpu -and $EmbeddedCpu) { throw 'Choose GPU or embedded CPU verification' }
$Executable = (Resolve-Path $Executable).Path
$work = Join-Path ([IO.Path]::GetTempPath()) ('irodoru-runtime-' + [Guid]::NewGuid())
$process = $null
try {
    New-Item -ItemType Directory -Path $work | Out-Null
    $binary = Join-Path $work 'irodoru.exe'
    $report = Join-Path $work 'report.txt'
    Copy-Item $Executable $binary
    $arguments = @('--verify-windows-ml', "`"$report`"")
    if ($Gpu) { $arguments += '--gpu' }
    if ($EmbeddedCpu) { $arguments += '--embedded-cpu' }
    $process = Start-Process -FilePath $binary -ArgumentList $arguments -WorkingDirectory $work -PassThru
    # GPU mode may download an execution provider.
    $timeout = if ($Gpu) { 600000 } else { 60000 }
    if (-not $process.WaitForExit($timeout)) { throw 'Runtime check timed out' }
    $process.Refresh()
    if (Test-Path $report) { Get-Content $report }
    if ($process.ExitCode -ne 0) { throw "Runtime check failed: exit $($process.ExitCode)" }
    if (-not (Test-Path $report)) { throw 'Runtime check did not write a report' }
    $text = Get-Content -Raw $report
    if ($EmbeddedCpu -and $text -notmatch 'Windows ML: embedded CPU') { throw 'Embedded CPU was not used' }
    if (-not $EmbeddedCpu -and $text -match 'Windows ML: embedded CPU') { throw 'Shared Windows ML was not used' }
} finally {
    if ($process -and -not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
