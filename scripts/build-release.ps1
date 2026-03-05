Param(
    [switch]$VerboseLogs
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
if (-not (Test-Path $cargo)) {
    throw "Cargo not found at '$cargo'. Install Rust from https://rustup.rs"
}

$llvmMingwBin = Join-Path $root ".toolchain\llvm-mingw-20260224-ucrt-x86_64\bin"
$localLinker = Join-Path $llvmMingwBin "x86_64-w64-mingw32-clang.exe"

if (Test-Path $localLinker) {
    if ($VerboseLogs) {
        Write-Host "Using local llvm-mingw linker: $localLinker"
    }
    $env:PATH = "$llvmMingwBin;$env:PATH"
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER = $localLinker
    & $cargo +stable-x86_64-pc-windows-gnullvm build --release
    $builtExe = Join-Path $root "target\x86_64-pc-windows-gnullvm\release\mergen-ade.exe"
} else {
    $link = (Get-Command link.exe -ErrorAction SilentlyContinue)
    if (-not $link) {
        throw "No linker found. Install Visual Studio Build Tools (Desktop C++) or restore local llvm-mingw under .toolchain."
    }
    if ($VerboseLogs) {
        Write-Host "Using MSVC linker from PATH: $($link.Source)"
    }
    & $cargo +stable-x86_64-pc-windows-msvc build --release
    $builtExe = Join-Path $root "target\release\mergen-ade.exe"
}

$exePath = Join-Path $root "target\release\mergen-ade.exe"
if (-not (Test-Path $builtExe)) {
    throw "Build completed but output EXE not found at '$builtExe'."
}

New-Item -ItemType Directory -Path (Split-Path -Parent $exePath) -Force | Out-Null
Copy-Item -Path $builtExe -Destination $exePath -Force

$exePath = Join-Path $root "target\release\mergen-ade.exe"
if (-not (Test-Path $exePath)) {
    throw "Build completed but EXE not found at '$exePath'."
}

Write-Host "OK: $exePath"
