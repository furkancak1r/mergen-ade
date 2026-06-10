import type { LauncherEntry } from '../../../shared/types';
import { ANTHROPIC_ENV_VARS_TO_REMOVE, BuiltinLauncherKind, ShellKind } from '../../../shared/types';

const PERMISSION_MODE_FLAG = '--permission-mode';
const DANGEROUS_SKIP_PERMISSIONS_FLAG = '--dangerously-skip-permissions';

export const launcherBypassPermissionsEffective = (launcher: LauncherEntry): boolean => {
  if (launcher.builtin === BuiltinLauncherKind.Claude) return true;
  return launcher.bypassPermissions ?? false;
};

export const effectiveLauncherCommand = (launcher: LauncherEntry, shell: ShellKind): string => {
  if (launcher.builtin !== BuiltinLauncherKind.Claude || !launcherBypassPermissionsEffective(launcher)) {
    return launcher.launchCommand;
  }
  return sanitizedClaudeLaunchCommand(shell, launcher.launchCommand);
};

export const sanitizedClaudeLaunchCommand = (shell: ShellKind, configuredCommand: string): string => {
  const command = claudeCommandWithBypassPermissions(configuredCommand);
  switch (shell) {
    case ShellKind.PowerShell:
      return `Remove-Item ${ANTHROPIC_ENV_VARS_TO_REMOVE.map((key) => `Env:${key}`).join(',')} -ErrorAction SilentlyContinue; ${powershellInvocationCommand(command)}`;
    case ShellKind.Cmd:
      return `${ANTHROPIC_ENV_VARS_TO_REMOVE.map((key) => `set ${key}=`).join(' & ')} & ${command}`;
    case ShellKind.Zsh:
      return command;
  }
};

export const claudeCommandWithBypassPermissions = (configuredCommand: string): string => {
  const trimmed = configuredCommand.trim();
  const command = trimmed.length === 0 ? 'claude' : trimmed;

  if (findShellFlag(command, DANGEROUS_SKIP_PERMISSIONS_FLAG) !== undefined) {
    return command;
  }

  const permissionModeIndex = findShellFlag(command, PERMISSION_MODE_FLAG);
  if (permissionModeIndex !== undefined) {
    return replaceOrInsertPermissionModeValue(command, permissionModeIndex);
  }

  return `${command} ${PERMISSION_MODE_FLAG} bypassPermissions`;
};

const replaceOrInsertPermissionModeValue = (command: string, flagIndex: number): string => {
  const flagEnd = flagIndex + PERMISSION_MODE_FLAG.length;
  const afterFlag = command.slice(flagEnd);

  if (afterFlag.startsWith('=')) {
    const valueStart = flagEnd + 1;
    const valueTail = command.slice(valueStart);
    const whitespaceIndex = valueTail.search(/\s/);
    const valueEnd = valueStart + (whitespaceIndex === -1 ? valueTail.length : whitespaceIndex);
    return `${command.slice(0, valueStart)}bypassPermissions${command.slice(valueEnd)}`;
  }

  const nonWhitespaceIndex = afterFlag.search(/\S/);
  if (nonWhitespaceIndex === -1 || nonWhitespaceIndex === 0 || flagEnd + nonWhitespaceIndex >= command.length) {
    return `${command} bypassPermissions`;
  }

  const valueStart = flagEnd + nonWhitespaceIndex;
  const valueTail = command.slice(valueStart);
  const whitespaceIndex = valueTail.search(/\s/);
  const valueEnd = valueStart + (whitespaceIndex === -1 ? valueTail.length : whitespaceIndex);
  return `${command.slice(0, valueStart)}bypassPermissions${command.slice(valueEnd)}`;
};

const powershellInvocationCommand = (command: string): string => {
  const trimmed = command.trim();
  if (trimmed.startsWith('&') || trimmed.startsWith('.')) return trimmed;
  if (trimmed.startsWith('"') || trimmed.startsWith("'")) return `& ${trimmed}`;
  return trimmed;
};

const findShellFlag = (command: string, flag: string): number | undefined => {
  const lowerCommand = command.toLowerCase();
  const lowerFlag = flag.toLowerCase();
  let searchStart = 0;

  while (searchStart < lowerCommand.length) {
    const index = lowerCommand.indexOf(lowerFlag, searchStart);
    if (index === -1) return undefined;

    const before = index === 0 ? undefined : command[index - 1];
    const after = command[index + flag.length];
    const beforeOk = before === undefined || /\s/.test(before);
    const afterOk = after === undefined || /\s|=/.test(after);
    if (beforeOk && afterOk) return index;

    searchStart = index + lowerFlag.length;
  }

  return undefined;
};
