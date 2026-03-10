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

$script:VsInstallations = $null

function Resolve-RustupCommand {
    $candidates = @(
        (Get-Command rustup -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source),
        (Get-Command rustup.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source),
        (Get-Command rustup.cmd -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source),
        (Join-Path $env:USERPROFILE ".cargo\bin\rustup.exe"),
        (Join-Path $env:USERPROFILE ".cargo\bin\rustup.cmd")
    ) | Where-Object { $_ -and (Test-Path $_) }

    return $candidates | Select-Object -First 1
}

function Ensure-MsvcToolchain {
    $cargoPath = Resolve-MsvcToolBinary -BinaryName "cargo" -AllowMissing
    $rustcPath = Resolve-MsvcToolBinary -BinaryName "rustc" -AllowMissing
    if ((Test-Path $cargoPath) -and (Test-Path $rustcPath)) {
        return
    }

    $rustup = Resolve-RustupCommand
    if (-not $rustup) {
        throw "Portable MSVC release requires rustup so the '$toolchain' toolchain can be installed."
    }

    Write-Host "Installing missing Rust toolchain: $toolchain"
    & $rustup toolchain install $toolchain --profile minimal
    if ($LASTEXITCODE -ne 0) {
        throw "Rust toolchain install failed: $toolchain"
    }
}

function Resolve-MsvcToolBinary {
    param(
        [Parameter(Mandatory = $true)]
        [string]$BinaryName,
        [switch]$AllowMissing
    )

    $rustup = Resolve-RustupCommand
    if ($rustup) {
        $resolved = & $rustup which $BinaryName --toolchain $toolchain 2>$null
        if ($LASTEXITCODE -eq 0 -and $resolved) {
            $candidate = ($resolved | Select-Object -First 1).Trim()
            if ($candidate -and (Test-Path $candidate)) {
                return $candidate
            }
        }
    }

    $rustupHome = $env:RUSTUP_HOME
    if ([string]::IsNullOrWhiteSpace($rustupHome)) {
        $rustupHome = Join-Path $env:USERPROFILE ".rustup"
    }

    $fallback = Join-Path $rustupHome "toolchains\$toolchain\bin\$BinaryName.exe"
    if (Test-Path $fallback) {
        return $fallback
    }

    if ($AllowMissing) {
        return $fallback
    }

    throw "MSVC $BinaryName.exe was not found for toolchain '$toolchain'."
}

function Resolve-MsvcCargo {
    return Resolve-MsvcToolBinary -BinaryName "cargo"
}

function Get-VsInstallations {
    if ($null -ne $script:VsInstallations) {
        return $script:VsInstallations
    }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) {
        $script:VsInstallations = @()
        return $script:VsInstallations
    }

    $json = & $vswhere -all -products * -format json | Out-String
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($json)) {
        $script:VsInstallations = @()
        return $script:VsInstallations
    }

    $parsed = ConvertFrom-Json -InputObject $json
    if ($parsed -is [System.Array]) {
        $script:VsInstallations = $parsed
    }
    elseif ($parsed) {
        $script:VsInstallations = @($parsed)
    }
    else {
        $script:VsInstallations = @()
    }

    return $script:VsInstallations
}

function Get-VsInstallationPriority {
    param(
        [Parameter(Mandatory = $true)]
        [object]$Installation
    )

    $isBuildTools = $Installation.productId -eq "Microsoft.VisualStudio.Product.BuildTools"
    $isComplete = $Installation.isComplete -eq $true

    if ($isBuildTools -and $isComplete) {
        return 0
    }

    if ($isComplete) {
        return 1
    }

    if ($isBuildTools) {
        return 2
    }

    return 3
}

function Get-VsInstallationVersionSortKey {
    param(
        [Parameter(Mandatory = $true)]
        [object]$Installation
    )

    $parsedVersion = $null
    if ([version]::TryParse([string]$Installation.installationVersion, [ref]$parsedVersion)) {
        return $parsedVersion
    }

    return [version]"0.0.0.0"
}

function Sort-VsInstallations {
    param(
        [Parameter(Mandatory = $true)]
        [object[]]$Installations
    )

    return @($Installations) |
        Sort-Object -Property `
            @{ Expression = { Get-VsInstallationPriority -Installation $_ }; Descending = $false },
            @{ Expression = { Get-VsInstallationVersionSortKey -Installation $_ }; Descending = $true }
}

function Get-OrderedVsInstallations {
    return Sort-VsInstallations -Installations @(Get-VsInstallations)
}

function Resolve-MsvcX64LinkerPathFromToolRoot {
    param(
        [AllowEmptyString()]
        [string]$ToolRoot
    )

    if ([string]::IsNullOrWhiteSpace($ToolRoot)) {
        return $null
    }

    $preferredCandidates = @(
        (Join-Path $ToolRoot "bin\Hostx64\x64\link.exe"),
        (Join-Path $ToolRoot "bin\Hostx86\x64\link.exe"),
        (Join-Path $ToolRoot "bin\Hostarm64\x64\link.exe")
    )

    foreach ($candidate in $preferredCandidates) {
        if (Test-Path $candidate) {
            return $candidate
        }
    }

    return $null
}

function Test-MsvcToolRootIsBuildReady {
    param(
        [AllowEmptyString()]
        [string]$ToolRoot
    )

    if ([string]::IsNullOrWhiteSpace($ToolRoot)) {
        return $false
    }

    $requiredPaths = @(
        (Resolve-MsvcX64LinkerPathFromToolRoot -ToolRoot $ToolRoot),
        (Join-Path $ToolRoot "lib\x64\vcruntime.lib"),
        (Join-Path $ToolRoot "include\vcruntime.h")
    )

    return (@($requiredPaths | Where-Object { [string]::IsNullOrWhiteSpace($_) -or -not (Test-Path $_) }).Count -eq 0)
}

function Resolve-MsvcToolRootForInstall {
    param(
        [AllowEmptyString()]
        [string]$InstallationPath
    )

    if ([string]::IsNullOrWhiteSpace($InstallationPath)) {
        return $null
    }

    $toolRoots = @(
        Get-ChildItem -Path (Join-Path $InstallationPath "VC\Tools\MSVC") -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending
    )

    foreach ($toolRoot in $toolRoots) {
        if (Test-MsvcToolRootIsBuildReady -ToolRoot $toolRoot.FullName) {
            return $toolRoot.FullName
        }
    }

    return $null
}

function Resolve-VsInstallRootFromVsDevCmdPath {
    param(
        [AllowEmptyString()]
        [string]$VsDevCmdPath
    )

    if ([string]::IsNullOrWhiteSpace($VsDevCmdPath)) {
        return $null
    }

    $toolsDir = Split-Path -Parent $VsDevCmdPath
    if (-not $toolsDir) {
        return $null
    }

    $common7Dir = Split-Path -Parent $toolsDir
    if (-not $common7Dir) {
        return $null
    }

    return Split-Path -Parent $common7Dir
}

function Resolve-VsDevCmdCandidates {
    $resolvedCandidates = @()
    $orderedInstalls = Get-OrderedVsInstallations

    foreach ($install in $orderedInstalls) {
        if (-not $install.installationPath) {
            continue
        }

        $candidate = Join-Path $install.installationPath "Common7\Tools\VsDevCmd.bat"
        $msvcToolRoot = Resolve-MsvcToolRootForInstall -InstallationPath $install.installationPath
        if ($msvcToolRoot -and (Test-Path $candidate) -and ($resolvedCandidates -notcontains $candidate)) {
            $resolvedCandidates += $candidate
        }
    }

    $roots = @(
        (Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio"),
        (Join-Path ${env:ProgramFiles} "Microsoft Visual Studio")
    ) | Where-Object { $_ -and (Test-Path $_) }

    foreach ($root in $roots) {
        $filesystemCandidates = @(
            Get-ChildItem -Path $root -Recurse -Filter VsDevCmd.bat -ErrorAction SilentlyContinue |
            Sort-Object FullName
        )

        foreach ($candidate in $filesystemCandidates) {
            $installationPath = Resolve-VsInstallRootFromVsDevCmdPath -VsDevCmdPath $candidate.FullName
            $msvcToolRoot = Resolve-MsvcToolRootForInstall -InstallationPath $installationPath
            if ($msvcToolRoot -and ($resolvedCandidates -notcontains $candidate.FullName)) {
                $resolvedCandidates += $candidate.FullName
            }
        }
    }

    return @($resolvedCandidates)
}

function Resolve-VsDevCmd {
    $candidates = @(Resolve-VsDevCmdCandidates)
    if ($candidates.Count -gt 0) {
        return $candidates[0]
    }

    return $null
}

function Resolve-MsvcToolRoot {
    $orderedInstalls = Get-OrderedVsInstallations
    foreach ($install in $orderedInstalls) {
        if (-not $install.installationPath) {
            continue
        }

        $candidate = Resolve-MsvcToolRootForInstall -InstallationPath $install.installationPath
        if ($candidate) {
            return $candidate
        }
    }

    return $null
}

function Test-WindowsSdkVersionRootIsBuildReady {
    param(
        [AllowEmptyString()]
        [string]$SdkRoot
    )

    if ([string]::IsNullOrWhiteSpace($SdkRoot)) {
        return $false
    }

    $sdkDir = Split-Path -Parent $SdkRoot
    $sdkVersion = Split-Path -Leaf $SdkRoot
    if ([string]::IsNullOrWhiteSpace($sdkDir) -or [string]::IsNullOrWhiteSpace($sdkVersion)) {
        return $false
    }

    $includeRoot = Join-Path $sdkDir "..\Include\$sdkVersion"
    $requiredPaths = @(
        (Join-Path $SdkRoot "um\x64\kernel32.lib"),
        (Join-Path $SdkRoot "ucrt\x64\ucrt.lib"),
        (Join-Path $includeRoot "ucrt\corecrt.h"),
        (Join-Path $includeRoot "um\Windows.h"),
        (Join-Path $includeRoot "shared\sdkddkver.h")
    )

    return (@($requiredPaths | Where-Object { [string]::IsNullOrWhiteSpace($_) -or -not (Test-Path $_) }).Count -eq 0)
}

function Resolve-WindowsSdkVersionRoot {
    $sdkRoots = @(
        (Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\Lib"),
        (Join-Path ${env:ProgramFiles} "Windows Kits\10\Lib")
    ) | Where-Object { $_ -and (Test-Path $_) }

    foreach ($root in $sdkRoots) {
        $candidates = @(
            Get-ChildItem -Path $root -Directory -ErrorAction SilentlyContinue |
            Sort-Object Name -Descending
        )

        foreach ($candidate in $candidates) {
            if (Test-WindowsSdkVersionRootIsBuildReady -SdkRoot $candidate.FullName) {
                return $candidate.FullName
            }
        }
    }

    return $null
}

function Prepend-EnvPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $true)]
        [string[]]$Entries
    )

    $existing = @()
    if (Test-Path "Env:$Name") {
        $existing = @((Get-Item "Env:$Name").Value -split ";" | Where-Object { $_ })
    }

    $combined = @($Entries + $existing) |
        Where-Object { $_ -and (Test-Path $_) } |
        Select-Object -Unique

    Set-Item -Path "Env:$Name" -Value ($combined -join ";")
}

function Configure-DirectMsvcEnvironment {
    $msvcRoot = Resolve-MsvcToolRoot
    if (-not $msvcRoot) {
        throw "MSVC tools were not found under Visual Studio Build Tools."
    }

    $linkerPath = Resolve-MsvcX64LinkerPathFromToolRoot -ToolRoot $msvcRoot
    if (-not $linkerPath) {
        throw "x64 MSVC linker was not found under Visual Studio Build Tools."
    }

    $sdkRoot = Resolve-WindowsSdkVersionRoot
    if (-not $sdkRoot) {
        throw "Windows 10 SDK libraries were not found."
    }

    $sdkDir = Split-Path -Parent $sdkRoot
    $sdkVersion = Split-Path -Leaf $sdkRoot

    $pathEntries = @(
        (Split-Path -Parent $linkerPath),
        (Join-Path $sdkDir "..\bin\$sdkVersion\x64")
    )
    $libEntries = @(
        (Join-Path $msvcRoot "lib\x64"),
        (Join-Path $sdkRoot "um\x64"),
        (Join-Path $sdkRoot "ucrt\x64")
    )
    $includeEntries = @(
        (Join-Path $msvcRoot "include"),
        (Join-Path $sdkDir "..\Include\$sdkVersion\ucrt"),
        (Join-Path $sdkDir "..\Include\$sdkVersion\um"),
        (Join-Path $sdkDir "..\Include\$sdkVersion\shared"),
        (Join-Path $sdkDir "..\Include\$sdkVersion\winrt"),
        (Join-Path $sdkDir "..\Include\$sdkVersion\cppwinrt")
    )

    Prepend-EnvPath -Name "Path" -Entries $pathEntries
    Prepend-EnvPath -Name "LIB" -Entries $libEntries
    Prepend-EnvPath -Name "INCLUDE" -Entries $includeEntries

    Set-Item -Path "Env:VCToolsInstallDir" -Value ($msvcRoot + "\")
    Set-Item -Path "Env:WindowsSdkDir" -Value ((Split-Path -Parent $sdkDir) + "\")
    Set-Item -Path "Env:WindowsSdkVersion" -Value ($sdkVersion + "\")
    Set-Item -Path "Env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER" -Value $linkerPath
}

function Import-VsDevEnvironment {
    param(
        [Parameter(Mandatory = $true)]
        [string]$VsDevCmdPath
    )

    $tempCmdPath = Join-Path ([System.IO.Path]::GetTempPath()) ("mergen-vsdevcmd-" + [guid]::NewGuid().ToString("N") + ".cmd")
    try {
        @(
            '@echo off',
            ('call "{0}" -arch=x64 -host_arch=x64 >nul' -f $VsDevCmdPath.Replace('"', '""')),
            'set "MERGEN_VSDEV_EXITCODE=%ERRORLEVEL%"',
            'if not "%MERGEN_VSDEV_EXITCODE%"=="0" exit /b %MERGEN_VSDEV_EXITCODE%',
            'set "MERGEN_VSDEV_EXITCODE="',
            'set'
        ) | Set-Content -Path $tempCmdPath -Encoding ASCII

        $output = @(& cmd.exe /d /c $tempCmdPath 2>&1)
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to import Visual Studio build environment from $VsDevCmdPath"
        }
    }
    finally {
        if (Test-Path $tempCmdPath) {
            Remove-Item $tempCmdPath -Force -ErrorAction SilentlyContinue
        }
    }

    foreach ($line in $output) {
        if ([string]::IsNullOrWhiteSpace($line)) {
            continue
        }

        $separator = $line.IndexOf("=")
        if ($separator -lt 1) {
            continue
        }

        $name = $line.Substring(0, $separator)
        if ($name.StartsWith("=")) {
            continue
        }

        $value = $line.Substring($separator + 1)
        Set-Item -Path "Env:$name" -Value $value
    }
}

function Resolve-MsvcLinkerPath {
    if ($env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER) {
        return $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER.Trim('"')
    }

    $link = Get-Command link.exe -ErrorAction SilentlyContinue
    if ($link) {
        return $link.Source
    }

    return $null
}

function Test-MsvcLinkerTargetsX64 {
    param(
        [AllowEmptyString()]
        [string]$LinkerPath
    )

    if ([string]::IsNullOrWhiteSpace($LinkerPath)) {
        return $false
    }

    $normalized = $LinkerPath.Replace("/", "\").ToLowerInvariant()
    return $normalized -match '\\bin\\host(x64|x86|arm64)\\x64\\link\.exe$'
}

function Find-MsvcFirstResolvedLibPath {
    param(
        [string[]]$LibEntries,
        [Parameter(Mandatory = $true)]
        [string]$LibraryName,
        [Parameter(Mandatory = $true)]
        [string]$ExpectedPathPattern
    )

    foreach ($entry in @($LibEntries | Where-Object { $_ })) {
        $candidatePath = Join-Path $entry $LibraryName
        if (-not (Test-Path $candidatePath)) {
            continue
        }

        $normalized = $candidatePath.Replace("/", "\").ToLowerInvariant()
        if ($normalized -match $ExpectedPathPattern) {
            return $candidatePath
        }

        return $null
    }

    return $null
}

function Find-MsvcX64Kernel32LibPath {
    param(
        [string[]]$LibEntries
    )

    return Find-MsvcFirstResolvedLibPath -LibEntries $LibEntries -LibraryName "kernel32.lib" -ExpectedPathPattern '\\um\\x64\\kernel32\.lib$'
}

function Find-MsvcX64UcrtLibPath {
    param(
        [string[]]$LibEntries
    )

    return Find-MsvcFirstResolvedLibPath -LibEntries $LibEntries -LibraryName "ucrt.lib" -ExpectedPathPattern '\\ucrt\\x64\\ucrt\.lib$'
}

function Find-MsvcX64VcRuntimeLibPath {
    param(
        [string[]]$LibEntries
    )

    return Find-MsvcFirstResolvedLibPath -LibEntries $LibEntries -LibraryName "vcruntime.lib" -ExpectedPathPattern '\\lib\\x64\\vcruntime\.lib$'
}

function Get-MsvcX64LibrarySetStatus {
    param(
        [string[]]$LibEntries
    )

    $kernel32Path = Find-MsvcX64Kernel32LibPath -LibEntries $LibEntries
    $ucrtPath = Find-MsvcX64UcrtLibPath -LibEntries $LibEntries
    $vcRuntimePath = Find-MsvcX64VcRuntimeLibPath -LibEntries $LibEntries

    return [pscustomobject]@{
        Kernel32Path = $kernel32Path
        UcrtPath = $ucrtPath
        VcRuntimePath = $vcRuntimePath
        IsReady = ($kernel32Path -and $ucrtPath -and $vcRuntimePath)
    }
}

function Assert-MsvcBuildEnvironment {
    $linkerPath = Resolve-MsvcLinkerPath
    $libStatus = Get-MsvcX64LibrarySetStatus -LibEntries @($env:LIB -split ";" | Where-Object { $_ })

    if (-not (Test-MsvcLinkerTargetsX64 -LinkerPath $linkerPath) -or -not $libStatus.IsReady) {
        Configure-DirectMsvcEnvironment

        $linkerPath = Resolve-MsvcLinkerPath
        if (-not (Test-MsvcLinkerTargetsX64 -LinkerPath $linkerPath)) {
            throw "x64 link.exe was not found after configuring the Visual Studio build environment. Install Visual Studio Build Tools or Visual Studio 2022 with Desktop development with C++."
        }

        $libStatus = Get-MsvcX64LibrarySetStatus -LibEntries @($env:LIB -split ";" | Where-Object { $_ })
        if (-not $libStatus.IsReady) {
            $missing = @()
            if (-not $libStatus.Kernel32Path) { $missing += "Windows SDK um\\x64\\kernel32.lib" }
            if (-not $libStatus.UcrtPath) { $missing += "Windows SDK ucrt\\x64\\ucrt.lib" }
            if (-not $libStatus.VcRuntimePath) { $missing += "MSVC lib\\x64\\vcruntime.lib" }
            $missingMessage = $missing -join ", "
            throw "Required x64 MSVC/SDK libraries were not found in LIB after configuring the Visual Studio build environment. Missing: $missingMessage"
        }
    }
}

function Save-EnvironmentSnapshot {
    $snapshot = @{}
    foreach ($entry in Get-ChildItem Env:) {
        $snapshot[$entry.Name] = $entry.Value
    }

    return $snapshot
}

function Restore-EnvironmentSnapshot {
    param(
        [Parameter(Mandatory = $true)]
        [hashtable]$Snapshot
    )

    $currentNames = @((Get-ChildItem Env:).Name)
    foreach ($name in $currentNames) {
        if (-not $Snapshot.ContainsKey($name)) {
            Remove-Item -Path ("Env:" + $name) -ErrorAction SilentlyContinue
        }
    }

    foreach ($name in $Snapshot.Keys) {
        Set-Item -Path ("Env:" + $name) -Value $Snapshot[$name]
    }
}

function Ensure-MsvcBuildEnvironment {
    try {
        Assert-MsvcBuildEnvironment
        return
    }
    catch {
        $initialError = $_.Exception.Message
    }

    $vsDevCmdCandidates = @(Resolve-VsDevCmdCandidates)
    if ($vsDevCmdCandidates.Count -eq 0) {
        throw "MSVC build environment was not ready and Visual Studio build environment could not be resolved. Initial check failed with: $initialError"
    }

    $baseSnapshot = Save-EnvironmentSnapshot
    $attemptFailures = @()

    foreach ($vsDevCmd in $vsDevCmdCandidates) {
        Restore-EnvironmentSnapshot -Snapshot $baseSnapshot

        try {
            Import-VsDevEnvironment -VsDevCmdPath $vsDevCmd
            Assert-MsvcBuildEnvironment
            return
        }
        catch {
            $attemptFailures += ("{0}: {1}" -f $vsDevCmd, $_.Exception.Message)
        }
    }

    Restore-EnvironmentSnapshot -Snapshot $baseSnapshot
    $attemptSummary = $attemptFailures -join " | "
    throw "MSVC build environment was not ready. Initial check failed with: $initialError. Visual Studio environment attempts failed: $attemptSummary"
}

function Resolve-Dumpbin {
    $dumpbin = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
    if ($dumpbin) {
        return $dumpbin.Source
    }

    $orderedInstalls = Get-OrderedVsInstallations
    foreach ($install in $orderedInstalls) {
        if (-not $install.installationPath) {
            continue
        }

        $candidate = Get-ChildItem -Path (Join-Path $install.installationPath "VC\Tools") -Recurse -Filter dumpbin.exe -ErrorAction SilentlyContinue |
            Sort-Object FullName -Descending |
            Select-Object -First 1 -ExpandProperty FullName
        if ($candidate) {
            return $candidate
        }
    }

    return Find-DumpbinUnderRoots -Roots @(
        (Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio"),
        (Join-Path ${env:ProgramFiles} "Microsoft Visual Studio")
    )
}

function Find-DumpbinUnderRoots {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Roots
    )

    $roots = @(
        $Roots
    ) | Where-Object { $_ -and (Test-Path $_) }

    foreach ($root in $roots) {
        $candidate = Get-ChildItem -Path $root -Recurse -Filter dumpbin.exe -ErrorAction SilentlyContinue |
            Sort-Object FullName -Descending |
            Select-Object -First 1 -ExpandProperty FullName
        if ($candidate) {
            return $candidate
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

            $llvmObjdumpError = "Repo-local llvm-objdump.exe could not read imports."
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
        throw "Dependency inspection failed. llvm-objdump was unusable and dumpbin.exe could not be resolved: $llvmObjdumpError"
    }

    throw "Dependency inspection tool was not found. Neither repo-local llvm-objdump.exe nor Visual Studio dumpbin.exe could be used."
}

function Remove-UnsupportedConvenienceArtifacts {
    $legacyPaths = Get-UnsupportedConvenienceArtifactPaths
    $failedPaths = @()

    foreach ($path in $legacyPaths) {
        if (Test-Path $path) {
            try {
                Remove-Item $path -Force -ErrorAction Stop
            }
            catch {
                $failedPaths += $path
            }
        }
    }

    if ($failedPaths.Count -gt 0) {
        $joined = ($failedPaths | Sort-Object -Unique) -join ", "
        throw "Unsupported convenience artifacts could not be removed: $joined. Close or delete the stale executable(s) and rerun the release build."
    }
}

function Get-UnsupportedConvenienceArtifactPaths {
    return @(
        (Join-Path $repoRoot "target\release\mergen-ade.exe"),
        (Join-Path $repoRoot "mergen-ade.exe")
    )
}

function Invoke-BuildRelease {
    Push-Location $repoRoot
    try {
        Ensure-MsvcToolchain
        $cargo = Resolve-MsvcCargo
        $rustc = Resolve-MsvcToolBinary -BinaryName "rustc"
        $toolchainBin = Split-Path -Parent $cargo
        Prepend-EnvPath -Name "Path" -Entries @($toolchainBin)
        Set-Item -Path "Env:RUSTC" -Value $rustc
        Set-Item -Path "Env:CARGO" -Value $cargo
        Ensure-MsvcBuildEnvironment

        & $cargo build --release --target $target -j 1
        if ($LASTEXITCODE -ne 0) {
            throw "Portable release build failed."
        }

        if (-not (Test-Path $exePath)) {
            throw "Portable release EXE was not created: $exePath"
        }

        $imports = Get-ImportedDllNames -ExecutablePath $exePath
        $blockedImports = $imports | Where-Object { $blockedDlls -contains $_ }
        if ($blockedImports) {
            $joined = ($blockedImports | Sort-Object -Unique) -join ", "
            throw "Portable EXE still depends on blocked runtime DLLs: $joined"
        }

        Remove-UnsupportedConvenienceArtifacts
        Write-Host "Portable release EXE ready: $exePath"
    }
    finally {
        Pop-Location
    }
}

if ($MyInvocation.InvocationName -ne ".") {
    Invoke-BuildRelease
}
