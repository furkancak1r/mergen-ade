import re

with open('src/app.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# 1. User-facing labels
replacements = [
    # Header
    ('"{} OpenCode Chat"', '"{} OpenCode ACP"'),
    # Launcher row
    ('RichText::new("OpenCode Chat")', 'RichText::new("OpenCode ACP")'),
    # Settings card
    ('"OpenCode Chat Mode Toggle",', '"OpenCode ACP Mode Toggle",'),
    # Status line messages
    ('format!("ACP chat {chat_id} connected")', 'format!("OpenCode ACP {chat_id} connected")'),
    ('format!("ACP chat {chat_id} session created")', 'format!("OpenCode ACP {chat_id} session created")'),
    ('format!("ACP chat {chat_id} stopped: {stop_reason}")', 'format!("OpenCode ACP {chat_id} stopped: {stop_reason}")'),
    ('format!("ACP chat {chat_id} error: {message}")', 'format!("OpenCode ACP {chat_id} error: {message}")'),
    ('format!("ACP chat {chat_id} disconnected")', 'format!("OpenCode ACP {chat_id} disconnected")'),
    ('format!("Switched to ACP chat for {}", project.name)', 'format!("Switched to OpenCode ACP for {}", project.name)'),
    ('format!("Started ACP chat for {}", project.name)', 'format!("Started OpenCode ACP for {}", project.name)'),
    ('format!("Failed to start ACP chat: {err}")', 'format!("Failed to start OpenCode ACP: {err}")'),
    # Comments
    ('2 // OpenCode Chat + hint row', '2 // OpenCode ACP + hint row'),
    ('launcher_count + 1 // OpenCode Chat + launchers', 'launcher_count + 1 // OpenCode ACP + launchers'),
    ('// Synthetic OpenCode Chat row (always shown, non-persisted)', '// Synthetic OpenCode ACP row (always shown, non-persisted)'),
    ('// When no launchers are enabled, show a hint row after OpenCode Chat', '// When no launchers are enabled, show a hint row after OpenCode ACP'),
    ('2 // OpenCode Chat + hint', '2 // OpenCode ACP + hint'),
    ('// Open a terminal-less OpenCode ACP chat session.', '// Open a terminal-less OpenCode ACP session.'),
    ('/// ACP chat sessions (OpenCode ACP) per chat ID.', '/// OpenCode ACP sessions per chat ID.'),
    ('/// Sender for ACP chat events (cloned per spawn).', '/// Sender for OpenCode ACP events (cloned per spawn).'),
    ('/// Receiver for ACP chat events from the agent threads.', '/// Receiver for OpenCode ACP events from the agent threads.'),
    ('/// Currently active ACP chat session ID (replaces terminal grid in main area).', '/// Currently active OpenCode ACP session ID (replaces terminal grid in main area).'),
    ('/// Next ACP chat ID counter.', '/// Next OpenCode ACP session ID counter.'),
    ('// Restore the active ACP chat for the newly selected project.', '// Restore the active OpenCode ACP session for the newly selected project.'),
    ('// ACP chat mode toggle shortcut section', '// OpenCode ACP mode toggle shortcut section'),
    ('// If an ACP chat is active, show it instead of the terminal grid.', '// If an OpenCode ACP session is active, show it instead of the terminal grid.'),
    ('// ACP chat mode toggle shortcut (default Tab when ACP chat is active)', '// OpenCode ACP mode toggle shortcut (default Tab when OpenCode ACP is active)'),
    ('// Kill all ACP chat sessions', '// Kill all OpenCode ACP sessions'),
    # The interactive constraint change
    ('.interactive(!is_running && session_ready)', '.interactive(!is_running)'),
]

for old, new in replacements:
    content = content.replace(old, new)

with open('src/app.rs', 'w', encoding='utf-8') as f:
    f.write(content)

print("Done")
