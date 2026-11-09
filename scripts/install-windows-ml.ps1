# Install the reviewed shared runtime for the current user (also used by CI).
$ErrorActionPreference = 'Stop'
$version = '2.6.5'
$minimum = [Version]'2.6.5.0'
$existing = Get-AppxPackage -Name Microsoft.WindowsAppRuntime.2 |
    Where-Object { $_.Architecture -eq 'X64' -and [Version]$_.Version -ge $minimum }
if ($existing) {
    Write-Output "Windows ML framework already installed: $($existing.PackageFullName)"
    # Still install companion packages if needed below; the EP catalog uses them.
}
$work = Join-Path ([IO.Path]::GetTempPath()) ('irodoru-winml-install-' + [Guid]::NewGuid())
try {
    New-Item -ItemType Directory -Path $work | Out-Null
    $archive = Join-Path $work 'runtime.zip'
    Invoke-WebRequest -Uri "https://api.nuget.org/v3-flatcontainer/microsoft.windowsappsdk.runtime/$version/microsoft.windowsappsdk.runtime.$version.nupkg" -OutFile $archive
    $digest = (Get-FileHash -Algorithm SHA256 $archive).Hash
    if ($digest -ne 'B18D0D0ECAE0C62230DC70FCC30F843D3F3A49C806F861D2B1990B2F01EE6352') {
        throw 'Windows App SDK Runtime archive checksum mismatch'
    }
    Expand-Archive $archive -DestinationPath (Join-Path $work 'runtime')
    $packages = Join-Path $work 'runtime/tools/MSIX/win10-x64'
    # Framework before its dependents. Add-AppxPackage verifies package signatures.
    foreach ($file in @(
        'Microsoft.WindowsAppRuntime.2.msix',
        'Microsoft.WindowsAppRuntime.Main.2.msix',
        'Microsoft.WindowsAppRuntime.Singleton.2.msix',
        'Microsoft.WindowsAppRuntime.DDLM.2.msix'
    )) {
        $path = Join-Path $packages $file
        # Avoid attempting to downgrade any already installed companion package.
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $zip = [IO.Compression.ZipFile]::OpenRead($path)
        try {
            $reader = [IO.StreamReader]::new($zip.GetEntry('AppxManifest.xml').Open())
            try { [xml]$manifest = $reader.ReadToEnd() } finally { $reader.Dispose() }
        } finally { $zip.Dispose() }
        $identity = $manifest.Package.Identity
        $installed = Get-AppxPackage -Name $identity.Name | Where-Object {
            $_.Architecture -eq 'X64' -and [Version]$_.Version -ge [Version]$identity.Version
        }
        if (-not $installed) { Add-AppxPackage -Path $path }
    }
} finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
