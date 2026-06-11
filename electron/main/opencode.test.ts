import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { DEFAULT_OPENCODE_BUILD_MODEL } from '../shared/types';
import { generateOpencodeTerminalConfig } from './opencode';

let tempDir: string | undefined;

describe('OpenCode config generation', () => {
  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mergen-electron-opencode-'));
  });

  afterEach(() => {
    if (tempDir) {
      fs.rmSync(tempDir, { recursive: true, force: true });
    }
    tempDir = undefined;
  });

  it('writes the Mimo default build model and provider into terminal configs', () => {
    const configPath = generateOpencodeTerminalConfig(tempDir!, { kimiStrictPermissions: false });
    const config = JSON.parse(fs.readFileSync(configPath, 'utf-8'));

    expect(config.agent.build.model).toBe(DEFAULT_OPENCODE_BUILD_MODEL);
    expect(config.mode.build.model).toBe(DEFAULT_OPENCODE_BUILD_MODEL);
    expect(config.provider.mimo.models['mimo-v2.5-pro']).toEqual({
      id: 'mimo-v2.5-pro',
      name: 'Mimo v2.5 Pro',
    });
    expect(config.provider.mimo.options).toMatchObject({
      apiKey: '{env:MIMO_API_KEY}',
      baseURL: 'https://token-plan-sgp.xiaomimimo.com/v1',
    });
    expect(config.agent.build.permission['*']).toBe('allow');
  });
});
