$ErrorActionPreference = "Stop"

$link = Get-Command link.exe -ErrorAction SilentlyContinue
if ($link) {
    Write-Host "MSVC linker already available: $($link.Source)"
    exit 0
}

Write-Host "MSVC Build Tools are missing."
Write-Host "Install with:"
Write-Host "  winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override ""--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"""
Write-Host ""
Write-Host "After install, open 'x64 Native Tools Command Prompt for VS 2022' and run:"
Write-Host "  cargo +stable-x86_64-pc-windows-msvc build --release"

