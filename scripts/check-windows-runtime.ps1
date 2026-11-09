param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [switch]$Gpu,
    [switch]$EmbeddedCpu
)
$ErrorActionPreference = 'Stop'
if ($Gpu -and $EmbeddedCpu) { throw 'Choose GPU or embedded CPU verification' }
if (-not [Environment]::Is64BitProcess) { throw 'Run with 64-bit PowerShell' }
$Executable = (Resolve-Path $Executable).Path
$system = [Environment]::SystemDirectory
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$dumpbin = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -find 'VC/Tools/MSVC/**/bin/Hostx64/x64/dumpbin.exe' | Select-Object -First 1
if (-not $dumpbin) { throw 'MSVC dumpbin.exe is required to inspect dependencies' }
$imports = & $dumpbin /dependents $Executable
if ($LASTEXITCODE -ne 0) { throw 'dumpbin failed' }
$dependencies = @($imports | ForEach-Object {
    if ($_ -match '^\s+([\w.-]+\.dll)\s*$') { $Matches[1] }
} | Sort-Object -Unique)
if ($dependencies.Count -eq 0) { throw 'No PE dependencies found' }
foreach ($name in $dependencies) {
    if ($name -match '^(onnxruntime|DirectML|Microsoft.Windows.AI.MachineLearning|vcruntime.*|msvcp.*)\.dll$') {
        throw "Application must not import redistributable runtime DLLs: $name"
    }
}
foreach ($name in $dependencies) {
    # API sets are virtual OS contracts; other imports must be installed in
    # System32. Windows ML is loaded separately from its registered package.
    if ($name -notmatch '^(api|ext)-ms-' -and -not (Test-Path (Join-Path $system $name))) {
        throw "Dependency is not installed in System32: $name"
    }
}
Write-Output "PE dependencies: $($dependencies -join ', ')"
$work = Join-Path ([IO.Path]::GetTempPath()) ('irodoru-runtime-' + [Guid]::NewGuid())
$originalPath = $env:PATH
$process = $null
try {
    New-Item -ItemType Directory -Path $work | Out-Null
    $binary = Join-Path $work 'irodoru.exe'
    $report = Join-Path $work 'report.txt'
    Copy-Item $Executable $binary
    # Copy only the exe, and keep build/dependency directories out of DLL search.
    $env:PATH = $system
    $arguments = @('--verify-windows-ml', "`"$report`"")
    if ($Gpu) { $arguments += '--gpu' }
    if ($EmbeddedCpu) { $arguments += '--embedded-cpu' }
    $process = Start-Process -FilePath $binary -ArgumentList $arguments -WorkingDirectory $work -PassThru
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
    $env:PATH = $originalPath
    if ($process -and -not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
