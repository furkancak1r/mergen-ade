import { describe, expect, it } from 'vitest';
import { resolvePromptRoute } from './promptRoute';

describe('promptRoute', () => {
  it('routes greetings to build', () => {
    expect(resolvePromptRoute('hi').route).toBe('build');
  });

  it('routes simple visible UI tweaks to build', () => {
    expect(resolvePromptRoute('copy path butonunu kaldır').route).toBe('build');
  });

  it('routes planning conversation to plan', () => {
    expect(resolvePromptRoute('bu mimariyi önce konuşalım').route).toBe('plan');
    expect(resolvePromptRoute('önce planla, sadece konuşalım').route).toBe('plan');
  });

  it('routes medium coding work to codex plan when allowed', () => {
    expect(resolvePromptRoute('Hook routing bug var, renderer ve main tarafında testleriyle düzelt').route).toBe('codex_plan');
  });

  it('does not choose codex plan when disabled', () => {
    expect(resolvePromptRoute('Hook routing bug var, renderer ve main tarafında testleriyle düzelt', { allowCodexPlan: false }).route).toBe('build');
  });

  it('respects a manual route override', () => {
    expect(resolvePromptRoute('hi', { selectedRoute: 'plan' })).toMatchObject({ route: 'plan', auto: false });
  });

  it('returns a critical question instead of running unsafe requests', () => {
    const decision = resolvePromptRoute('tümünü sil ve reset --hard yap');
    expect(decision.route).toBe('build');
    expect(decision.question).toContain('destructive');
  });
});
