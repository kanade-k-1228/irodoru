param([string]$Output = "$PSScriptRoot/../target/cpu-ort")
$ErrorActionPreference = 'Stop'
# Matches the native notices already shipped in licenses/native/.
$commit = '402979ac1bef89dcd55add41eaa7277c407599eb' # ONNX Runtime 1.31.0
$Output = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Force $Output | Out-Null
$source = Join-Path $Output 'source'
if (-not (Test-Path "$source/.git")) {
    git clone --filter=blob:none --no-checkout https://github.com/microsoft/onnxruntime.git $source
    if ($LASTEXITCODE -ne 0) { throw 'ORT clone failed' }
}
git -C $source checkout --detach $commit
if ($LASTEXITCODE -ne 0) { throw 'ORT checkout failed' }
git -C $source submodule update --init --recursive
if ($LASTEXITCODE -ne 0) { throw 'ORT submodule checkout failed' }
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$version = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion
if (-not $version) { throw 'Visual Studio C++ tools are required' }
$generator = switch (([Version]$version).Major) {
    17 { 'Visual Studio 17 2022' }
    18 { 'Visual Studio 18 2026' }
    default { throw "Unsupported Visual Studio version: $version" }
}
# No DirectML, shared libraries, reduced operator set, or dynamic CRT.
& "$source/build.bat" --cmake_generator $generator --config Release --parallel 2 --skip_tests --build_dir "$Output/build" --enable_msvc_static_runtime --cmake_extra_defines onnxruntime_BUILD_UNIT_TESTS=OFF onnxruntime_USE_MIMALLOC=OFF onnxruntime_USE_FULL_PROTOBUF=OFF
if ($LASTEXITCODE -ne 0) { throw 'CPU ORT build failed' }
# RE2 is excluded from ALL by upstream FetchContent. In a static-only build
# there is no final ORT shared-library link to pull this target into the build.
cmake --build "$Output/build/Release" --config Release --target re2 --parallel 2
if ($LASTEXITCODE -ne 0) { throw 'ORT RE2 dependency build failed' }
$lib = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -find 'VC/Tools/MSVC/**/bin/Hostx64/x64/lib.exe' | Select-Object -First 1
if (-not $lib) { throw 'MSVC lib.exe is required' }
# Flatten the built static libraries including transitive dependencies. Tests
# are disabled; exclude compiler probes and the full protobuf/protoc libraries
# built for the host code generator (the runtime uses protobuf-lite).
$libraries = @(Get-ChildItem "$Output/build/Release" -Recurse -Filter '*.lib' |
    Where-Object { $_.FullName -notmatch '[\\/](CMakeFiles|CompilerIdCXX|CompilerIdC)[\\/]' -and $_.Name -notin @('libprotobuf.lib', 'libprotoc.lib') } |
    Sort-Object FullName)
foreach ($required in @('onnxruntime_session.lib', 're2.lib')) {
    if ($libraries.Name -notcontains $required) { throw "ORT static dependency missing: $required" }
}
$destination = Join-Path $Output 'lib'
New-Item -ItemType Directory -Force $destination | Out-Null
$response = Join-Path $Output 'libraries.rsp'
@('/NOLOGO', "/OUT:`"$destination/irodoru_ort_cpu.lib`"") + @($libraries | ForEach-Object { '"' + $_.FullName + '"' }) |
    Set-Content -Encoding utf8 $response
& $lib "@$response"
if ($LASTEXITCODE -ne 0) { throw 'Combining CPU ORT libraries failed' }
Write-Output "Set IRODORU_CPU_ORT_LIB_DIR=$destination before building the app."
