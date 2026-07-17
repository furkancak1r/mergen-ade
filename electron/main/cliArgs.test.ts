import { describe, expect, it } from 'vitest';
import { getMergenCliArgs } from './cliArgs';

describe('Mergen CLI argument detection', () => {
  it('finds helper mode in a packaged Electron argv', () => {
    expect(getMergenCliArgs([
      'C:/Temp/Mergen ADE.exe',
      '--browser-mcp-helper',
      '--caps=devtools',
    ])).toEqual(['--browser-mcp-helper', '--caps=devtools']);
  });

  it('finds helper mode after the app path in a development Electron argv', () => {
    expect(getMergenCliArgs([
      'C:/electron.exe',
      'C:/repo/electron',
      '--browser-mcp-helper',
      '--caps=vision',
    ])).toEqual(['--browser-mcp-helper', '--caps=vision']);
  });
});
