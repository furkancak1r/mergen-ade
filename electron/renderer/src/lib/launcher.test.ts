import { describe, expect, it } from 'vitest';
import { BuiltinLauncherKind, defaultLaunchers, LauncherIconKey, ShellKind } from '../../../shared/types';
import {
  claudeCommandWithBypassPermissions,
  effectiveLauncherCommand,
  launcherBypassPermissionsEffective,
  sanitizedClaudeLaunchCommand,
} from './launcher';

describe('launcher helpers', () => {
  it('marks the Claude builtin launcher as bypass-enabled by default', () => {
    const launchers = defaultLaunchers();
    expect(launchers.find((launcher) => launcher.builtin === BuiltinLauncherKind.Claude)?.bypassPermissions).toBe(true);
    expect(launchers.find((launcher) => launcher.builtin === BuiltinLauncherKind.Codex)?.bypassPermissions).toBe(false);
  });

  it('treats Claude bypass as effective even when config stores explicit false', () => {
    const claude = defaultLaunchers().find((launcher) => launcher.builtin === BuiltinLauncherKind.Claude);
    expect(claude).toBeDefined();
    expect(launcherBypassPermissionsEffective({ ...claude!, bypassPermissions: false })).toBe(true);
  });

  it('clears Anthropic env and preserves configured aliases for PowerShell Claude launchers', () => {
    const cmd = sanitizedClaudeLaunchCommand(ShellKind.PowerShell, 'cc');
    expect(cmd).toContain('Remove-Item Env:ANTHROPIC_AUTH_TOKEN');
    expect(cmd).toMatch(/; cc --permission-mode bypassPermissions$/);
    expect(cmd).not.toContain('claude.cmd');
  });

  it('uses the PowerShell call operator for quoted Claude executable paths', () => {
    const cmd = sanitizedClaudeLaunchCommand(ShellKind.PowerShell, '"C:\\Program Files\\Claude\\claude.cmd"');
    expect(cmd).toMatch(/; & "C:\\Program Files\\Claude\\claude\.cmd" --permission-mode bypassPermissions$/);
  });

  it('clears Anthropic env and preserves configured aliases for CMD Claude launchers', () => {
    const cmd = sanitizedClaudeLaunchCommand(ShellKind.Cmd, 'cc');
    expect(cmd).toContain('set ANTHROPIC_AUTH_TOKEN=');
    expect(cmd).toMatch(/& cc --permission-mode bypassPermissions$/);
    expect(cmd).not.toContain('claude.cmd');
  });

  it('appends bypass permission mode for zsh Claude launchers', () => {
    expect(sanitizedClaudeLaunchCommand(ShellKind.Zsh, 'cc')).toBe('cc --permission-mode bypassPermissions');
  });

  it('defaults to claude when the configured Claude command is empty', () => {
    expect(sanitizedClaudeLaunchCommand(ShellKind.Zsh, '  ')).toBe('claude --permission-mode bypassPermissions');
  });

  it('does not duplicate an existing bypass permission mode', () => {
    const cmd = claudeCommandWithBypassPermissions('cc --permission-mode bypassPermissions');
    expect(cmd).toBe('cc --permission-mode bypassPermissions');
    expect(cmd.match(/--permission-mode/g)).toHaveLength(1);
  });

  it('replaces non-bypass permission mode values', () => {
    expect(claudeCommandWithBypassPermissions('cc --permission-mode ask')).toBe('cc --permission-mode bypassPermissions');
    expect(claudeCommandWithBypassPermissions('cc --permission-mode=default')).toBe('cc --permission-mode=bypassPermissions');
  });

  it('preserves dangerous skip permissions because it already requests bypass behavior', () => {
    expect(claudeCommandWithBypassPermissions('cc --dangerously-skip-permissions')).toBe('cc --dangerously-skip-permissions');
  });

  it('only rewrites built-in Claude launchers when computing the effective command', () => {
    const custom = {
      id: 'custom-claude-wrapper',
      displayName: 'Wrapper',
      launchCommand: 'cc',
      enabled: true,
      iconKey: LauncherIconKey.Rocket,
      bypassPermissions: true,
    };
    const claude = { ...custom, builtin: BuiltinLauncherKind.Claude };
    expect(effectiveLauncherCommand(custom, ShellKind.Zsh)).toBe('cc');
    expect(effectiveLauncherCommand(claude, ShellKind.Zsh)).toBe('cc --permission-mode bypassPermissions');
  });
});
