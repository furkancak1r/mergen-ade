export interface WebProject {
  id: number;
  name: string;
  path: string;
  is_worktree: boolean;
  repo_root?: string;
  saved_messages: string[];
  foreground_saved_messages: string[];
  browser_last_url?: string;
  checklist: string[];
}

export interface WebTerminal {
  id: number;
  project_id: number;
  kind: string;
  shell: string;
  title: string;
  exited: boolean;
  in_main_view: boolean;
  ai_tool?: string;
  ai_status?: string;
}

export type ServerMessage =
  | { kind: 'hello'; version: string; auth_required: boolean }
  | { kind: 'state_snapshot'; projects: WebProject[]; terminals: WebTerminal[]; active_terminal_id?: number; selected_project_id?: number }
  | { kind: 'state_patch'; updates: StatePatchUpdate[] }
  | { kind: 'terminal_output'; terminal_id: number; data: number[] }
  | { kind: 'terminal_status'; terminal_id: number; title: string; exited: boolean; ai_tool?: string; ai_status?: string }
  | { kind: 'error'; message: string };

export type StatePatchUpdate =
  | { type: 'project_added'; project: WebProject }
  | { type: 'project_removed'; project_id: number }
  | { type: 'project_selected'; project_id?: number }
  | { type: 'terminal_added'; terminal: WebTerminal }
  | { type: 'terminal_removed'; terminal_id: number }
  | { type: 'terminal_updated'; terminal: WebTerminal }
  | { type: 'active_terminal_changed'; terminal_id?: number }
  | { type: 'status_line'; text: string };

export interface WebShortcut {
  id: string;
  label: string;
  key: string;
  command: string;
  enabled: boolean;
}

export interface WebLauncher {
  id: string;
  display_name: string;
  command: string;
  enabled: boolean;
}

export interface ConfigResponse {
  default_shell: string;
  launchers: WebLauncher[];
  shortcuts: WebShortcut[];
}

export interface WebDirectoryNode {
  name: string;
  path: string;
  is_dir: boolean;
  is_deferred: boolean;
  children: WebDirectoryNode[];
}

export interface WebSourceControlFile {
  path: string;
  status: string;
}

export type ClientMessage =
  | { kind: 'auth'; token: string }
  | { kind: 'spawn_terminal'; project_id: number; shell: string; terminal_kind: string }
  | { kind: 'terminal_input'; terminal_id: number; data: number[] }
  | { kind: 'terminal_paste'; terminal_id: number; text: string }
  | { kind: 'terminal_resize'; terminal_id: number; cols: number; lines: number }
  | { kind: 'close_terminal'; terminal_id: number }
  | { kind: 'select_project'; project_id: number }
  | { kind: 'add_project'; name: string; path: string }
  | { kind: 'remove_project'; project_id: number }
  | { kind: 'send_shortcut'; terminal_id: number; command: string }
  | { kind: 'smart_input_submit'; terminal_id: number; text: string; mode: string }
  | { kind: 'request_directory_index'; project_id: number }
  | { kind: 'request_source_control'; project_id: number };
