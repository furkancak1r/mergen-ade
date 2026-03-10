$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. (Join-Path $repoRoot "scripts\build-release.ps1")

function Assert-True {
    param(
        [Parameter(Mandatory = $true)]
        [bool]$Condition,
        [Parameter(Mandatory = $true)]
        [string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Assert-Equal {
    param(
        [Parameter(Mandatory = $true)]
        $Actual,
        [Parameter(Mandatory = $true)]
        $Expected,
        [Parameter(Mandatory = $true)]
        [string]$Message
    )

    if ($Actual -ne $Expected) {
        throw "$Message`nExpected: $Expected`nActual:   $Actual"
    }
}

function Test-CargoConfigRetainsRepoLocalGnullvmLinker {
    $configPath = Join-Path $repoRoot ".cargo\config.toml"
    $config = Get-Content -Path $configPath -Raw

    Assert-True -Condition $config.Contains('target = "x86_64-pc-windows-gnullvm"') -Message "Expected .cargo/config.toml to keep the gnullvm default target."
    Assert-True -Condition $config.Contains('[target.x86_64-pc-windows-gnullvm]') -Message "Expected .cargo/config.toml to keep the gnullvm target section."
    Assert-True -Condition $config.Contains('linker = ".toolchain/llvm-mingw-20260224-ucrt-x86_64/bin/x86_64-w64-mingw32-clang.exe"') -Message "Expected .cargo/config.toml to pin the repo-local gnullvm linker."
    Assert-True -Condition $config.Contains('[target.x86_64-pc-windows-msvc]') -Message "Expected .cargo/config.toml to keep the MSVC target section."
    Assert-True -Condition $config.Contains('rustflags = ["-C", "target-feature=+crt-static"]') -Message "Expected .cargo/config.toml to keep MSVC static CRT rustflags."
}

function Test-SortVsInstallationsPrefersHealthyBuildTools {
    $installations = @(
        [pscustomobject]@{
            productId = "Microsoft.VisualStudio.Product.Enterprise"
            isComplete = $false
            installationVersion = "17.11.5"
            installationPath = "C:\VS\BrokenEnterprise"
        },
        [pscustomobject]@{
            productId = "Microsoft.VisualStudio.Product.BuildTools"
            isComplete = $true
            installationVersion = "17.10.1"
            installationPath = "C:\VS\BuildTools"
        },
        [pscustomobject]@{
            productId = "Microsoft.VisualStudio.Product.Community"
            isComplete = $true
            installationVersion = "17.9.3"
            installationPath = "C:\VS\CommunityOld"
        },
        [pscustomobject]@{
            productId = "Microsoft.VisualStudio.Product.Community"
            isComplete = $true
            installationVersion = "17.10.5"
            installationPath = "C:\VS\CommunityNew"
        },
        [pscustomobject]@{
            productId = "Microsoft.VisualStudio.Product.BuildTools"
            isComplete = $false
            installationVersion = "17.12.0"
            installationPath = "C:\VS\BrokenBuildTools"
        }
    )

    $ordered = @(Sort-VsInstallations -Installations $installations)
    $paths = @($ordered | ForEach-Object { $_.installationPath })

    Assert-Equal -Actual $paths.Count -Expected 5 -Message "Expected all Visual Studio installations to remain in the ordered output."
    Assert-Equal -Actual $paths[0] -Expected "C:\VS\BuildTools" -Message "Expected complete Build Tools to be preferred first."
    Assert-Equal -Actual $paths[1] -Expected "C:\VS\CommunityNew" -Message "Expected complete non-BuildTools installs to follow in descending version order."
    Assert-Equal -Actual $paths[2] -Expected "C:\VS\CommunityOld" -Message "Expected older complete installs to remain behind newer complete installs."
    Assert-Equal -Actual $paths[3] -Expected "C:\VS\BrokenBuildTools" -Message "Expected incomplete Build Tools to remain behind all complete installs."
    Assert-Equal -Actual $paths[4] -Expected "C:\VS\BrokenEnterprise" -Message "Expected incomplete non-BuildTools installs to be checked last."
}

function Test-FindDumpbinUnderRootsFallsBackToProgramFilesSearch {
    $tempRoot = Join-Path $env:TEMP ("mergen-dumpbin-test-" + [guid]::NewGuid().ToString("N"))
    $vsRoot = Join-Path $tempRoot "Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64"
    New-Item -ItemType Directory -Path $vsRoot -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $vsRoot "dumpbin.exe") -Force | Out-Null

    try {
        $resolved = Find-DumpbinUnderRoots -Roots @($tempRoot)
        $expected = [System.IO.Path]::GetFullPath((Join-Path $vsRoot "dumpbin.exe")).ToLowerInvariant()
        $actual = [System.IO.Path]::GetFullPath($resolved).ToLowerInvariant()
        Assert-Equal -Actual $actual -Expected $expected -Message "Expected recursive Program Files fallback to find dumpbin.exe."
    }
    finally {
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-ResolveVsDevCmdSkipsInstallsWithoutMsvcTools {
    $tempRoot = Join-Path $env:TEMP ("mergen-vsdevcmd-resolve-test-" + [guid]::NewGuid().ToString("N"))
    $communityRoot = Join-Path $tempRoot "Community"
    $buildToolsRoot = Join-Path $tempRoot "BuildTools"
    $communityVsDevCmd = Join-Path $communityRoot "Common7\Tools\VsDevCmd.bat"
    $buildToolsVsDevCmd = Join-Path $buildToolsRoot "Common7\Tools\VsDevCmd.bat"
    $buildToolsLinker = Join-Path $buildToolsRoot "VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64\link.exe"
    $buildToolsRuntimeLib = Join-Path $buildToolsRoot "VC\Tools\MSVC\14.44.35207\lib\x64\vcruntime.lib"
    $buildToolsHeader = Join-Path $buildToolsRoot "VC\Tools\MSVC\14.44.35207\include\vcruntime.h"
    $originalGetOrdered = (Get-Command Get-OrderedVsInstallations).ScriptBlock

    New-Item -ItemType Directory -Path (Split-Path -Parent $communityVsDevCmd) -Force | Out-Null
    New-Item -ItemType Directory -Path (Split-Path -Parent $buildToolsVsDevCmd) -Force | Out-Null
    New-Item -ItemType Directory -Path (Split-Path -Parent $buildToolsLinker) -Force | Out-Null
    New-Item -ItemType Directory -Path (Split-Path -Parent $buildToolsRuntimeLib) -Force | Out-Null
    New-Item -ItemType Directory -Path (Split-Path -Parent $buildToolsHeader) -Force | Out-Null
    New-Item -ItemType File -Path $communityVsDevCmd -Force | Out-Null
    New-Item -ItemType File -Path $buildToolsVsDevCmd -Force | Out-Null
    New-Item -ItemType File -Path $buildToolsLinker -Force | Out-Null
    New-Item -ItemType File -Path $buildToolsRuntimeLib -Force | Out-Null
    New-Item -ItemType File -Path $buildToolsHeader -Force | Out-Null

    try {
        Set-Item -Path Function:\Get-OrderedVsInstallations -Value {
            @(
                [pscustomobject]@{ installationPath = $communityRoot },
                [pscustomobject]@{ installationPath = $buildToolsRoot }
            )
        }

        $resolved = Resolve-VsDevCmd
        $expected = [System.IO.Path]::GetFullPath($buildToolsVsDevCmd).ToLowerInvariant()
        $actual = [System.IO.Path]::GetFullPath($resolved).ToLowerInvariant()
        Assert-Equal -Actual $actual -Expected $expected -Message "Expected Resolve-VsDevCmd to skip installs without usable MSVC x64 tools."
    }
    finally {
        Set-Item -Path Function:\Get-OrderedVsInstallations -Value $originalGetOrdered
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-ResolveVsDevCmdFilesystemFallbackUsesInstallRoot {
    $tempRoot = Join-Path $env:TEMP ("mergen-vsdevcmd-filesystem-fallback-test-" + [guid]::NewGuid().ToString("N"))
    $programFilesRoot = Join-Path $tempRoot "Program Files"
    $vsSearchRoot = Join-Path $programFilesRoot "Microsoft Visual Studio"
    $buildToolsRoot = Join-Path $vsSearchRoot "2022\BuildTools"
    $vsDevCmdPath = Join-Path $buildToolsRoot "Common7\Tools\VsDevCmd.bat"
    $linkerPath = Join-Path $buildToolsRoot "VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64\link.exe"
    $runtimeLibPath = Join-Path $buildToolsRoot "VC\Tools\MSVC\14.44.35207\lib\x64\vcruntime.lib"
    $headerPath = Join-Path $buildToolsRoot "VC\Tools\MSVC\14.44.35207\include\vcruntime.h"
    $originalGetOrdered = (Get-Command Get-OrderedVsInstallations).ScriptBlock
    $originalProgramFiles = $env:ProgramFiles
    $originalProgramFilesX86 = ${env:ProgramFiles(x86)}

    New-Item -ItemType Directory -Path (Split-Path -Parent $vsDevCmdPath) -Force | Out-Null
    New-Item -ItemType Directory -Path (Split-Path -Parent $linkerPath) -Force | Out-Null
    New-Item -ItemType Directory -Path (Split-Path -Parent $runtimeLibPath) -Force | Out-Null
    New-Item -ItemType Directory -Path (Split-Path -Parent $headerPath) -Force | Out-Null
    New-Item -ItemType File -Path $vsDevCmdPath -Force | Out-Null
    New-Item -ItemType File -Path $linkerPath -Force | Out-Null
    New-Item -ItemType File -Path $runtimeLibPath -Force | Out-Null
    New-Item -ItemType File -Path $headerPath -Force | Out-Null

    try {
        Set-Item -Path Function:\Get-OrderedVsInstallations -Value { @() }
        Set-Item -Path Env:ProgramFiles -Value $programFilesRoot
        Set-Item -Path 'Env:ProgramFiles(x86)' -Value (Join-Path $tempRoot "Program Files (x86)\missing")

        $resolved = Resolve-VsDevCmd
        $expected = [System.IO.Path]::GetFullPath($vsDevCmdPath).ToLowerInvariant()
        $actual = [System.IO.Path]::GetFullPath($resolved).ToLowerInvariant()
        Assert-Equal -Actual $actual -Expected $expected -Message "Expected Resolve-VsDevCmd filesystem fallback to derive the real install root before probing MSVC tools."
    }
    finally {
        Set-Item -Path Function:\Get-OrderedVsInstallations -Value $originalGetOrdered
        if ($null -ne $originalProgramFiles) { Set-Item -Path Env:ProgramFiles -Value $originalProgramFiles } else { Remove-Item Env:ProgramFiles -ErrorAction SilentlyContinue }
        if ($null -ne $originalProgramFilesX86) { Set-Item -Path 'Env:ProgramFiles(x86)' -Value $originalProgramFilesX86 } else { Remove-Item 'Env:ProgramFiles(x86)' -ErrorAction SilentlyContinue }
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-EnsureMsvcBuildEnvironmentSkipsVsDevCmdWhenShellIsAlreadyReady {
    $originalAssert = (Get-Command Assert-MsvcBuildEnvironment).ScriptBlock
    $originalResolveCandidates = (Get-Command Resolve-VsDevCmdCandidates).ScriptBlock
    $originalImport = (Get-Command Import-VsDevEnvironment).ScriptBlock
    [int]$script:assertCalls = 0
    [int]$script:resolveCandidateCalls = 0
    [int]$script:importCalls = 0

    try {
        Set-Item -Path Function:\Assert-MsvcBuildEnvironment -Value {
            $script:assertCalls++
        }
        Set-Item -Path Function:\Resolve-VsDevCmdCandidates -Value {
            $script:resolveCandidateCalls++
            throw "Resolve-VsDevCmdCandidates should not be called when the shell is already configured."
        }
        Set-Item -Path Function:\Import-VsDevEnvironment -Value {
            param([string]$VsDevCmdPath)
            $script:importCalls++
        }

        Ensure-MsvcBuildEnvironment

        Assert-Equal -Actual $script:assertCalls -Expected 1 -Message "Expected the current shell environment to be validated exactly once when it is already MSVC-ready."
        Assert-Equal -Actual $script:resolveCandidateCalls -Expected 0 -Message "Expected Ensure-MsvcBuildEnvironment to skip Resolve-VsDevCmdCandidates when Assert-MsvcBuildEnvironment already passes."
        Assert-Equal -Actual $script:importCalls -Expected 0 -Message "Expected Ensure-MsvcBuildEnvironment to skip VsDevCmd import when the current shell is already ready."
    }
    finally {
        Set-Item -Path Function:\Assert-MsvcBuildEnvironment -Value $originalAssert
        Set-Item -Path Function:\Resolve-VsDevCmdCandidates -Value $originalResolveCandidates
        Set-Item -Path Function:\Import-VsDevEnvironment -Value $originalImport
        Remove-Variable -Name assertCalls -Scope Script -ErrorAction SilentlyContinue
        Remove-Variable -Name resolveCandidateCalls -Scope Script -ErrorAction SilentlyContinue
        Remove-Variable -Name importCalls -Scope Script -ErrorAction SilentlyContinue
    }
}

function Test-EnsureMsvcBuildEnvironmentRetriesLaterVsDevCmdCandidates {
    $originalAssert = (Get-Command Assert-MsvcBuildEnvironment).ScriptBlock
    $originalResolveCandidates = (Get-Command Resolve-VsDevCmdCandidates).ScriptBlock
    $originalImport = (Get-Command Import-VsDevEnvironment).ScriptBlock
    [int]$script:retryAssertCalls = 0
    [int]$script:retryImportCalls = 0
    $script:firstRetryCandidate = "C:\VS\BuildTools\Common7\Tools\VsDevCmd.bat"
    $script:secondRetryCandidate = "C:\VS\Community\Common7\Tools\VsDevCmd.bat"

    try {
        Remove-Item Env:MERGEN_IMPORTED_CANDIDATE -ErrorAction SilentlyContinue

        Set-Item -Path Function:\Assert-MsvcBuildEnvironment -Value {
            $script:retryAssertCalls++
            switch ($script:retryAssertCalls) {
                1 { throw "initial shell failed" }
                2 { throw "first imported shell missing SDK" }
                3 {
                    if ($env:MERGEN_IMPORTED_CANDIDATE -ne $script:secondRetryCandidate) {
                        throw "Expected retry to reach the second VsDevCmd candidate."
                    }
                }
                default { throw "Unexpected Assert-MsvcBuildEnvironment call count." }
            }
        }
        Set-Item -Path Function:\Resolve-VsDevCmdCandidates -Value {
            @($script:firstRetryCandidate, $script:secondRetryCandidate)
        }
        Set-Item -Path Function:\Import-VsDevEnvironment -Value {
            param([string]$VsDevCmdPath)
            $script:retryImportCalls++
            Set-Item -Path Env:MERGEN_IMPORTED_CANDIDATE -Value $VsDevCmdPath
        }

        Ensure-MsvcBuildEnvironment

        Assert-Equal -Actual $script:retryImportCalls -Expected 2 -Message "Expected Ensure-MsvcBuildEnvironment to retry the second VsDevCmd candidate after the first import still fails."
        Assert-Equal -Actual $script:retryAssertCalls -Expected 3 -Message "Expected initial shell plus both imported candidates to be validated."
    }
    finally {
        Set-Item -Path Function:\Assert-MsvcBuildEnvironment -Value $originalAssert
        Set-Item -Path Function:\Resolve-VsDevCmdCandidates -Value $originalResolveCandidates
        Set-Item -Path Function:\Import-VsDevEnvironment -Value $originalImport
        Remove-Item Env:MERGEN_IMPORTED_CANDIDATE -ErrorAction SilentlyContinue
        Remove-Variable -Name retryAssertCalls -Scope Script -ErrorAction SilentlyContinue
        Remove-Variable -Name retryImportCalls -Scope Script -ErrorAction SilentlyContinue
        Remove-Variable -Name firstRetryCandidate -Scope Script -ErrorAction SilentlyContinue
        Remove-Variable -Name secondRetryCandidate -Scope Script -ErrorAction SilentlyContinue
    }
}

function Test-EnsureMsvcBuildEnvironmentRestoresEnvironmentBetweenRetries {
    $originalAssert = (Get-Command Assert-MsvcBuildEnvironment).ScriptBlock
    $originalResolveCandidates = (Get-Command Resolve-VsDevCmdCandidates).ScriptBlock
    $originalImport = (Get-Command Import-VsDevEnvironment).ScriptBlock
    [int]$script:restoreAssertCalls = 0
    [int]$script:restoreImportCalls = 0
    $script:firstRestoreCandidate = "C:\VS\BuildTools\Common7\Tools\VsDevCmd.bat"
    $script:secondRestoreCandidate = "C:\VS\Community\Common7\Tools\VsDevCmd.bat"

    try {
        Remove-Item Env:MERGEN_IMPORTED_CANDIDATE -ErrorAction SilentlyContinue
        Remove-Item Env:MERGEN_ATTEMPT_LEAK -ErrorAction SilentlyContinue

        Set-Item -Path Function:\Assert-MsvcBuildEnvironment -Value {
            $script:restoreAssertCalls++
            switch ($script:restoreAssertCalls) {
                1 { throw "initial shell failed" }
                2 {
                    Set-Item -Path Env:MERGEN_ATTEMPT_LEAK -Value "leaked-from-first-attempt"
                    throw "first imported shell missing SDK"
                }
                3 {
                    if ($env:MERGEN_ATTEMPT_LEAK) {
                        throw "Expected environment leak to be cleared before retrying the next VsDevCmd candidate."
                    }
                }
                default { throw "Unexpected Assert-MsvcBuildEnvironment call count." }
            }
        }
        Set-Item -Path Function:\Resolve-VsDevCmdCandidates -Value {
            @($script:firstRestoreCandidate, $script:secondRestoreCandidate)
        }
        Set-Item -Path Function:\Import-VsDevEnvironment -Value {
            param([string]$VsDevCmdPath)
            $script:restoreImportCalls++
            Set-Item -Path Env:MERGEN_IMPORTED_CANDIDATE -Value $VsDevCmdPath
        }

        Ensure-MsvcBuildEnvironment

        Assert-Equal -Actual $script:restoreImportCalls -Expected 2 -Message "Expected both VsDevCmd candidates to be imported during retry."
    }
    finally {
        Set-Item -Path Function:\Assert-MsvcBuildEnvironment -Value $originalAssert
        Set-Item -Path Function:\Resolve-VsDevCmdCandidates -Value $originalResolveCandidates
        Set-Item -Path Function:\Import-VsDevEnvironment -Value $originalImport
        Remove-Item Env:MERGEN_IMPORTED_CANDIDATE -ErrorAction SilentlyContinue
        Remove-Item Env:MERGEN_ATTEMPT_LEAK -ErrorAction SilentlyContinue
        Remove-Variable -Name restoreAssertCalls -Scope Script -ErrorAction SilentlyContinue
        Remove-Variable -Name restoreImportCalls -Scope Script -ErrorAction SilentlyContinue
        Remove-Variable -Name firstRestoreCandidate -Scope Script -ErrorAction SilentlyContinue
        Remove-Variable -Name secondRestoreCandidate -Scope Script -ErrorAction SilentlyContinue
    }
}

function Test-EnsureMsvcBuildEnvironmentReportsAllFailedVsDevCmdAttempts {
    $originalAssert = (Get-Command Assert-MsvcBuildEnvironment).ScriptBlock
    $originalResolveCandidates = (Get-Command Resolve-VsDevCmdCandidates).ScriptBlock
    $originalImport = (Get-Command Import-VsDevEnvironment).ScriptBlock
    $script:firstFailureCandidate = "C:\VS\BuildTools\Common7\Tools\VsDevCmd.bat"
    $script:secondFailureCandidate = "C:\VS\Community\Common7\Tools\VsDevCmd.bat"
    [int]$script:failureAssertCalls = 0

    try {
        Set-Item -Path Function:\Assert-MsvcBuildEnvironment -Value {
            $script:failureAssertCalls++
            switch ($script:failureAssertCalls) {
                1 { throw "initial shell failed" }
                2 { throw "first imported shell missing SDK" }
                3 { throw "second imported shell missing linker" }
                default { throw "Unexpected Assert-MsvcBuildEnvironment call count." }
            }
        }
        Set-Item -Path Function:\Resolve-VsDevCmdCandidates -Value {
            @($script:firstFailureCandidate, $script:secondFailureCandidate)
        }
        Set-Item -Path Function:\Import-VsDevEnvironment -Value {
            param([string]$VsDevCmdPath)
        }

        try {
            Ensure-MsvcBuildEnvironment
            throw "Expected Ensure-MsvcBuildEnvironment to fail when all VsDevCmd attempts fail."
        }
        catch {
            $message = $_.Exception.Message
            Assert-True -Condition $message.Contains("initial shell failed") -Message "Expected failure message to include the initial shell validation error."
            Assert-True -Condition $message.Contains($script:firstFailureCandidate) -Message "Expected failure message to include the first VsDevCmd candidate."
            Assert-True -Condition $message.Contains("first imported shell missing SDK") -Message "Expected failure message to include the first VsDevCmd failure."
            Assert-True -Condition $message.Contains($script:secondFailureCandidate) -Message "Expected failure message to include the second VsDevCmd candidate."
            Assert-True -Condition $message.Contains("second imported shell missing linker") -Message "Expected failure message to include the second VsDevCmd failure."
        }
    }
    finally {
        Set-Item -Path Function:\Assert-MsvcBuildEnvironment -Value $originalAssert
        Set-Item -Path Function:\Resolve-VsDevCmdCandidates -Value $originalResolveCandidates
        Set-Item -Path Function:\Import-VsDevEnvironment -Value $originalImport
        Remove-Variable -Name firstFailureCandidate -Scope Script -ErrorAction SilentlyContinue
        Remove-Variable -Name secondFailureCandidate -Scope Script -ErrorAction SilentlyContinue
        Remove-Variable -Name failureAssertCalls -Scope Script -ErrorAction SilentlyContinue
    }
}

function Test-ResolveMsvcToolRootAcceptsHostx86X64Linker {
    $tempRoot = Join-Path $env:TEMP ("mergen-msvc-root-test-" + [guid]::NewGuid().ToString("N"))
    $installRoot = Join-Path $tempRoot "BuildTools"
    $toolRoot = Join-Path $installRoot "VC\Tools\MSVC\14.44.35207"
    $linkerPath = Join-Path $toolRoot "bin\Hostx86\x64\link.exe"
    $runtimeLibPath = Join-Path $toolRoot "lib\x64\vcruntime.lib"
    $headerPath = Join-Path $toolRoot "include\vcruntime.h"
    $originalGetOrdered = (Get-Command Get-OrderedVsInstallations).ScriptBlock

    New-Item -ItemType Directory -Path (Split-Path -Parent $linkerPath) -Force | Out-Null
    New-Item -ItemType Directory -Path (Split-Path -Parent $runtimeLibPath) -Force | Out-Null
    New-Item -ItemType Directory -Path (Split-Path -Parent $headerPath) -Force | Out-Null
    New-Item -ItemType File -Path $linkerPath -Force | Out-Null
    New-Item -ItemType File -Path $runtimeLibPath -Force | Out-Null
    New-Item -ItemType File -Path $headerPath -Force | Out-Null

    try {
        Set-Item -Path Function:\Get-OrderedVsInstallations -Value {
            @([pscustomobject]@{ installationPath = $installRoot })
        }

        $resolved = Resolve-MsvcToolRoot
        $expected = [System.IO.Path]::GetFullPath($toolRoot).ToLowerInvariant()
        $actual = [System.IO.Path]::GetFullPath($resolved).ToLowerInvariant()
        Assert-Equal -Actual $actual -Expected $expected -Message "Expected Resolve-MsvcToolRoot to accept Hostx86\\x64 cross tools."
    }
    finally {
        Set-Item -Path Function:\Get-OrderedVsInstallations -Value $originalGetOrdered
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-ResolveMsvcToolRootAcceptsHostarm64X64Linker {
    $tempRoot = Join-Path $env:TEMP ("mergen-msvc-root-arm64-test-" + [guid]::NewGuid().ToString("N"))
    $installRoot = Join-Path $tempRoot "BuildTools"
    $toolRoot = Join-Path $installRoot "VC\Tools\MSVC\14.44.35207"
    $linkerPath = Join-Path $toolRoot "bin\Hostarm64\x64\link.exe"
    $runtimeLibPath = Join-Path $toolRoot "lib\x64\vcruntime.lib"
    $headerPath = Join-Path $toolRoot "include\vcruntime.h"
    $originalGetOrdered = (Get-Command Get-OrderedVsInstallations).ScriptBlock

    New-Item -ItemType Directory -Path (Split-Path -Parent $linkerPath) -Force | Out-Null
    New-Item -ItemType Directory -Path (Split-Path -Parent $runtimeLibPath) -Force | Out-Null
    New-Item -ItemType Directory -Path (Split-Path -Parent $headerPath) -Force | Out-Null
    New-Item -ItemType File -Path $linkerPath -Force | Out-Null
    New-Item -ItemType File -Path $runtimeLibPath -Force | Out-Null
    New-Item -ItemType File -Path $headerPath -Force | Out-Null

    try {
        Set-Item -Path Function:\Get-OrderedVsInstallations -Value {
            @([pscustomobject]@{ installationPath = $installRoot })
        }

        $resolved = Resolve-MsvcToolRoot
        $expected = [System.IO.Path]::GetFullPath($toolRoot).ToLowerInvariant()
        $actual = [System.IO.Path]::GetFullPath($resolved).ToLowerInvariant()
        Assert-Equal -Actual $actual -Expected $expected -Message "Expected Resolve-MsvcToolRoot to accept Hostarm64\\x64 cross tools."
    }
    finally {
        Set-Item -Path Function:\Get-OrderedVsInstallations -Value $originalGetOrdered
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-ResolveMsvcToolRootForInstallSkipsIncompleteNewerToolsets {
    $tempRoot = Join-Path $env:TEMP ("mergen-msvc-root-fallback-test-" + [guid]::NewGuid().ToString("N"))
    $installRoot = Join-Path $tempRoot "BuildTools"
    $newerToolRoot = Join-Path $installRoot "VC\Tools\MSVC\14.45.36000"
    $olderToolRoot = Join-Path $installRoot "VC\Tools\MSVC\14.44.35207"
    $newerLinkerPath = Join-Path $newerToolRoot "bin\Hostx64\x64\link.exe"
    $olderLinkerPath = Join-Path $olderToolRoot "bin\Hostx64\x64\link.exe"
    $olderRuntimeLibPath = Join-Path $olderToolRoot "lib\x64\vcruntime.lib"
    $olderHeaderPath = Join-Path $olderToolRoot "include\vcruntime.h"

    New-Item -ItemType Directory -Path (Split-Path -Parent $newerLinkerPath) -Force | Out-Null
    New-Item -ItemType Directory -Path (Split-Path -Parent $olderLinkerPath) -Force | Out-Null
    New-Item -ItemType Directory -Path (Split-Path -Parent $olderRuntimeLibPath) -Force | Out-Null
    New-Item -ItemType Directory -Path (Split-Path -Parent $olderHeaderPath) -Force | Out-Null
    New-Item -ItemType File -Path $newerLinkerPath -Force | Out-Null
    New-Item -ItemType File -Path $olderLinkerPath -Force | Out-Null
    New-Item -ItemType File -Path $olderRuntimeLibPath -Force | Out-Null
    New-Item -ItemType File -Path $olderHeaderPath -Force | Out-Null

    try {
        $resolved = Resolve-MsvcToolRootForInstall -InstallationPath $installRoot
        $expected = [System.IO.Path]::GetFullPath($olderToolRoot).ToLowerInvariant()
        $actual = [System.IO.Path]::GetFullPath($resolved).ToLowerInvariant()
        Assert-Equal -Actual $actual -Expected $expected -Message "Expected Resolve-MsvcToolRootForInstall to skip newer toolsets that only contain link.exe."
    }
    finally {
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-ResolveMsvcToolRootForInstallReturnsNullWithoutCompleteToolsets {
    $tempRoot = Join-Path $env:TEMP ("mergen-msvc-root-negative-test-" + [guid]::NewGuid().ToString("N"))
    $installRoot = Join-Path $tempRoot "BuildTools"
    $toolRoot = Join-Path $installRoot "VC\Tools\MSVC\14.45.36000"
    $linkerPath = Join-Path $toolRoot "bin\Hostx64\x64\link.exe"

    New-Item -ItemType Directory -Path (Split-Path -Parent $linkerPath) -Force | Out-Null
    New-Item -ItemType File -Path $linkerPath -Force | Out-Null

    try {
        $resolved = Resolve-MsvcToolRootForInstall -InstallationPath $installRoot
        Assert-True -Condition ([string]::IsNullOrEmpty($resolved)) -Message "Expected Resolve-MsvcToolRootForInstall to return null when no MSVC toolset is build-ready."
    }
    finally {
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-TestMsvcLinkerTargetsX64RejectsX86Linkers {
    Assert-True -Condition (Test-MsvcLinkerTargetsX64 -LinkerPath "C:\VS\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64\link.exe") -Message "Expected Hostx64\\x64 link.exe to be accepted for x64 builds."
    Assert-True -Condition (Test-MsvcLinkerTargetsX64 -LinkerPath "C:\VS\VC\Tools\MSVC\14.44.35207\bin\Hostx86\x64\link.exe") -Message "Expected Hostx86\\x64 link.exe to be accepted for x64 builds."
    Assert-True -Condition (Test-MsvcLinkerTargetsX64 -LinkerPath "C:\VS\VC\Tools\MSVC\14.44.35207\bin\Hostarm64\x64\link.exe") -Message "Expected Hostarm64\\x64 link.exe to be accepted for x64 builds."
    Assert-True -Condition (-not (Test-MsvcLinkerTargetsX64 -LinkerPath "C:\VS\VC\Tools\MSVC\14.44.35207\bin\Hostx86\x86\link.exe")) -Message "Expected Hostx86\\x86 link.exe to be rejected for x64 builds."
    Assert-True -Condition (-not (Test-MsvcLinkerTargetsX64 -LinkerPath "C:\VS\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x86\link.exe")) -Message "Expected Hostx64\\x86 link.exe to be rejected for x64 builds."
}

function Test-FindMsvcX64Kernel32LibPathRejectsX86Entries {
    $tempRoot = Join-Path $env:TEMP ("mergen-kernel32-test-" + [guid]::NewGuid().ToString("N"))
    $x64LibRoot = Join-Path $tempRoot "Windows Kits\10\Lib\10.0.26100.0\um\x64"
    $x86LibRoot = Join-Path $tempRoot "Windows Kits\10\Lib\10.0.26100.0\um\x86"
    New-Item -ItemType Directory -Path $x64LibRoot -Force | Out-Null
    New-Item -ItemType Directory -Path $x86LibRoot -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $x64LibRoot "kernel32.lib") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $x86LibRoot "kernel32.lib") -Force | Out-Null

    try {
        $resolvedX64 = Find-MsvcX64Kernel32LibPath -LibEntries @($x64LibRoot, $x86LibRoot)
        $expectedX64 = Join-Path $x64LibRoot "kernel32.lib"
        Assert-Equal -Actual ([System.IO.Path]::GetFullPath($resolvedX64).ToLowerInvariant()) -Expected ([System.IO.Path]::GetFullPath($expectedX64).ToLowerInvariant()) -Message "Expected x64-first LIB entries to resolve the x64 kernel32.lib."

        $resolvedShadowed = Find-MsvcX64Kernel32LibPath -LibEntries @($x86LibRoot, $x64LibRoot)
        Assert-True -Condition ([string]::IsNullOrEmpty($resolvedShadowed)) -Message "Expected earlier x86 kernel32.lib entries to shadow later x64 candidates."

        $resolvedX86Only = Find-MsvcX64Kernel32LibPath -LibEntries @($x86LibRoot)
        Assert-True -Condition ([string]::IsNullOrEmpty($resolvedX86Only)) -Message "Expected x86-only LIB entries to be rejected for x64 builds."
    }
    finally {
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-ResolveWindowsSdkVersionRootSkipsIncompleteNewerSdk {
    $tempRoot = Join-Path $env:TEMP ("mergen-sdk-root-fallback-test-" + [guid]::NewGuid().ToString("N"))
    $programFilesRoot = Join-Path $tempRoot "Program Files"
    $newerSdkRoot = Join-Path $programFilesRoot "Windows Kits\10\Lib\10.0.26100.0"
    $olderSdkRoot = Join-Path $programFilesRoot "Windows Kits\10\Lib\10.0.22621.0"
    $originalProgramFiles = $env:ProgramFiles
    $originalProgramFilesX86 = ${env:ProgramFiles(x86)}

    New-Item -ItemType Directory -Path (Join-Path $newerSdkRoot "um\x64") -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $olderSdkRoot "um\x64") -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $olderSdkRoot "ucrt\x64") -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $programFilesRoot "Windows Kits\10\Include\10.0.22621.0\ucrt") -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $programFilesRoot "Windows Kits\10\Include\10.0.22621.0\um") -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $programFilesRoot "Windows Kits\10\Include\10.0.22621.0\shared") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $newerSdkRoot "um\x64\kernel32.lib") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $olderSdkRoot "um\x64\kernel32.lib") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $olderSdkRoot "ucrt\x64\ucrt.lib") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $programFilesRoot "Windows Kits\10\Include\10.0.22621.0\ucrt\corecrt.h") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $programFilesRoot "Windows Kits\10\Include\10.0.22621.0\um\Windows.h") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $programFilesRoot "Windows Kits\10\Include\10.0.22621.0\shared\sdkddkver.h") -Force | Out-Null

    try {
        Set-Item -Path Env:ProgramFiles -Value $programFilesRoot
        Set-Item -Path 'Env:ProgramFiles(x86)' -Value (Join-Path $tempRoot "Program Files (x86)\missing")

        $resolved = Resolve-WindowsSdkVersionRoot
        $expected = [System.IO.Path]::GetFullPath($olderSdkRoot).ToLowerInvariant()
        $actual = [System.IO.Path]::GetFullPath($resolved).ToLowerInvariant()
        Assert-Equal -Actual $actual -Expected $expected -Message "Expected Resolve-WindowsSdkVersionRoot to fall back to the next older complete SDK."
    }
    finally {
        if ($null -ne $originalProgramFiles) { Set-Item -Path Env:ProgramFiles -Value $originalProgramFiles } else { Remove-Item Env:ProgramFiles -ErrorAction SilentlyContinue }
        if ($null -ne $originalProgramFilesX86) { Set-Item -Path 'Env:ProgramFiles(x86)' -Value $originalProgramFilesX86 } else { Remove-Item 'Env:ProgramFiles(x86)' -ErrorAction SilentlyContinue }
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-ResolveWindowsSdkVersionRootReturnsNullWithoutCompleteSdk {
    $tempRoot = Join-Path $env:TEMP ("mergen-sdk-root-negative-test-" + [guid]::NewGuid().ToString("N"))
    $programFilesRoot = Join-Path $tempRoot "Program Files"
    $sdkRoot = Join-Path $programFilesRoot "Windows Kits\10\Lib\10.0.26100.0"
    $originalProgramFiles = $env:ProgramFiles
    $originalProgramFilesX86 = ${env:ProgramFiles(x86)}

    New-Item -ItemType Directory -Path (Join-Path $sdkRoot "um\x64") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $sdkRoot "um\x64\kernel32.lib") -Force | Out-Null

    try {
        Set-Item -Path Env:ProgramFiles -Value $programFilesRoot
        Set-Item -Path 'Env:ProgramFiles(x86)' -Value (Join-Path $tempRoot "Program Files (x86)\missing")

        $resolved = Resolve-WindowsSdkVersionRoot
        Assert-True -Condition ([string]::IsNullOrEmpty($resolved)) -Message "Expected Resolve-WindowsSdkVersionRoot to return null when no SDK version is build-ready."
    }
    finally {
        if ($null -ne $originalProgramFiles) { Set-Item -Path Env:ProgramFiles -Value $originalProgramFiles } else { Remove-Item Env:ProgramFiles -ErrorAction SilentlyContinue }
        if ($null -ne $originalProgramFilesX86) { Set-Item -Path 'Env:ProgramFiles(x86)' -Value $originalProgramFilesX86 } else { Remove-Item 'Env:ProgramFiles(x86)' -ErrorAction SilentlyContinue }
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-GetMsvcX64LibrarySetStatusRequiresSdkAndMsvcCrtLibs {
    $tempRoot = Join-Path $env:TEMP ("mergen-msvc-libset-test-" + [guid]::NewGuid().ToString("N"))
    $umX64Root = Join-Path $tempRoot "Windows Kits\10\Lib\10.0.26100.0\um\x64"
    $ucrtX64Root = Join-Path $tempRoot "Windows Kits\10\Lib\10.0.26100.0\ucrt\x64"
    $msvcX64Root = Join-Path $tempRoot "VC\Tools\MSVC\14.44.35207\lib\x64"
    New-Item -ItemType Directory -Path $umX64Root -Force | Out-Null
    New-Item -ItemType Directory -Path $ucrtX64Root -Force | Out-Null
    New-Item -ItemType Directory -Path $msvcX64Root -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $umX64Root "kernel32.lib") -Force | Out-Null

    try {
        $kernelOnly = Get-MsvcX64LibrarySetStatus -LibEntries @($umX64Root)
        Assert-True -Condition (-not $kernelOnly.IsReady) -Message "Expected kernel32.lib alone to be insufficient for x64 MSVC readiness."

        New-Item -ItemType File -Path (Join-Path $ucrtX64Root "ucrt.lib") -Force | Out-Null
        $kernelAndUcrt = Get-MsvcX64LibrarySetStatus -LibEntries @($umX64Root, $ucrtX64Root)
        Assert-True -Condition (-not $kernelAndUcrt.IsReady) -Message "Expected kernel32.lib plus ucrt.lib to remain insufficient without the MSVC CRT library path."

        New-Item -ItemType File -Path (Join-Path $msvcX64Root "vcruntime.lib") -Force | Out-Null
        $complete = Get-MsvcX64LibrarySetStatus -LibEntries @($umX64Root, $ucrtX64Root, $msvcX64Root)
        Assert-True -Condition $complete.IsReady -Message "Expected x64 kernel32.lib, ucrt.lib, and vcruntime.lib together to satisfy the full MSVC readiness check."
    }
    finally {
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-GetMsvcX64LibrarySetStatusHonorsLibOrder {
    $tempRoot = Join-Path $env:TEMP ("mergen-msvc-lib-order-test-" + [guid]::NewGuid().ToString("N"))
    $umX64Root = Join-Path $tempRoot "Windows Kits\10\Lib\10.0.26100.0\um\x64"
    $umX86Root = Join-Path $tempRoot "Windows Kits\10\Lib\10.0.26100.0\um\x86"
    $ucrtX64Root = Join-Path $tempRoot "Windows Kits\10\Lib\10.0.26100.0\ucrt\x64"
    $ucrtX86Root = Join-Path $tempRoot "Windows Kits\10\Lib\10.0.26100.0\ucrt\x86"
    $msvcX64Root = Join-Path $tempRoot "VC\Tools\MSVC\14.44.35207\lib\x64"
    $msvcX86Root = Join-Path $tempRoot "VC\Tools\MSVC\14.44.35207\lib\x86"

    foreach ($path in @($umX64Root, $umX86Root, $ucrtX64Root, $ucrtX86Root, $msvcX64Root, $msvcX86Root)) {
        New-Item -ItemType Directory -Path $path -Force | Out-Null
    }

    New-Item -ItemType File -Path (Join-Path $umX64Root "kernel32.lib") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $umX86Root "kernel32.lib") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $ucrtX64Root "ucrt.lib") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $ucrtX86Root "ucrt.lib") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $msvcX64Root "vcruntime.lib") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $msvcX86Root "vcruntime.lib") -Force | Out-Null

    try {
        $shadowed = Get-MsvcX64LibrarySetStatus -LibEntries @($umX86Root, $umX64Root, $ucrtX86Root, $ucrtX64Root, $msvcX86Root, $msvcX64Root)
        Assert-True -Condition (-not $shadowed.IsReady) -Message "Expected x86-first LIB entries to fail the x64 readiness check."
        Assert-True -Condition ([string]::IsNullOrEmpty($shadowed.Kernel32Path)) -Message "Expected shadowed kernel32.lib to be treated as unavailable."
        Assert-True -Condition ([string]::IsNullOrEmpty($shadowed.UcrtPath)) -Message "Expected shadowed ucrt.lib to be treated as unavailable."
        Assert-True -Condition ([string]::IsNullOrEmpty($shadowed.VcRuntimePath)) -Message "Expected shadowed vcruntime.lib to be treated as unavailable."

        $preferred = Get-MsvcX64LibrarySetStatus -LibEntries @($umX64Root, $umX86Root, $ucrtX64Root, $ucrtX86Root, $msvcX64Root, $msvcX86Root)
        Assert-True -Condition $preferred.IsReady -Message "Expected x64-first LIB entries to remain build-ready even when x86 duplicates exist later."
        Assert-Equal -Actual ([System.IO.Path]::GetFullPath($preferred.Kernel32Path).ToLowerInvariant()) -Expected ([System.IO.Path]::GetFullPath((Join-Path $umX64Root "kernel32.lib")).ToLowerInvariant()) -Message "Expected kernel32.lib to resolve from the first x64 LIB entry."
        Assert-Equal -Actual ([System.IO.Path]::GetFullPath($preferred.UcrtPath).ToLowerInvariant()) -Expected ([System.IO.Path]::GetFullPath((Join-Path $ucrtX64Root "ucrt.lib")).ToLowerInvariant()) -Message "Expected ucrt.lib to resolve from the first x64 LIB entry."
        Assert-Equal -Actual ([System.IO.Path]::GetFullPath($preferred.VcRuntimePath).ToLowerInvariant()) -Expected ([System.IO.Path]::GetFullPath((Join-Path $msvcX64Root "vcruntime.lib")).ToLowerInvariant()) -Message "Expected vcruntime.lib to resolve from the first x64 LIB entry."
    }
    finally {
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-AssertMsvcBuildEnvironmentConfiguresWhenOnlyKernel32IsPresent {
    $originalResolveLinker = (Get-Command Resolve-MsvcLinkerPath).ScriptBlock
    $originalGetLibStatus = (Get-Command Get-MsvcX64LibrarySetStatus).ScriptBlock
    $originalConfigure = (Get-Command Configure-DirectMsvcEnvironment).ScriptBlock
    [int]$script:configureCalls = 0
    [int]$script:libStatusCalls = 0

    try {
        Set-Item -Path Function:\Resolve-MsvcLinkerPath -Value {
            "C:\VS\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64\link.exe"
        }
        Set-Item -Path Function:\Get-MsvcX64LibrarySetStatus -Value {
            param([string[]]$LibEntries)
            $script:libStatusCalls++
            if ($script:libStatusCalls -eq 1) {
                return [pscustomobject]@{
                    Kernel32Path = "C:\SDK\um\x64\kernel32.lib"
                    UcrtPath = $null
                    VcRuntimePath = $null
                    IsReady = $false
                }
            }

            return [pscustomobject]@{
                Kernel32Path = "C:\SDK\um\x64\kernel32.lib"
                UcrtPath = "C:\SDK\ucrt\x64\ucrt.lib"
                VcRuntimePath = "C:\VS\VC\Tools\MSVC\14.44.35207\lib\x64\vcruntime.lib"
                IsReady = $true
            }
        }
        Set-Item -Path Function:\Configure-DirectMsvcEnvironment -Value {
            $script:configureCalls++
        }

        Assert-MsvcBuildEnvironment

        Assert-Equal -Actual $script:configureCalls -Expected 1 -Message "Expected Assert-MsvcBuildEnvironment to keep configuring the environment when only kernel32.lib is present."
    }
    finally {
        Set-Item -Path Function:\Resolve-MsvcLinkerPath -Value $originalResolveLinker
        Set-Item -Path Function:\Get-MsvcX64LibrarySetStatus -Value $originalGetLibStatus
        Set-Item -Path Function:\Configure-DirectMsvcEnvironment -Value $originalConfigure
        Remove-Variable -Name configureCalls -Scope Script -ErrorAction SilentlyContinue
        Remove-Variable -Name libStatusCalls -Scope Script -ErrorAction SilentlyContinue
    }
}

function Test-AssertMsvcBuildEnvironmentAcceptsHostarm64X64Linker {
    $originalResolveLinker = (Get-Command Resolve-MsvcLinkerPath).ScriptBlock
    $originalGetLibStatus = (Get-Command Get-MsvcX64LibrarySetStatus).ScriptBlock
    $originalConfigure = (Get-Command Configure-DirectMsvcEnvironment).ScriptBlock
    [int]$script:configureCalls = 0

    try {
        Set-Item -Path Function:\Resolve-MsvcLinkerPath -Value {
            "C:\VS\VC\Tools\MSVC\14.44.35207\bin\Hostarm64\x64\link.exe"
        }
        Set-Item -Path Function:\Get-MsvcX64LibrarySetStatus -Value {
            param([string[]]$LibEntries)
            [pscustomobject]@{
                Kernel32Path = "C:\SDK\um\x64\kernel32.lib"
                UcrtPath = "C:\SDK\ucrt\x64\ucrt.lib"
                VcRuntimePath = "C:\VS\VC\Tools\MSVC\14.44.35207\lib\x64\vcruntime.lib"
                IsReady = $true
            }
        }
        Set-Item -Path Function:\Configure-DirectMsvcEnvironment -Value {
            $script:configureCalls++
        }

        Assert-MsvcBuildEnvironment

        Assert-Equal -Actual $script:configureCalls -Expected 0 -Message "Expected Assert-MsvcBuildEnvironment to accept an already ready Hostarm64\\x64 linker without reconfiguring the shell."
    }
    finally {
        Set-Item -Path Function:\Resolve-MsvcLinkerPath -Value $originalResolveLinker
        Set-Item -Path Function:\Get-MsvcX64LibrarySetStatus -Value $originalGetLibStatus
        Set-Item -Path Function:\Configure-DirectMsvcEnvironment -Value $originalConfigure
        Remove-Variable -Name configureCalls -Scope Script -ErrorAction SilentlyContinue
    }
}

function Test-AssertMsvcBuildEnvironmentConfiguresWhenX86LibsShadowX64 {
    $tempRoot = Join-Path $env:TEMP ("mergen-msvc-assert-shadowed-lib-test-" + [guid]::NewGuid().ToString("N"))
    $umX64Root = Join-Path $tempRoot "Windows Kits\10\Lib\10.0.26100.0\um\x64"
    $umX86Root = Join-Path $tempRoot "Windows Kits\10\Lib\10.0.26100.0\um\x86"
    $ucrtX64Root = Join-Path $tempRoot "Windows Kits\10\Lib\10.0.26100.0\ucrt\x64"
    $ucrtX86Root = Join-Path $tempRoot "Windows Kits\10\Lib\10.0.26100.0\ucrt\x86"
    $msvcX64Root = Join-Path $tempRoot "VC\Tools\MSVC\14.44.35207\lib\x64"
    $msvcX86Root = Join-Path $tempRoot "VC\Tools\MSVC\14.44.35207\lib\x86"
    $originalResolveLinker = (Get-Command Resolve-MsvcLinkerPath).ScriptBlock
    $originalConfigure = (Get-Command Configure-DirectMsvcEnvironment).ScriptBlock
    $originalLib = $env:LIB

    foreach ($path in @($umX64Root, $umX86Root, $ucrtX64Root, $ucrtX86Root, $msvcX64Root, $msvcX86Root)) {
        New-Item -ItemType Directory -Path $path -Force | Out-Null
    }

    New-Item -ItemType File -Path (Join-Path $umX64Root "kernel32.lib") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $umX86Root "kernel32.lib") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $ucrtX64Root "ucrt.lib") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $ucrtX86Root "ucrt.lib") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $msvcX64Root "vcruntime.lib") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $msvcX86Root "vcruntime.lib") -Force | Out-Null

    try {
        Set-Item -Path Env:LIB -Value (($umX86Root, $umX64Root, $ucrtX86Root, $ucrtX64Root, $msvcX86Root, $msvcX64Root) -join ";")
        Set-Item -Path Function:\Resolve-MsvcLinkerPath -Value {
            "C:\VS\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64\link.exe"
        }
        Set-Item -Path Env:MERGEN_CONFIGURE_CALLS -Value "0"
        $preConfigureStatus = Get-MsvcX64LibrarySetStatus -LibEntries @($env:LIB -split ";" | Where-Object { $_ })
        Assert-True -Condition (-not $preConfigureStatus.IsReady) -Message "Expected x86-first LIB ordering to fail readiness before Configure-DirectMsvcEnvironment runs."
        Set-Item -Path Function:\Configure-DirectMsvcEnvironment -Value {
            Set-Item -Path Env:MERGEN_CONFIGURE_CALLS -Value ([string](([int]$env:MERGEN_CONFIGURE_CALLS) + 1))
            Set-Item -Path Env:LIB -Value (($umX64Root, $ucrtX64Root, $msvcX64Root) -join ";")
        }.GetNewClosure()

        Assert-MsvcBuildEnvironment

        Assert-Equal -Actual $env:MERGEN_CONFIGURE_CALLS -Expected "1" -Message "Expected Assert-MsvcBuildEnvironment to reconfigure when x86 LIB entries shadow required x64 libraries."
    }
    finally {
        Set-Item -Path Function:\Resolve-MsvcLinkerPath -Value $originalResolveLinker
        Set-Item -Path Function:\Configure-DirectMsvcEnvironment -Value $originalConfigure
        if ($null -ne $originalLib) { Set-Item -Path Env:LIB -Value $originalLib } else { Remove-Item Env:LIB -ErrorAction SilentlyContinue }
        Remove-Item Env:MERGEN_CONFIGURE_CALLS -ErrorAction SilentlyContinue
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-ConfigureDirectMsvcEnvironmentUsesResolvedX64Linker {
    $tempRoot = Join-Path $env:TEMP ("mergen-msvc-configure-test-" + [guid]::NewGuid().ToString("N"))
    $toolRoot = Join-Path $tempRoot "VC\Tools\MSVC\14.44.35207"
    $linkerPath = Join-Path $toolRoot "bin\Hostx86\x64\link.exe"
    $sdkRoot = Join-Path $tempRoot "Windows Kits\10\Lib\10.0.26100.0"
    $originalResolveToolRoot = (Get-Command Resolve-MsvcToolRoot).ScriptBlock
    $originalResolveSdkRoot = (Get-Command Resolve-WindowsSdkVersionRoot).ScriptBlock
    $originalPath = $env:Path
    $originalLib = $env:LIB
    $originalInclude = $env:INCLUDE
    $originalLinker = $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER
    $originalVcTools = $env:VCToolsInstallDir
    $originalSdkDir = $env:WindowsSdkDir
    $originalSdkVersion = $env:WindowsSdkVersion

    New-Item -ItemType Directory -Path (Split-Path -Parent $linkerPath) -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $toolRoot "lib\x64") -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $toolRoot "include") -Force | Out-Null
    New-Item -ItemType File -Path $linkerPath -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $tempRoot "Windows Kits\10\bin\10.0.26100.0\x64") -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $tempRoot "Windows Kits\10\Include\10.0.26100.0\ucrt") -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $tempRoot "Windows Kits\10\Include\10.0.26100.0\um") -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $tempRoot "Windows Kits\10\Include\10.0.26100.0\shared") -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $tempRoot "Windows Kits\10\Include\10.0.26100.0\winrt") -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $tempRoot "Windows Kits\10\Include\10.0.26100.0\cppwinrt") -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $sdkRoot "um\x64") -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $sdkRoot "ucrt\x64") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $toolRoot "lib\x64\vcruntime.lib") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $sdkRoot "um\x64\kernel32.lib") -Force | Out-Null
    New-Item -ItemType File -Path (Join-Path $sdkRoot "ucrt\x64\ucrt.lib") -Force | Out-Null

    try {
        Set-Item -Path Function:\Resolve-MsvcToolRoot -Value {
            $toolRoot
        }
        Set-Item -Path Function:\Resolve-WindowsSdkVersionRoot -Value {
            $sdkRoot
        }

        Configure-DirectMsvcEnvironment

        Assert-Equal -Actual ([System.IO.Path]::GetFullPath($env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER).ToLowerInvariant()) -Expected ([System.IO.Path]::GetFullPath($linkerPath).ToLowerInvariant()) -Message "Expected Configure-DirectMsvcEnvironment to export the resolved x64 linker path."
        Assert-True -Condition (($env:Path -split ";") -contains (Split-Path -Parent $linkerPath)) -Message "Expected Configure-DirectMsvcEnvironment to prepend the resolved linker directory to PATH."
    }
    finally {
        Set-Item -Path Function:\Resolve-MsvcToolRoot -Value $originalResolveToolRoot
        Set-Item -Path Function:\Resolve-WindowsSdkVersionRoot -Value $originalResolveSdkRoot
        if ($null -ne $originalPath) { Set-Item -Path Env:Path -Value $originalPath } else { Remove-Item Env:Path -ErrorAction SilentlyContinue }
        if ($null -ne $originalLib) { Set-Item -Path Env:LIB -Value $originalLib } else { Remove-Item Env:LIB -ErrorAction SilentlyContinue }
        if ($null -ne $originalInclude) { Set-Item -Path Env:INCLUDE -Value $originalInclude } else { Remove-Item Env:INCLUDE -ErrorAction SilentlyContinue }
        if ($null -ne $originalLinker) { Set-Item -Path Env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER -Value $originalLinker } else { Remove-Item Env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER -ErrorAction SilentlyContinue }
        if ($null -ne $originalVcTools) { Set-Item -Path Env:VCToolsInstallDir -Value $originalVcTools } else { Remove-Item Env:VCToolsInstallDir -ErrorAction SilentlyContinue }
        if ($null -ne $originalSdkDir) { Set-Item -Path Env:WindowsSdkDir -Value $originalSdkDir } else { Remove-Item Env:WindowsSdkDir -ErrorAction SilentlyContinue }
        if ($null -ne $originalSdkVersion) { Set-Item -Path Env:WindowsSdkVersion -Value $originalSdkVersion } else { Remove-Item Env:WindowsSdkVersion -ErrorAction SilentlyContinue }
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-ImportVsDevEnvironmentCleansUpWrapperScript {
    $tempRoot = Join-Path $env:TEMP ("mergen-vsdev-import-test-" + [guid]::NewGuid().ToString("N"))
    $vsDevCmdPath = Join-Path $tempRoot "VsDevCmd.bat"
    $before = @(
        Get-ChildItem -Path ([System.IO.Path]::GetTempPath()) -Filter "mergen-vsdevcmd-*" -File -ErrorAction SilentlyContinue |
        ForEach-Object { $_.FullName.ToLowerInvariant() }
    )

    New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null
    @(
        '@echo off',
        'set MERGEN_VSDEV_IMPORT_TEST=1',
        'exit /b 0'
    ) | Set-Content -Path $vsDevCmdPath -Encoding ASCII

    try {
        Import-VsDevEnvironment -VsDevCmdPath $vsDevCmdPath

        $after = @(
            Get-ChildItem -Path ([System.IO.Path]::GetTempPath()) -Filter "mergen-vsdevcmd-*" -File -ErrorAction SilentlyContinue |
            ForEach-Object { $_.FullName.ToLowerInvariant() }
        )

        Assert-Equal -Actual (($after | Sort-Object) -join "`n") -Expected (($before | Sort-Object) -join "`n") -Message "Expected Import-VsDevEnvironment to clean up its temporary wrapper script."
        Assert-Equal -Actual $env:MERGEN_VSDEV_IMPORT_TEST -Expected "1" -Message "Expected Import-VsDevEnvironment to import variables from the temporary VsDevCmd wrapper."
    }
    finally {
        Remove-Item Env:MERGEN_VSDEV_IMPORT_TEST -ErrorAction SilentlyContinue
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-ImportVsDevEnvironmentPreservesVsDevCmdFailure {
    $tempRoot = Join-Path $env:TEMP ("mergen-vsdev-import-failure-test-" + [guid]::NewGuid().ToString("N"))
    $vsDevCmdPath = Join-Path $tempRoot "VsDevCmd.bat"

    New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null
    @(
        '@echo off',
        'set MERGEN_VSDEV_IMPORT_SHOULD_NOT_LEAK=1',
        'exit /b 42'
    ) | Set-Content -Path $vsDevCmdPath -Encoding ASCII

    try {
        try {
            Import-VsDevEnvironment -VsDevCmdPath $vsDevCmdPath
            throw "Expected Import-VsDevEnvironment to fail when VsDevCmd.bat exits non-zero."
        }
        catch {
            Assert-True -Condition $_.Exception.Message.Contains("Failed to import Visual Studio build environment from $vsDevCmdPath") -Message "Expected Import-VsDevEnvironment to preserve a failing VsDevCmd exit."
        }

        Assert-True -Condition (-not $env:MERGEN_VSDEV_IMPORT_SHOULD_NOT_LEAK) -Message "Expected a failing VsDevCmd import to avoid leaking partial environment variables into the PowerShell session."
    }
    finally {
        Remove-Item Env:MERGEN_VSDEV_IMPORT_SHOULD_NOT_LEAK -ErrorAction SilentlyContinue
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-EnsureMsvcBuildEnvironmentRetriesAfterImportFailure {
    $originalAssert = (Get-Command Assert-MsvcBuildEnvironment).ScriptBlock
    $originalResolveCandidates = (Get-Command Resolve-VsDevCmdCandidates).ScriptBlock
    $originalImport = (Get-Command Import-VsDevEnvironment).ScriptBlock
    [int]$script:importFailureAssertCalls = 0
    [int]$script:importFailureImportCalls = 0
    $script:firstImportFailureCandidate = "C:\VS\Broken\Common7\Tools\VsDevCmd.bat"
    $script:secondImportFailureCandidate = "C:\VS\Working\Common7\Tools\VsDevCmd.bat"

    try {
        Remove-Item Env:MERGEN_IMPORTED_CANDIDATE -ErrorAction SilentlyContinue

        Set-Item -Path Function:\Assert-MsvcBuildEnvironment -Value {
            $script:importFailureAssertCalls++
            switch ($script:importFailureAssertCalls) {
                1 { throw "initial shell failed" }
                2 {
                    if ($env:MERGEN_IMPORTED_CANDIDATE -ne $script:secondImportFailureCandidate) {
                        throw "Expected retry after import failure to continue with the second VsDevCmd candidate."
                    }
                }
                default { throw "Unexpected Assert-MsvcBuildEnvironment call count." }
            }
        }
        Set-Item -Path Function:\Resolve-VsDevCmdCandidates -Value {
            @($script:firstImportFailureCandidate, $script:secondImportFailureCandidate)
        }
        Set-Item -Path Function:\Import-VsDevEnvironment -Value {
            param([string]$VsDevCmdPath)
            $script:importFailureImportCalls++
            if ($VsDevCmdPath -eq $script:firstImportFailureCandidate) {
                throw "Failed to import Visual Studio build environment from $VsDevCmdPath"
            }

            Set-Item -Path Env:MERGEN_IMPORTED_CANDIDATE -Value $VsDevCmdPath
        }

        Ensure-MsvcBuildEnvironment

        Assert-Equal -Actual $script:importFailureImportCalls -Expected 2 -Message "Expected Ensure-MsvcBuildEnvironment to retry the next VsDevCmd candidate after an import failure."
        Assert-Equal -Actual $script:importFailureAssertCalls -Expected 2 -Message "Expected Assert-MsvcBuildEnvironment to rerun after the later successful import."
    }
    finally {
        Set-Item -Path Function:\Assert-MsvcBuildEnvironment -Value $originalAssert
        Set-Item -Path Function:\Resolve-VsDevCmdCandidates -Value $originalResolveCandidates
        Set-Item -Path Function:\Import-VsDevEnvironment -Value $originalImport
        Remove-Item Env:MERGEN_IMPORTED_CANDIDATE -ErrorAction SilentlyContinue
        Remove-Variable -Name importFailureAssertCalls -Scope Script -ErrorAction SilentlyContinue
        Remove-Variable -Name importFailureImportCalls -Scope Script -ErrorAction SilentlyContinue
        Remove-Variable -Name firstImportFailureCandidate -Scope Script -ErrorAction SilentlyContinue
        Remove-Variable -Name secondImportFailureCandidate -Scope Script -ErrorAction SilentlyContinue
    }
}

function Test-UnsupportedConvenienceArtifactsExcludeGnullvmDevOutputs {
    $paths = @(Get-UnsupportedConvenienceArtifactPaths)

    Assert-Equal -Actual $paths.Count -Expected 2 -Message "Expected only unsupported convenience artifact paths to be cleaned."
    Assert-True -Condition ($paths -contains (Join-Path $repoRoot "target\release\mergen-ade.exe")) -Message "Expected flat target\\release EXE to remain an unsupported cleanup target."
    Assert-True -Condition ($paths -contains (Join-Path $repoRoot "mergen-ade.exe")) -Message "Expected repo-root EXE to remain an unsupported cleanup target."
    Assert-True -Condition (-not ($paths -contains (Join-Path $repoRoot "target\x86_64-pc-windows-gnullvm\release\mergen-ade.exe"))) -Message "Expected the gnullvm dev EXE to survive release cleanup."
    Assert-True -Condition (-not ($paths -contains (Join-Path $repoRoot "target\x86_64-pc-windows-gnullvm\release\libunwind.dll"))) -Message "Expected the gnullvm libunwind.dll to survive release cleanup."
}

function Test-RemoveUnsupportedConvenienceArtifactsFailsWhenDeletionFails {
    $originalGetPaths = (Get-Command Get-UnsupportedConvenienceArtifactPaths).ScriptBlock
    $removeItemFunctionExists = Test-Path Function:\Remove-Item
    $originalRemoveItem = $null
    if ($removeItemFunctionExists) {
        $originalRemoveItem = (Get-Command Remove-Item).ScriptBlock
    }
    $testPathFunctionExists = Test-Path Function:\Test-Path
    $originalTestPath = $null
    if ($testPathFunctionExists) {
        $originalTestPath = (Get-Command Test-Path).ScriptBlock
    }
    $lockedPath = Join-Path $repoRoot "mergen-ade.exe"

    try {
        Set-Item -Path Function:\Get-UnsupportedConvenienceArtifactPaths -Value {
            @($lockedPath)
        }.GetNewClosure()
        Set-Item -Path Function:\Test-Path -Value {
            param([string]$Path)
            return $Path -eq $lockedPath
        }.GetNewClosure()
        Set-Item -Path Function:\Remove-Item -Value {
            param(
                [string]$Path,
                [switch]$Force,
                [string]$ErrorAction
            )
            throw "file is locked"
        }

        try {
            Remove-UnsupportedConvenienceArtifacts
            throw "Expected Remove-UnsupportedConvenienceArtifacts to fail when a stale unsupported executable cannot be removed."
        }
        catch {
            $message = $_.Exception.Message
            Assert-True -Condition $message.Contains("Unsupported convenience artifacts could not be removed") -Message "Expected cleanup failure to be fatal."
            Assert-True -Condition $message.Contains($lockedPath) -Message "Expected cleanup failure to report the locked unsupported executable path."
        }
    }
    finally {
        Set-Item -Path Function:\Get-UnsupportedConvenienceArtifactPaths -Value $originalGetPaths
        if ($removeItemFunctionExists) {
            Set-Item -Path Function:\Remove-Item -Value $originalRemoveItem
        }
        else {
            Microsoft.PowerShell.Management\Remove-Item Function:\Remove-Item -ErrorAction SilentlyContinue
        }

        if ($testPathFunctionExists) {
            Set-Item -Path Function:\Test-Path -Value $originalTestPath
        }
        else {
            Microsoft.PowerShell.Management\Remove-Item Function:\Test-Path -ErrorAction SilentlyContinue
        }
    }
}

function Test-CleanupRunsAfterSuccessfulBuildValidation {
    $scriptContent = Get-Content -Path (Join-Path $repoRoot "scripts\build-release.ps1") -Raw
    $buildIndex = $scriptContent.IndexOf('& $cargo build --release --target $target -j 1')
    $importValidationIndex = $scriptContent.IndexOf('$blockedImports = $imports | Where-Object { $blockedDlls -contains $_ }')
    $cleanupIndex = $scriptContent.LastIndexOf('Remove-UnsupportedConvenienceArtifacts')
    $successMessageIndex = $scriptContent.LastIndexOf('Write-Host "Portable release EXE ready: $exePath"')

    Assert-True -Condition ($buildIndex -ge 0) -Message "Expected the release script to invoke the MSVC cargo build."
    Assert-True -Condition ($importValidationIndex -ge 0) -Message "Expected the release script to validate blocked DLL imports."
    Assert-True -Condition ($cleanupIndex -ge 0) -Message "Expected the release script to clean unsupported convenience artifacts on the success path."
    Assert-True -Condition ($successMessageIndex -ge 0) -Message "Expected the release script to print a success message after cleanup."
    Assert-True -Condition ($cleanupIndex -gt $buildIndex) -Message "Expected cleanup to run after the MSVC build command."
    Assert-True -Condition ($cleanupIndex -gt $importValidationIndex) -Message "Expected cleanup to run after import validation passes."
    Assert-True -Condition ($cleanupIndex -lt $successMessageIndex) -Message "Expected cleanup to finish before the success message is printed."
}

function Test-WindowsIconPipelineIsConfigured {
    $buildRsPath = Join-Path $repoRoot "build.rs"
    $mainRsPath = Join-Path $repoRoot "src\main.rs"
    $logoPath = Join-Path $repoRoot "logo.png"

    Assert-True -Condition (Test-Path $buildRsPath) -Message "Expected build.rs to exist for Windows icon generation."
    Assert-True -Condition (Test-Path $logoPath) -Message "Expected logo.png to exist as the committed icon source asset."

    $buildContent = Get-Content -Path $buildRsPath -Raw
    $mainContent = Get-Content -Path $mainRsPath -Raw

    Assert-True -Condition $buildContent.Contains('const SOURCE_LOGO: &str = "logo.png";') -Message "Expected build.rs to use logo.png as the icon source asset."
    Assert-True -Condition $buildContent.Contains('const OUTPUT_PNG: &str = "app-icon.png";') -Message "Expected build.rs to generate a runtime PNG icon."
    Assert-True -Condition $buildContent.Contains('cargo:rerun-if-env-changed=WindowsSdkDir') -Message "Expected build.rs to rerun when WindowsSdkDir changes."
    Assert-True -Condition $buildContent.Contains('cargo:rerun-if-env-changed=WindowsSdkVersion') -Message "Expected build.rs to rerun when WindowsSdkVersion changes."
    Assert-True -Condition $buildContent.Contains('cargo:rerun-if-env-changed=ProgramFiles') -Message "Expected build.rs to rerun when ProgramFiles changes."
    Assert-True -Condition $buildContent.Contains('cargo:rerun-if-env-changed=ProgramFiles(x86)') -Message "Expected build.rs to rerun when ProgramFiles(x86) changes."
    Assert-True -Condition $buildContent.Contains('cargo:rerun-if-env-changed=PATH') -Message "Expected build.rs to rerun when PATH changes."
    Assert-True -Condition $buildContent.Contains('if target_os == "windows" && !is_test_build {') -Message "Expected build.rs to compile the icon resource for all non-test Windows targets."
    Assert-True -Condition $buildContent.Contains('if target.contains("-windows-gnu") {') -Message "Expected build.rs to configure a GNU resource compiler path for the default gnullvm build."
    Assert-True -Condition $buildContent.Contains('.toolchain/llvm-mingw-20260224-ucrt-x86_64/bin') -Message "Expected build.rs to use the repo-local LLVM-MinGW toolkit for GNU resource compilation."
    Assert-True -Condition $buildContent.Contains('.set_windres_path(') -Message "Expected build.rs to explicitly set the GNU windres path."
    Assert-True -Condition $buildContent.Contains('.set_ar_path(') -Message "Expected build.rs to explicitly set the GNU ar path."
    Assert-True -Condition $buildContent.Contains('Skipping GNU exe icon embedding') -Message "Expected build.rs to skip GNU resource compilation gracefully when repo-local tools are unavailable."
    Assert-True -Condition $buildContent.Contains('else if target.contains("-windows-msvc") {') -Message "Expected build.rs to handle the MSVC target separately from GNU."
    Assert-True -Condition $buildContent.Contains('resolve_msvc_toolkit_path()') -Message "Expected build.rs to probe for an MSVC rc.exe toolkit path."
    Assert-True -Condition $buildContent.Contains('resolve_msvc_toolkit_path_from_path') -Message "Expected build.rs to fall back to PATH-based rc.exe detection for MSVC."
    Assert-True -Condition $buildContent.Contains('cargo:warning=Skipping MSVC exe icon embedding') -Message "Expected build.rs to skip MSVC resource compilation gracefully when rc.exe is unavailable."
    Assert-True -Condition $buildContent.Contains('resource.set_icon') -Message "Expected build.rs to embed the generated .ico into the Windows executable."
    Assert-True -Condition $mainContent.Contains('.with_icon(app_icon)') -Message "Expected src/main.rs to set the runtime window icon."
    Assert-True -Condition $mainContent.Contains('include_bytes!(concat!(') -Message "Expected src/main.rs to load the generated runtime icon at compile time."
}

Test-CargoConfigRetainsRepoLocalGnullvmLinker
Test-SortVsInstallationsPrefersHealthyBuildTools
Test-FindDumpbinUnderRootsFallsBackToProgramFilesSearch
Test-ResolveVsDevCmdSkipsInstallsWithoutMsvcTools
Test-ResolveVsDevCmdFilesystemFallbackUsesInstallRoot
Test-EnsureMsvcBuildEnvironmentSkipsVsDevCmdWhenShellIsAlreadyReady
Test-EnsureMsvcBuildEnvironmentRetriesLaterVsDevCmdCandidates
Test-EnsureMsvcBuildEnvironmentRestoresEnvironmentBetweenRetries
Test-EnsureMsvcBuildEnvironmentReportsAllFailedVsDevCmdAttempts
Test-ResolveMsvcToolRootAcceptsHostx86X64Linker
Test-ResolveMsvcToolRootAcceptsHostarm64X64Linker
Test-ResolveMsvcToolRootForInstallSkipsIncompleteNewerToolsets
Test-ResolveMsvcToolRootForInstallReturnsNullWithoutCompleteToolsets
Test-TestMsvcLinkerTargetsX64RejectsX86Linkers
Test-FindMsvcX64Kernel32LibPathRejectsX86Entries
Test-ResolveWindowsSdkVersionRootSkipsIncompleteNewerSdk
Test-ResolveWindowsSdkVersionRootReturnsNullWithoutCompleteSdk
Test-GetMsvcX64LibrarySetStatusRequiresSdkAndMsvcCrtLibs
Test-GetMsvcX64LibrarySetStatusHonorsLibOrder
Test-AssertMsvcBuildEnvironmentConfiguresWhenOnlyKernel32IsPresent
Test-AssertMsvcBuildEnvironmentAcceptsHostarm64X64Linker
Test-AssertMsvcBuildEnvironmentConfiguresWhenX86LibsShadowX64
Test-ConfigureDirectMsvcEnvironmentUsesResolvedX64Linker
Test-ImportVsDevEnvironmentCleansUpWrapperScript
Test-ImportVsDevEnvironmentPreservesVsDevCmdFailure
Test-EnsureMsvcBuildEnvironmentRetriesAfterImportFailure
Test-UnsupportedConvenienceArtifactsExcludeGnullvmDevOutputs
Test-RemoveUnsupportedConvenienceArtifactsFailsWhenDeletionFails
Test-CleanupRunsAfterSuccessfulBuildValidation
Test-WindowsIconPipelineIsConfigured

Write-Host "build-release PowerShell tests passed."
