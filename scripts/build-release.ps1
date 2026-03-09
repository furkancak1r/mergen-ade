$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$target = "x86_64-pc-windows-msvc"
$releaseDir = Join-Path $repoRoot "target\$target\release"
$exePath = Join-Path $releaseDir "mergen-ade.exe"
$toolchain = "stable-x86_64-pc-windows-msvc"
$blockedDlls = @(
    "libunwind.dll",
    "libwinpthread-1.dll",
    "libgcc_s_seh-1.dll",
    "libstdc++-6.dll",
    "libc++.dll",
    "vcruntime140.dll",
    "vcruntime140_1.dll",
    "msvcp140.dll"
)

function Resolve-Rustup {
    $rustup = Get-Command rustup -ErrorAction SilentlyContinue
    if ($rustup) {
        return $rustup.Source
    }

    $fallback = Join-Path $env:USERPROFILE ".cargo\bin\rustup.exe"
    if (Test-Path $fallback) {
        return $fallback
    }

    throw "Portable MSVC release icin rustup gerekli. 'rustup run $toolchain cargo ...' cagrisi bulunamadi."
}

function Ensure-MsvcToolchain {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RustupPath
    )

    $installed = & $RustupPath toolchain list
    if ($LASTEXITCODE -ne 0) {
        throw "Yuklu Rust toolchain listesi okunamadi."
    }

    if ($installed -match [regex]::Escape($toolchain)) {
        return
    }

    Write-Host "MSVC host toolchain bulunamadi; yukleniyor: $toolchain"
    & $RustupPath toolchain install $toolchain --profile minimal
    if ($LASTEXITCODE -ne 0) {
        throw "MSVC host toolchain yuklenemedi: $toolchain"
    }
}

function Resolve-Dumpbin {
    $dumpbin = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
    if ($dumpbin) {
        return $dumpbin.Source
    }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $installationPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($LASTEXITCODE -eq 0 -and $installationPath) {
            $dumpbinCandidate = Get-ChildItem -Path (Join-Path $installationPath "VC\Tools\MSVC") -Recurse -Filter dumpbin.exe -ErrorAction SilentlyContinue |
                Sort-Object FullName -Descending |
                Select-Object -First 1 -ExpandProperty FullName
            if ($dumpbinCandidate) {
                return $dumpbinCandidate
            }
        }
    }

    $visualStudioRoots = @(
        (Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio"),
        (Join-Path ${env:ProgramFiles} "Microsoft Visual Studio")
    ) | Where-Object { $_ -and (Test-Path $_) }

    foreach ($root in $visualStudioRoots) {
        $dumpbinCandidate = Get-ChildItem -Path $root -Recurse -Filter dumpbin.exe -ErrorAction SilentlyContinue |
            Sort-Object FullName -Descending |
            Select-Object -First 1 -ExpandProperty FullName
        if ($dumpbinCandidate) {
            return $dumpbinCandidate
        }
    }

    return $null
}

function Get-ImportedDllNames {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ExecutablePath
    )

    $llvmObjdumpError = $null
    $llvmObjdump = Join-Path $repoRoot ".toolchain\llvm-mingw-20260224-ucrt-x86_64\bin\llvm-objdump.exe"
    if (Test-Path $llvmObjdump) {
        try {
            $output = & $llvmObjdump -p $ExecutablePath 2>&1 | Out-String
            if ($LASTEXITCODE -eq 0) {
                $imports = [regex]::Matches($output, "DLL Name:\s+([^\r\n]+)") |
                    ForEach-Object { $_.Groups[1].Value.ToLowerInvariant() } |
                    Sort-Object -Unique
                if ($imports) {
                    return $imports
                }
            }

            $llvmObjdumpError = "Repo-local llvm-objdump.exe import listesi uretemedi."
        }
        catch {
            $llvmObjdumpError = $_.Exception.Message
        }
    }

    $dumpbin = Resolve-Dumpbin
    if ($dumpbin) {
        $output = & $dumpbin /dependents $ExecutablePath 2>&1 | Out-String
        if ($LASTEXITCODE -eq 0) {
            return [regex]::Matches($output, "(?im)^\s+([a-z0-9._-]+\.dll)\s*$") |
                ForEach-Object { $_.Groups[1].Value.ToLowerInvariant() } |
                Sort-Object -Unique
        }
    }

    if ($llvmObjdumpError) {
        throw "Bağımlılık kontrolü başarısız oldu. llvm-objdump hata verdi ve dumpbin kullanılamadı: $llvmObjdumpError"
    }

    throw "Bağımlılık kontrol aracı bulunamadı. Repo-local llvm-objdump.exe veya Visual Studio dumpbin.exe çözümlenemedi."
}

Push-Location $repoRoot
try {
    $rustup = Resolve-Rustup
    Ensure-MsvcToolchain -RustupPath $rustup

    & $rustup run $toolchain cargo build --release --target $target -j 1
    if ($LASTEXITCODE -ne 0) {
        throw "Portable release build başarısız oldu."
    }

    if (-not (Test-Path $exePath)) {
        throw "Portable release EXE bulunamadı: $exePath"
    }

    $imports = Get-ImportedDllNames -ExecutablePath $exePath
    if ($LASTEXITCODE -ne 0) {
        throw "Portable release bağımlılık kontrolü başarısız oldu."
    }
    $blockedImports = $imports | Where-Object { $blockedDlls -contains $_ }
    if ($blockedImports) {
        $joined = ($blockedImports | Sort-Object -Unique) -join ", "
        throw "Portable EXE hala ek runtime DLL'lerine bağlı: $joined"
    }

    Write-Host "Portable release EXE ready: $exePath"
}
finally {
    Pop-Location
}
