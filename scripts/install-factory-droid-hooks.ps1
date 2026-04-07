$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Set-JsonPropertyValue {
    param(
        [Parameter(Mandatory = $true)]
        [psobject]$Object,
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [AllowNull()]
        $Value
    )

    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) {
        $Object | Add-Member -NotePropertyName $Name -NotePropertyValue $Value
    }
    else {
        $property.Value = $Value
    }
}

function New-FactorySettingsObject {
    return [pscustomobject]@{}
}

function Get-FactoryHooksRoot {
    param(
        [Parameter(Mandatory = $true)]
        [string]$HomeDir
    )

    return Join-Path $HomeDir ".factory\hooks"
}

function Get-FactorySettingsPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$HomeDir
    )

    return Join-Path $HomeDir ".factory\settings.json"
}

function Get-InstalledFactoryDroidHookPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$HomeDir
    )

    return Join-Path (Get-FactoryHooksRoot -HomeDir $HomeDir) "mergen-ade-droid-status.ps1"
}

function Get-FactoryHookEventNames {
    return @("UserPromptSubmit", "Notification", "Stop")
}

function Assert-FactoryHookScriptPathIsSafe {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$Label
    )

    if ([string]::IsNullOrWhiteSpace($Path)) {
        throw "$Label path is empty."
    }
}

function Escape-PowerShellSingleQuotedString {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Value
    )

    return $Value.Replace("'", "''")
}

function New-FactoryDroidEncodedCommand {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ScriptPath
    )

    $escapedScriptPath = Escape-PowerShellSingleQuotedString -Value $ScriptPath
    $bootstrap = "& '$escapedScriptPath'"
    $bytes = [System.Text.Encoding]::Unicode.GetBytes($bootstrap)
    return [Convert]::ToBase64String($bytes)
}

function Get-FactoryDroidPowerShellCommandToken {
    $candidatePaths = @(
        (Join-Path $PSHOME "powershell.exe")
        (Join-Path $PSHOME "pwsh.exe")
    )

    foreach ($commandName in @("powershell.exe", "pwsh.exe")) {
        $commandInfo = Get-Command $commandName -CommandType Application -ErrorAction SilentlyContinue
        if ($null -ne $commandInfo) {
            $candidatePaths += $commandInfo.Source
        }
    }

    $seenCandidates = @{}
    foreach ($candidatePath in $candidatePaths) {
        if ([string]::IsNullOrWhiteSpace($candidatePath)) {
            continue
        }

        $resolvedCandidatePath = [System.IO.Path]::GetFullPath($candidatePath)
        $candidateKey = $resolvedCandidatePath.ToLowerInvariant()
        if ($seenCandidates.ContainsKey($candidateKey)) {
            continue
        }

        $seenCandidates[$candidateKey] = $true

        if (-not (Test-Path $resolvedCandidatePath)) {
            continue
        }

        if ($resolvedCandidatePath.Contains('"') -or $resolvedCandidatePath.Contains("'")) {
            continue
        }

        if ($resolvedCandidatePath -match "\s") {
            continue
        }

        return $resolvedCandidatePath
    }

    foreach ($commandName in @("powershell.exe", "pwsh.exe")) {
        if ($null -ne (Get-Command $commandName -CommandType Application -ErrorAction SilentlyContinue)) {
            return $commandName
        }
    }

    throw "Could not find a PowerShell executable for Factory Droid hooks."
}

function Get-FactoryDroidHookCommand {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ScriptPath
    )

    $resolvedScriptPath = [System.IO.Path]::GetFullPath($ScriptPath)
    $commandToken = Get-FactoryDroidPowerShellCommandToken
    Assert-FactoryHookScriptPathIsSafe -Path $resolvedScriptPath -Label "Factory Droid hook script"
    $encodedCommand = New-FactoryDroidEncodedCommand -ScriptPath $resolvedScriptPath
    return ('{0} -NoLogo -NonInteractive -NoProfile -ExecutionPolicy Bypass -EncodedCommand {1}' -f $commandToken, $encodedCommand)
}

function New-FactoryHookCommandEntry {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Command
    )

    return [pscustomobject]@{
        hooks = @(
            [pscustomobject]@{
                type    = "command"
                command = $Command
                timeout = 5
            }
        )
    }
}

function Test-IsManagedFactoryDroidHookCommand {
    param(
        [AllowNull()]
        [string]$Command,
        [AllowNull()]
        [string]$ManagedCommand
    )

    if ([string]::IsNullOrWhiteSpace($Command)) {
        return $false
    }

    if ($null -ne $ManagedCommand -and $Command -eq $ManagedCommand) {
        return $true
    }

    if ([string]$Command -match '(?i)mergen-ade-droid-status\.ps1') {
        return $true
    }

    $decoded = Get-FactoryDroidEncodedCommandScriptBlock -Command $Command
    if ($null -ne $decoded -and $decoded -match '(?i)mergen-ade-droid-status\.ps1') {
        return $true
    }

    return $false
}

function Get-FactoryDroidEncodedCommandScriptBlock {
    param(
        [AllowNull()]
        [string]$Command
    )

    if ([string]::IsNullOrWhiteSpace($Command)) {
        return $null
    }

    $match = [regex]::Match($Command, '(?i)-EncodedCommand\s+(\S+)')
    if (-not $match.Success) {
        return $null
    }

    try {
        $bytes = [Convert]::FromBase64String($match.Groups[1].Value)
        return [System.Text.Encoding]::Unicode.GetString($bytes)
    }
    catch {
        return $null
    }
}

function Get-FactoryHookCommandMatchCount {
    param(
        [AllowNull()]
        $EventEntries,
        [Parameter(Mandatory = $true)]
        [string]$Command
    )

    $matches = 0
    foreach ($entry in @($EventEntries)) {
        if ($null -eq $entry) {
            continue
        }

        foreach ($hook in @($entry.hooks)) {
            if ($null -eq $hook) {
                continue
            }

            if ([string]$hook.type -eq "command" -and [string]$hook.command -eq $Command) {
                $matches++
            }
        }
    }

    return $matches
}

function Test-FactoryHookCommandPresent {
    param(
        [AllowNull()]
        $EventEntries,
        [Parameter(Mandatory = $true)]
        [string]$Command
    )

    return (Get-FactoryHookCommandMatchCount -EventEntries $EventEntries -Command $Command) -gt 0
}

function Normalize-FactoryHookEventEntries {
    param(
        [AllowNull()]
        $EventEntries,
        [Parameter(Mandatory = $true)]
        [string]$Command
    )

    $normalizedEntries = @()
    $managedCommandPresent = $false

    foreach ($entry in @($EventEntries)) {
        if ($null -eq $entry) {
            continue
        }

        $hooksProperty = $entry.PSObject.Properties["hooks"]
        if ($null -eq $hooksProperty) {
            $normalizedEntries += ,$entry
            continue
        }

        $normalizedHooks = @()
        foreach ($hook in @($hooksProperty.Value)) {
            if ($null -eq $hook) {
                continue
            }

            if ([string]$hook.type -eq "command" -and (Test-IsManagedFactoryDroidHookCommand -Command ([string]$hook.command) -ManagedCommand $Command)) {
                if (-not $managedCommandPresent) {
                    Set-JsonPropertyValue -Object $hook -Name "command" -Value $Command
                    Set-JsonPropertyValue -Object $hook -Name "timeout" -Value 5
                    $normalizedHooks += ,$hook
                    $managedCommandPresent = $true
                }

                continue
            }

            $normalizedHooks += ,$hook
        }

        if ($normalizedHooks.Count -gt 0) {
            Set-JsonPropertyValue -Object $entry -Name "hooks" -Value $normalizedHooks
            $normalizedEntries += ,$entry
        }
    }

    if (-not $managedCommandPresent) {
        $normalizedEntries += ,(New-FactoryHookCommandEntry -Command $Command)
    }

    return ,$normalizedEntries
}

function Merge-FactoryHookSettings {
    param(
        [AllowNull()]
        [psobject]$Settings,
        [Parameter(Mandatory = $true)]
        [string]$Command
    )

    if ($null -eq $Settings) {
        $Settings = New-FactorySettingsObject
    }

    if ($null -eq $Settings.PSObject.Properties["hooks"]) {
        Set-JsonPropertyValue -Object $Settings -Name "hooks" -Value ([pscustomobject]@{})
    }
    elseif ($null -eq $Settings.hooks) {
        $Settings.hooks = [pscustomobject]@{}
    }

    foreach ($eventName in Get-FactoryHookEventNames) {
        $eventProperty = $Settings.hooks.PSObject.Properties[$eventName]
        $eventEntries = if ($null -eq $eventProperty) { @() } else { @($eventProperty.Value) }
        $normalizedEntries = Normalize-FactoryHookEventEntries -EventEntries $eventEntries -Command $Command
        Set-JsonPropertyValue -Object $Settings.hooks -Name $eventName -Value @($normalizedEntries)
    }

    return $Settings
}

function Read-FactorySettings {
    param(
        [Parameter(Mandatory = $true)]
        [string]$SettingsPath
    )

    if (-not (Test-Path $SettingsPath)) {
        return New-FactorySettingsObject
    }

    $raw = Get-Content -Path $SettingsPath -Raw
    if ([string]::IsNullOrWhiteSpace($raw)) {
        return New-FactorySettingsObject
    }

    return $raw | ConvertFrom-Json
}

function Backup-FactorySettings {
    param(
        [Parameter(Mandatory = $true)]
        [string]$SettingsPath
    )

    if (-not (Test-Path $SettingsPath)) {
        return $null
    }

    $timestamp = Get-Date -Format "yyyyMMddHHmmss"
    $backupPath = "$SettingsPath.$timestamp.bak"
    Copy-Item -LiteralPath $SettingsPath -Destination $backupPath -Force
    return $backupPath
}

function Write-FactorySettings {
    param(
        [Parameter(Mandatory = $true)]
        [string]$SettingsPath,
        [Parameter(Mandatory = $true)]
        [psobject]$Settings
    )

    $parent = Split-Path -Parent $SettingsPath
    if (-not (Test-Path $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }

    $json = $Settings | ConvertTo-Json -Depth 20
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($SettingsPath, $json + [Environment]::NewLine, $utf8NoBom)
}

function Assert-FactoryHookEventsSerializeAsArrays {
    param(
        [Parameter(Mandatory = $true)]
        [string]$SettingsPath
    )

    $rawSettings = Get-Content -Path $SettingsPath -Raw
    $parsed = $rawSettings | ConvertFrom-Json

    foreach ($eventName in Get-FactoryHookEventNames) {
        if ($null -eq $parsed.hooks) {
            throw "Missing hooks object in $SettingsPath"
        }

        $eventValue = $parsed.hooks.PSObject.Properties[$eventName].Value
        if ($null -eq $eventValue) {
            throw "Missing serialized hook event $eventName in $SettingsPath"
        }

        if (-not ($eventValue -is [System.Array])) {
            throw "Expected serialized hook event $eventName in $SettingsPath to be a JSON array but found $($eventValue.GetType().FullName)"
        }
    }
}

function Assert-FactoryHooksInstalled {
    param(
        [Parameter(Mandatory = $true)]
        [string]$SettingsPath,
        [Parameter(Mandatory = $true)]
        [string]$Command
    )

    $settings = Read-FactorySettings -SettingsPath $SettingsPath
    foreach ($eventName in Get-FactoryHookEventNames) {
        $entries = @($settings.hooks.$eventName)
        $matchCount = Get-FactoryHookCommandMatchCount -EventEntries $entries -Command $Command
        if ($matchCount -ne 1) {
            throw "Expected exactly one Factory hook command for $eventName in $SettingsPath but found $matchCount"
        }
    }
}

function Install-FactoryDroidHooks {
    param(
        [string]$RepoRoot = (Split-Path -Parent $PSScriptRoot),
        [string]$HomeDir = $env:USERPROFILE
    )

    if ([string]::IsNullOrWhiteSpace($HomeDir)) {
        throw "Home directory is not available."
    }

    $sourceHookScript = Join-Path $RepoRoot "scripts\factory-droid-status-hook.ps1"
    if (-not (Test-Path $sourceHookScript)) {
        throw "Factory Droid hook source script not found: $sourceHookScript"
    }

    $installedHookPath = Get-InstalledFactoryDroidHookPath -HomeDir $HomeDir
    $hooksRoot = Split-Path -Parent $installedHookPath
    New-Item -ItemType Directory -Path $hooksRoot -Force | Out-Null
    Copy-Item -LiteralPath $sourceHookScript -Destination $installedHookPath -Force

    $settingsPath = Get-FactorySettingsPath -HomeDir $HomeDir
    $settings = Read-FactorySettings -SettingsPath $settingsPath
    $command = Get-FactoryDroidHookCommand -ScriptPath $installedHookPath
    $mergedSettings = Merge-FactoryHookSettings -Settings $settings -Command $command
    $backupPath = Backup-FactorySettings -SettingsPath $settingsPath
    Write-FactorySettings -SettingsPath $settingsPath -Settings $mergedSettings
    Assert-FactoryHookEventsSerializeAsArrays -SettingsPath $settingsPath
    Assert-FactoryHooksInstalled -SettingsPath $settingsPath -Command $command

    $legacyUnsupportedPaths = @(
        Join-Path $HomeDir ".claude\hooks\on-working.ps1"
        Join-Path $HomeDir ".claude\hooks\on-stop.ps1"
    ) | Where-Object { Test-Path $_ }

    return [pscustomobject]@{
        InstalledHookPath      = $installedHookPath
        SettingsPath           = $settingsPath
        BackupPath             = $backupPath
        Command                = $command
        LegacyUnsupportedPaths = $legacyUnsupportedPaths
    }
}

if ($MyInvocation.InvocationName -ne ".") {
    $result = Install-FactoryDroidHooks
    Write-Host "Installed Factory Droid hooks in $($result.SettingsPath)"
    if ($null -ne $result.BackupPath) {
        Write-Host "Backed up previous settings to $($result.BackupPath)"
    }
    foreach ($legacyPath in @($result.LegacyUnsupportedPaths)) {
        Write-Host "Left unsupported legacy hook untouched: $legacyPath"
    }
}
