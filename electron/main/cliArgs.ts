const CLI_MODES = new Set([
  '--browser-mcp-helper',
  '--opencode-notify',
  '--codex-notify',
  '--codex-hook',
]);

export function getMergenCliArgs(argv: readonly string[]): string[] {
  const modeIndex = argv.findIndex((arg, index) => index > 0 && CLI_MODES.has(arg));
  return modeIndex < 0 ? [] : argv.slice(modeIndex);
}
