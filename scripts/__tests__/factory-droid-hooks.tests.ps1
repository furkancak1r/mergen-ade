$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. (Join-Path $repoRoot "scripts\factory-droid-status-hook.ps1")
. (Join-Path $repoRoot "scripts\install-factory-droid-hooks.ps1")

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
        [AllowNull()]
        $Actual,
        [Parameter(Mandatory = $true)]
        [AllowNull()]
        $Expected,
        [Parameter(Mandatory = $true)]
        [string]$Message
    )

    if ($Actual -ne $Expected) {
        throw "$Message`nExpected: $Expected`nActual:   $Actual"
    }
}

function Assert-EquivalentResolvedPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ActualPath,
        [Parameter(Mandatory = $true)]
        [string]$ExpectedPath,
        [Parameter(Mandatory = $true)]
        [string]$Message
    )

    $resolvedActual = (Get-Item -LiteralPath $ActualPath).FullName
    $resolvedExpected = (Get-Item -LiteralPath $ExpectedPath).FullName
    if ($resolvedActual -ne $resolvedExpected) {
        throw "$Message`nExpected: $resolvedExpected`nActual:   $resolvedActual"
    }
}

function Assert-ThrowsLike {
    param(
        [Parameter(Mandatory = $true)]
        [scriptblock]$Action,
        [Parameter(Mandatory = $true)]
        [string]$Pattern,
        [Parameter(Mandatory = $true)]
        [string]$Message
    )

    try {
        & $Action
    }
    catch {
        if ($_.Exception.Message -like $Pattern) {
            return
        }

        throw "$Message`nExpected message like: $Pattern`nActual: $($_.Exception.Message)"
    }

    throw "$Message`nExpected action to throw."
}

function Assert-FactoryHookEventsAreArrays {
    param(
        [Parameter(Mandatory = $true)]
        [psobject]$Settings,
        [Parameter(Mandatory = $true)]
        [string]$MessagePrefix
    )

    foreach ($eventName in Get-FactoryHookEventNames) {
        $eventValue = $Settings.hooks.PSObject.Properties[$eventName].Value
        Assert-True -Condition ($eventValue -is [System.Array]) -Message "$MessagePrefix Expected $eventName to remain an array-shaped hook event."
    }
}

function New-TestHookInboxDir {
    $path = Join-Path $env:TEMP ("mergen-factory-hook-inbox-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $path -Force | Out-Null
    return $path
}

function Read-TestHookInboxRecord {
    param(
        [Parameter(Mandatory = $true)]
        [string]$InboxDir,
        [Parameter(Mandatory = $true)]
        [string]$TerminalId
    )

    $path = Join-Path $InboxDir "$TerminalId.jsonl"
    Assert-True -Condition (Test-Path $path) -Message "Expected hook inbox file to exist at $path."
    $lines = @(Get-Content -Path $path)
    Assert-Equal -Actual $lines.Count -Expected 1 -Message "Expected a single JSONL hook record in $path."
    return $lines[0] | ConvertFrom-Json
}

function Get-FactoryDroidDecodedLauncherScriptBlock {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Command
    )

    $match = [regex]::Match($Command, '(?i)-EncodedCommand\s+(\S+)')
    if (-not $match.Success) {
        return $null
    }

    $bytes = [Convert]::FromBase64String($match.Groups[1].Value)
    return [System.Text.Encoding]::Unicode.GetString($bytes)
}

function Test-GetFactoryDroidHookCommandReturnsCanonicalWindowsCommand {
    $scriptPath = "C:\Users\furkan.cakir\.factory\hooks\mergen-ade-droid-status.ps1"
    $expectedPrefix = ('{0} -NoLogo -NonInteractive -NoProfile -ExecutionPolicy Bypass -EncodedCommand ' -f (Get-FactoryDroidPowerShellCommandToken))
    $actual = Get-FactoryDroidHookCommand -ScriptPath $scriptPath
    $decoded = Get-FactoryDroidDecodedLauncherScriptBlock -Command $actual

    Assert-True -Condition $actual.StartsWith($expectedPrefix) -Message "Expected Get-FactoryDroidHookCommand to return the canonical encoded launcher command."
    Assert-True -Condition ($actual.Contains(' -File ') -eq $false) -Message "Expected Get-FactoryDroidHookCommand to avoid the brittle -File launcher shape."
    Assert-True -Condition (-not $actual.Contains('"')) -Message "Expected Get-FactoryDroidHookCommand to avoid embedded quotes on this machine."
    Assert-Equal -Actual $decoded -Expected ("& '$scriptPath'") -Message "Expected the encoded launcher to invoke the installed hook script path."
}

function Test-GetFactoryDroidHookCommandSupportsWhitespaceInScriptPath {
    $scriptPath = "C:\Users\Name With Space\.factory\hooks\mergen-ade-droid-status.ps1"
    $actual = Get-FactoryDroidHookCommand -ScriptPath $scriptPath
    $decoded = Get-FactoryDroidDecodedLauncherScriptBlock -Command $actual

    Assert-True -Condition $actual.Contains(' -EncodedCommand ') -Message "Expected whitespace-containing script paths to use the encoded launcher."
    Assert-Equal -Actual $decoded -Expected ("& '$scriptPath'") -Message "Expected the encoded launcher to preserve the full whitespace-containing script path."
}

function Test-GetFactoryDroidPowerShellCommandTokenResolvesUsableShell {
    $token = Get-FactoryDroidPowerShellCommandToken

    Assert-True -Condition ([string]::IsNullOrWhiteSpace($token) -eq $false) -Message "Expected Get-FactoryDroidPowerShellCommandToken to return a non-empty shell token."
    if ($token.Contains('\') -or $token.Contains('/')) {
        Assert-True -Condition (Test-Path $token) -Message "Expected an absolute PowerShell launcher token to point at an existing executable."
        Assert-True -Condition (-not ($token -match '\s')) -Message "Expected an absolute PowerShell launcher token to avoid whitespace so the executable token stays shell-safe."
    }
    else {
        Assert-True -Condition ($token -in @("powershell.exe", "pwsh.exe")) -Message "Expected a non-path PowerShell launcher token to fall back to powershell.exe or pwsh.exe."
    }
}

function Test-GetFactoryDroidHookCommandSupportsPercentInScriptPath {
    $scriptPath = "C:\Users\name%temp%\.factory\hooks\mergen-ade-droid-status.ps1"
    $actual = Get-FactoryDroidHookCommand -ScriptPath $scriptPath
    $decoded = Get-FactoryDroidDecodedLauncherScriptBlock -Command $actual

    Assert-True -Condition $actual.Contains(' -EncodedCommand ') -Message "Expected percent-containing script paths to use the encoded launcher."
    Assert-Equal -Actual $decoded -Expected ("& '$scriptPath'") -Message "Expected the encoded launcher to preserve the percent-containing script path."
}

function Test-NewFactoryDroidHookRecordMapsPromptSubmitToRunning {
    $record = New-FactoryDroidHookRecord -HookInput ([pscustomobject]@{
            hook_event_name = "UserPromptSubmit"
            session_id      = "session-1"
            message         = "Hello"
        }) -InboxContext ([pscustomobject]@{
            TerminalId = "41"
            HooksDir   = "C:\temp"
            InboxPath  = "C:\temp\41.jsonl"
            InboxToken = "token-1"
        })

    Assert-Equal -Actual $record.terminal_id -Expected "41" -Message "Expected running records to preserve the terminal id."
    Assert-Equal -Actual $record.inbox_token -Expected "token-1" -Message "Expected running records to preserve the inbox token."
    Assert-Equal -Actual $record.hook_event_name -Expected "UserPromptSubmit" -Message "Expected running records to preserve the hook event name."
    Assert-Equal -Actual $record.status -Expected "running" -Message "Expected UserPromptSubmit to map to running."
    Assert-Equal -Actual $record.notification_kind -Expected $null -Message "Expected UserPromptSubmit to omit notification kind."
}

function Test-NewFactoryDroidHookRecordMapsOnlyDocumentedWaitingNotifications {
    $idleRecord = New-FactoryDroidHookRecord -HookInput ([pscustomobject]@{
            hook_event_name = "Notification"
            message         = "Droid is waiting for your input"
        }) -InboxContext ([pscustomobject]@{
            TerminalId = "41"
            HooksDir   = "C:\temp"
            InboxPath  = "C:\temp\41.jsonl"
            InboxToken = "token-2"
        })
    $permissionRecord = New-FactoryDroidHookRecord -HookInput ([pscustomobject]@{
            hook_event_name = "Notification"
            message         = "Droid needs your permission to use Execute"
        }) -InboxContext ([pscustomobject]@{
            TerminalId = "41"
            HooksDir   = "C:\temp"
            InboxPath  = "C:\temp\41.jsonl"
            InboxToken = "token-2"
        })
    $ignoredRecord = New-FactoryDroidHookRecord -HookInput ([pscustomobject]@{
            hook_event_name = "Notification"
            message         = "Droid completed your task"
        }) -InboxContext ([pscustomobject]@{
            TerminalId = "41"
            HooksDir   = "C:\temp"
            InboxPath  = "C:\temp\41.jsonl"
            InboxToken = "token-2"
        })

    Assert-Equal -Actual $idleRecord.inbox_token -Expected "token-2" -Message "Expected waiting notifications to preserve the inbox token."
    Assert-Equal -Actual $idleRecord.notification_kind -Expected "idle_prompt" -Message "Expected waiting-for-input notifications to map to idle_prompt."
    Assert-Equal -Actual $permissionRecord.notification_kind -Expected "permission_prompt" -Message "Expected permission notifications to map to permission_prompt."
    Assert-Equal -Actual $idleRecord.status -Expected "attention" -Message "Expected actionable notifications to map to attention."
    Assert-Equal -Actual $ignoredRecord -Expected $null -Message "Expected non-waiting notifications to be ignored."
}

function Test-InvokeFactoryDroidHookSignalWritesJsonlRecordAndStaysQuiet {
    $inboxDir = New-TestHookInboxDir
    $terminalId = "52"
    $inboxToken = "token-52"
    $previousTerminalId = $env:MERGEN_ADE_TERMINAL_ID
    $previousHooksDir = $env:MERGEN_ADE_FACTORY_DROID_HOOKS_DIR
    $previousInboxToken = $env:MERGEN_ADE_FACTORY_DROID_INBOX_TOKEN

    try {
        $env:MERGEN_ADE_TERMINAL_ID = $terminalId
        $env:MERGEN_ADE_FACTORY_DROID_HOOKS_DIR = $inboxDir
        $env:MERGEN_ADE_FACTORY_DROID_INBOX_TOKEN = $inboxToken
        $output = Invoke-FactoryDroidHookSignal -HookInput ([pscustomobject]@{
                hook_event_name = "Stop"
                session_id      = "session-stop"
                message         = "Droid is waiting for your input"
            }) | Out-String
        $record = Read-TestHookInboxRecord -InboxDir $inboxDir -TerminalId $terminalId

        Assert-True -Condition ([string]::IsNullOrWhiteSpace($output)) -Message "Expected Invoke-FactoryDroidHookSignal to stay silent on stdout."
        Assert-Equal -Actual $record.inbox_token -Expected $inboxToken -Message "Expected Stop hook records to preserve the inbox token."
        Assert-Equal -Actual $record.status -Expected "attention" -Message "Expected Stop hook records to map to attention."
        Assert-Equal -Actual $record.hook_event_name -Expected "Stop" -Message "Expected Stop hook records to preserve the hook event name."
        Assert-Equal -Actual $record.session_id -Expected "session-stop" -Message "Expected Stop hook records to preserve the session id."
    }
    finally {
        $env:MERGEN_ADE_TERMINAL_ID = $previousTerminalId
        $env:MERGEN_ADE_FACTORY_DROID_HOOKS_DIR = $previousHooksDir
        $env:MERGEN_ADE_FACTORY_DROID_INBOX_TOKEN = $previousInboxToken
        Remove-Item $inboxDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Test-InvokeFactoryDroidHookSignalIsNoOpWithoutEnvContext {
    $previousTerminalId = $env:MERGEN_ADE_TERMINAL_ID
    $previousHooksDir = $env:MERGEN_ADE_FACTORY_DROID_HOOKS_DIR
    $previousInboxToken = $env:MERGEN_ADE_FACTORY_DROID_INBOX_TOKEN

    try {
        Remove-Item Env:\MERGEN_ADE_TERMINAL_ID -ErrorAction SilentlyContinue
        Remove-Item Env:\MERGEN_ADE_FACTORY_DROID_HOOKS_DIR -ErrorAction SilentlyContinue
        Remove-Item Env:\MERGEN_ADE_FACTORY_DROID_INBOX_TOKEN -ErrorAction SilentlyContinue
        $output = Invoke-FactoryDroidHookSignal -HookInput ([pscustomobject]@{
                hook_event_name = "UserPromptSubmit"
                session_id      = "session-running"
            }) | Out-String

        Assert-True -Condition ([string]::IsNullOrWhiteSpace($output)) -Message "Expected missing hook inbox env vars to produce no stdout."
    }
    finally {
        $env:MERGEN_ADE_TERMINAL_ID = $previousTerminalId
        $env:MERGEN_ADE_FACTORY_DROID_HOOKS_DIR = $previousHooksDir
        $env:MERGEN_ADE_FACTORY_DROID_INBOX_TOKEN = $previousInboxToken
    }
}

function Test-MergeFactoryHookSettingsMigratesBrokenManagedCommands {
    $scriptPath = "C:\Users\furkan.cakir\.factory\hooks\mergen-ade-droid-status.ps1"
    $command = Get-FactoryDroidHookCommand -ScriptPath $scriptPath
    $brokenCommand = ('"{0}" -NoProfile -ExecutionPolicy Bypass -File "{1}"' -f (Join-Path $PSHOME "powershell.exe"), $scriptPath)
    $unrelatedCommand = "C:\tools\other-hook.cmd"
    $initial = [pscustomobject]@{
        profile = "balanced"
        hooks   = [pscustomobject]@{
            UserPromptSubmit = @(
                [pscustomobject]@{
                    hooks = @(
                        [pscustomobject]@{
                            type    = "command"
                            command = $brokenCommand
                            timeout = 9
                        }
                    )
                }
                [pscustomobject]@{
                    hooks = @(
                        [pscustomobject]@{
                            type    = "command"
                            command = $command
                            timeout = 5
                        }
                    )
                }
            )
            Notification = @(
                [pscustomobject]@{
                    hooks = @(
                        [pscustomobject]@{
                            type    = "command"
                            command = $brokenCommand
                            timeout = 9
                        }
                        [pscustomobject]@{
                            type    = "command"
                            command = $unrelatedCommand
                            timeout = 3
                        }
                    )
                }
            )
        }
    }

    $merged = Merge-FactoryHookSettings -Settings $initial -Command $command
    Assert-FactoryHookEventsAreArrays -Settings $merged -MessagePrefix "Expected Merge-FactoryHookSettings to preserve array shape."

    foreach ($eventName in Get-FactoryHookEventNames) {
        $entries = @($merged.hooks.$eventName)
        $matches = Get-FactoryHookCommandMatchCount -EventEntries $entries -Command $command
        Assert-Equal -Actual $matches -Expected 1 -Message "Expected Merge-FactoryHookSettings to keep exactly one canonical managed hook command for $eventName."
    }

    Assert-True -Condition (Test-FactoryHookCommandPresent -EventEntries @($merged.hooks.Notification) -Command $unrelatedCommand) -Message "Expected Merge-FactoryHookSettings to preserve unrelated hook commands."
    Assert-Equal -Actual $merged.profile -Expected "balanced" -Message "Expected Merge-FactoryHookSettings to preserve unrelated top-level settings while migrating broken managed hook commands."
}

function Test-MergeFactoryHookSettingsMigratesBrokenObjectShapedEvents {
    $scriptPath = "C:\Users\furkan.cakir\.factory\hooks\mergen-ade-droid-status.ps1"
    $command = Get-FactoryDroidHookCommand -ScriptPath $scriptPath
    $brokenCommand = ('"{0}" -NoProfile -ExecutionPolicy Bypass -File "{1}"' -f (Join-Path $PSHOME "powershell.exe"), $scriptPath)
    $initial = [pscustomobject]@{
        profile = "balanced"
        hooks   = [pscustomobject]@{
            UserPromptSubmit = [pscustomobject]@{
                hooks = @(
                    [pscustomobject]@{
                        type    = "command"
                        command = $brokenCommand
                        timeout = 5
                    }
                )
            }
            Notification = [pscustomobject]@{
                hooks = @(
                    [pscustomobject]@{
                        type    = "command"
                        command = $brokenCommand
                        timeout = 5
                    }
                )
            }
        }
    }

    $merged = Merge-FactoryHookSettings -Settings $initial -Command $command
    Assert-FactoryHookEventsAreArrays -Settings $merged -MessagePrefix "Expected broken object-shaped managed events to be migrated."

    foreach ($eventName in Get-FactoryHookEventNames) {
        $entries = @($merged.hooks.$eventName)
        $matches = Get-FactoryHookCommandMatchCount -EventEntries $entries -Command $command
        Assert-Equal -Actual $matches -Expected 1 -Message "Expected object-shaped managed hooks to migrate to exactly one canonical command for $eventName."
    }
}

function Test-MergeFactoryHookSettingsIsIdempotent {
    $command = Get-FactoryDroidHookCommand -ScriptPath "C:\Users\furkan.cakir\.factory\hooks\mergen-ade-droid-status.ps1"
    $initial = [pscustomobject]@{
        profile = "balanced"
        hooks   = [pscustomobject]@{
            Notification = @(
                [pscustomobject]@{
                    hooks = @(
                        [pscustomobject]@{
                            type    = "command"
                            command = $command
                            timeout = 5
                        }
                    )
                }
            )
        }
    }

    $once = Merge-FactoryHookSettings -Settings $initial -Command $command
    $twice = Merge-FactoryHookSettings -Settings $once -Command $command
    Assert-FactoryHookEventsAreArrays -Settings $twice -MessagePrefix "Expected Merge-FactoryHookSettings idempotent result to preserve array shape."

    foreach ($eventName in Get-FactoryHookEventNames) {
        $entries = @($twice.hooks.$eventName)
        $matches = 0
        foreach ($entry in $entries) {
            foreach ($hook in @($entry.hooks)) {
                if ([string]$hook.command -eq $command) {
                    $matches++
                }
            }
        }

        Assert-Equal -Actual $matches -Expected 1 -Message "Expected Merge-FactoryHookSettings to keep exactly one hook command for $eventName."
    }

    Assert-Equal -Actual $twice.profile -Expected "balanced" -Message "Expected Merge-FactoryHookSettings to preserve unrelated settings."
}

function Test-InstallFactoryDroidHooksWritesSettingsBackupAndWorkingCommand {
    $tempRoot = Join-Path $env:TEMP ("mergen-factory-hooks-test-" + [guid]::NewGuid().ToString("N"))
    $homeDir = Join-Path $tempRoot "home dir"
    $settingsPath = Get-FactorySettingsPath -HomeDir $homeDir
    $settingsDir = Split-Path -Parent $settingsPath
    $hookInboxDir = Join-Path $tempRoot "hook-inbox"
    $previousTerminalId = $env:MERGEN_ADE_TERMINAL_ID
    $previousHooksDir = $env:MERGEN_ADE_FACTORY_DROID_HOOKS_DIR
    $previousInboxToken = $env:MERGEN_ADE_FACTORY_DROID_INBOX_TOKEN

    New-Item -ItemType Directory -Path $settingsDir -Force | Out-Null
    New-Item -ItemType Directory -Path $hookInboxDir -Force | Out-Null
    $brokenCommand = ('"{0}" -NoProfile -ExecutionPolicy Bypass -File "{1}"' -f (Join-Path $PSHOME "powershell.exe"), (Join-Path $homeDir ".factory\hooks\mergen-ade-droid-status.ps1"))
    $seedSettings = [pscustomobject]@{
        personality = "pragmatic"
        hooks       = [pscustomobject]@{
            Stop = @(
                [pscustomobject]@{
                    hooks = @(
                        [pscustomobject]@{
                            type    = "command"
                            command = $brokenCommand
                            timeout = 5
                        }
                    )
                }
            )
        }
    }
    Set-Content -Path $settingsPath -Encoding UTF8 -Value ($seedSettings | ConvertTo-Json -Depth 10)

    try {
        $result = Install-FactoryDroidHooks -RepoRoot $repoRoot -HomeDir $homeDir
        $installedSettings = Read-FactorySettings -SettingsPath $settingsPath
        $rawInstalledSettings = Get-Content -Raw $settingsPath | ConvertFrom-Json
        $env:MERGEN_ADE_TERMINAL_ID = "77"
        $env:MERGEN_ADE_FACTORY_DROID_HOOKS_DIR = $hookInboxDir
        $env:MERGEN_ADE_FACTORY_DROID_INBOX_TOKEN = "token-77"
        $commandOutput = '{"hook_event_name":"Stop","message":"Droid is waiting for your input"}' | cmd /c $result.Command 2>&1 | Out-String
        $record = Read-TestHookInboxRecord -InboxDir $hookInboxDir -TerminalId "77"
        $expectedHookScript = Get-Content -Path (Join-Path $repoRoot "scripts\factory-droid-status-hook.ps1") -Raw
        $installedHookScript = Get-Content -Path $result.InstalledHookPath -Raw

        Assert-True -Condition (Test-Path $result.InstalledHookPath) -Message "Expected Install-FactoryDroidHooks to copy the hook script into the user Factory hooks directory."
        Assert-Equal -Actual $installedHookScript -Expected $expectedHookScript -Message "Expected Install-FactoryDroidHooks to refresh the installed hook script copy from the repo source."
        Assert-True -Condition (Test-Path $result.BackupPath) -Message "Expected Install-FactoryDroidHooks to back up an existing settings.json before rewriting it."
        Assert-Equal -Actual $installedSettings.personality -Expected "pragmatic" -Message "Expected Install-FactoryDroidHooks to preserve unrelated top-level settings."
        Assert-FactoryHookEventsAreArrays -Settings $rawInstalledSettings -MessagePrefix "Expected Install-FactoryDroidHooks to write array-shaped hook events to settings.json."
        Assert-True -Condition (-not $result.Command.StartsWith('"')) -Message "Expected Install-FactoryDroidHooks to persist an unquoted executable token for the managed hook command."
        Assert-True -Condition $result.Command.Contains(' -NoLogo -NonInteractive -NoProfile ') -Message "Expected Install-FactoryDroidHooks to persist the non-interactive no-logo flags."
        Assert-True -Condition (-not $result.Command.Contains('"')) -Message "Expected Install-FactoryDroidHooks to persist the managed hook command without embedded quotes."
        Assert-True -Condition $result.Command.Contains(' -EncodedCommand ') -Message "Expected Install-FactoryDroidHooks to persist the encoded launcher command."
        $commandScriptBlock = Get-FactoryDroidDecodedLauncherScriptBlock -Command $result.Command
        $commandScriptPath = [string]($commandScriptBlock -replace "^& '", '' -replace "'$", '')
        Assert-EquivalentResolvedPath -ActualPath $commandScriptPath -ExpectedPath $result.InstalledHookPath -Message "Expected Install-FactoryDroidHooks to persist the managed hook script path inside the encoded launcher."
        Assert-Equal -Actual $LASTEXITCODE -Expected 0 -Message "Expected the installed command to execute successfully through cmd /c."
        Assert-True -Condition ([string]::IsNullOrWhiteSpace($commandOutput)) -Message "Expected the installed hook command to stay silent on stdout when the inbox env vars are present."
        Assert-Equal -Actual $record.inbox_token -Expected "token-77" -Message "Expected the installed command to append the inbox token."
        Assert-Equal -Actual $record.status -Expected "attention" -Message "Expected the installed command to append an attention record for Stop."

        foreach ($eventName in Get-FactoryHookEventNames) {
            $entries = @($installedSettings.hooks.$eventName)
            $matches = Get-FactoryHookCommandMatchCount -EventEntries $entries -Command $result.Command
            Assert-Equal -Actual $matches -Expected 1 -Message "Expected Install-FactoryDroidHooks to register exactly one canonical command for $eventName."
        }
    }
    finally {
        $env:MERGEN_ADE_TERMINAL_ID = $previousTerminalId
        $env:MERGEN_ADE_FACTORY_DROID_HOOKS_DIR = $previousHooksDir
        $env:MERGEN_ADE_FACTORY_DROID_INBOX_TOKEN = $previousInboxToken
        Remove-Item $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Test-GetFactoryDroidHookCommandReturnsCanonicalWindowsCommand
Test-GetFactoryDroidHookCommandSupportsWhitespaceInScriptPath
Test-GetFactoryDroidPowerShellCommandTokenResolvesUsableShell
Test-GetFactoryDroidHookCommandSupportsPercentInScriptPath
Test-NewFactoryDroidHookRecordMapsPromptSubmitToRunning
Test-NewFactoryDroidHookRecordMapsOnlyDocumentedWaitingNotifications
Test-InvokeFactoryDroidHookSignalWritesJsonlRecordAndStaysQuiet
Test-InvokeFactoryDroidHookSignalIsNoOpWithoutEnvContext
Test-MergeFactoryHookSettingsMigratesBrokenManagedCommands
Test-MergeFactoryHookSettingsMigratesBrokenObjectShapedEvents
Test-MergeFactoryHookSettingsIsIdempotent
Test-InstallFactoryDroidHooksWritesSettingsBackupAndWorkingCommand

Write-Host "factory-droid-hooks PowerShell tests passed."
