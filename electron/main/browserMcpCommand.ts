import path from 'path';

export function getBrowserMcpStdioConfig(): { command: string; args: string[]; env: Record<string, string> } {
  const bundledPath = path.join(__dirname, 'browser-mcp-helper.js');
  const helperPath = bundledPath.replace(`${path.sep}app.asar${path.sep}`, `${path.sep}app.asar.unpacked${path.sep}`);
  return {
    command: process.execPath,
    args: [helperPath, '--caps=devtools,vision,network,storage'],
    env: { ELECTRON_RUN_AS_NODE: '1' },
  };
}
