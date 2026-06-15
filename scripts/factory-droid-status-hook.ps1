$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Read-FactoryDroidHookInput {
    $raw = [Console]::In.ReadToEnd()
    if ([string]::IsNullOrWhiteSpace($raw)) {
        return $null
    }

    try {
        return $raw | ConvertFrom-Json
    }
    catch {
        throw "Invalid Factory hook JSON: $($_.Exception.Message)"
    }
}

function Get-FactoryDroidHookInputPropertyValue {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$HookInput,
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    $property = $HookInput.PSObject.Properties[$Name]
    if ($null -eq $property) {
        return $null
    }

    return $property.Value
}

function Get-FactoryDroidHookInboxContext {
    $terminalId = [string]$env:MERGEN_ADE_TERMINAL_ID
    $hooksDir = [string]$env:MERGEN_ADE_FACTORY_DROID_HOOKS_DIR
    $inboxToken = $env:MERGEN_ADE_FACTORY_DROID_INBOX_TOKEN

    if ([string]::IsNullOrWhiteSpace($terminalId) -or [string]::IsNullOrWhiteSpace($hooksDir)) {
        return $null
    }

    $numericTerminalId = 0
    if (-not [uint64]::TryParse($terminalId, [ref]$numericTerminalId)) {
        return $null
    }

    $resolvedHooksDir = [System.IO.Path]::GetFullPath($hooksDir)
    return [pscustomobject]@{
        TerminalId = $terminalId
        HooksDir   = $resolvedHooksDir
        InboxPath  = Join-Path $resolvedHooksDir "$terminalId.jsonl"
        InboxToken = $inboxToken
    }
}

function Get-FactoryDroidNotificationKind {
    param(
        [AllowNull()]
        [string]$Message
    )

    $normalized = ([string]$Message).ToLowerInvariant()
    if ($normalized.Contains("needs your permission")) {
        return "permission_prompt"
    }

    if ($normalized.Contains("waiting for your input")) {
        return "idle_prompt"
    }

    return $null
}

function New-FactoryDroidHookRecord {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$HookInput,
        [Parameter(Mandatory = $true)]
        [pscustomobject]$InboxContext
    )

    $eventName = [string](Get-FactoryDroidHookInputPropertyValue -HookInput $HookInput -Name "hook_event_name")
    $message = [string](Get-FactoryDroidHookInputPropertyValue -HookInput $HookInput -Name "message")
    $sessionId = Get-FactoryDroidHookInputPropertyValue -HookInput $HookInput -Name "session_id"
    $status = $null
    $notificationKind = $null

    switch ($eventName) {
        "UserPromptSubmit" {
            $status = "running"
        }
        "Stop" {
            $status = "attention"
        }
        "Notification" {
            $notificationKind = Get-FactoryDroidNotificationKind -Message $message
            if ([string]::IsNullOrWhiteSpace($notificationKind)) {
                return $null
            }

            $status = "attention"
        }
        default {
            return $null
        }
    }

    return [pscustomobject]([ordered]@{
            terminal_id       = $InboxContext.TerminalId
            inbox_token       = $InboxContext.InboxToken
            session_id        = $sessionId
            hook_event_name   = $eventName
            status            = $status
            notification_kind = $notificationKind
            message           = $message
            timestamp_utc     = (Get-Date).ToUniversalTime().ToString("o")
        })
}

function Write-FactoryDroidHookRecord {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$InboxContext,
        [Parameter(Mandatory = $true)]
        [pscustomobject]$Record
    )

    if (-not (Test-Path $InboxContext.HooksDir)) {
        New-Item -ItemType Directory -Path $InboxContext.HooksDir -Force | Out-Null
    }

    $json = $Record | ConvertTo-Json -Compress -Depth 10
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    $stream = [System.IO.File]::Open(
        $InboxContext.InboxPath,
        [System.IO.FileMode]::Append,
        [System.IO.FileAccess]::Write,
        [System.IO.FileShare]::ReadWrite
    )

    try {
        $writer = New-Object System.IO.StreamWriter($stream, $utf8NoBom)
        try {
            $writer.WriteLine($json)
            $writer.Flush()
        }
        finally {
            $writer.Dispose()
        }
    }
    finally {
        $stream.Dispose()
    }
}

function Get-FactoryDroidHookAttentionKind {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$Record
    )

    if ([string]$Record.notification_kind -eq "permission_prompt") {
        return "permission"
    }

    if ([string]$Record.notification_kind -eq "idle_prompt") {
        return "user_input_requested"
    }

    if ([string]$Record.hook_event_name -eq "Stop") {
        return "turn_complete"
    }

    return $null
}

function Send-FactoryDroidHookStatus {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$HookInput,
        [Parameter(Mandatory = $true)]
        [pscustomobject]$Record
    )

    $port = 0
    if (-not [int]::TryParse([string]$env:MERGEN_HOOK_PORT, [ref]$port) -or $port -le 0) {
        return
    }

    $terminalId = 0
    if (-not [int]::TryParse([string]$Record.terminal_id, [ref]$terminalId)) {
        return
    }

    $eventName = [string]$Record.hook_event_name
    $payload = [pscustomobject]([ordered]@{
            terminalId    = $terminalId
            tool          = "droid"
            status        = [string]$Record.status
            reason        = [string]$Record.message
            attentionKind = Get-FactoryDroidHookAttentionKind -Record $Record
            rawJson       = ($HookInput | ConvertTo-Json -Compress -Depth 20)
            eventKind     = "factory-droid-hook:$eventName"
        })

    $client = $null
    $writer = $null
    try {
        $client = New-Object System.Net.Sockets.TcpClient
        $client.Connect("127.0.0.1", $port)
        $stream = $client.GetStream()
        $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
        $writer = New-Object System.IO.StreamWriter($stream, $utf8NoBom)
        $writer.WriteLine(($payload | ConvertTo-Json -Compress -Depth 20))
        $writer.Flush()
    }
    catch {
        return
    }
    finally {
        if ($null -ne $writer) {
            $writer.Dispose()
        }
        if ($null -ne $client) {
            $client.Close()
        }
    }
}

function Invoke-FactoryDroidHookSignal {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$HookInput
    )

    $inboxContext = Get-FactoryDroidHookInboxContext
    if ($null -eq $inboxContext) {
        return
    }

    $record = New-FactoryDroidHookRecord -HookInput $HookInput -InboxContext $inboxContext
    if ($null -eq $record) {
        return
    }

    Write-FactoryDroidHookRecord -InboxContext $inboxContext -Record $record
    Send-FactoryDroidHookStatus -HookInput $HookInput -Record $record
}

if ($MyInvocation.InvocationName -ne ".") {
    $hookInput = Read-FactoryDroidHookInput
    if ($null -ne $hookInput) {
        Invoke-FactoryDroidHookSignal -HookInput $hookInput
    }
}
