import { describe, expect, it } from 'vitest';
import { resetAcpRouteAfterSend, resolveAcpRoute } from './acpRoute';

describe('acpRoute', () => {
  it('routes greetings to build', () => {
    expect(resolveAcpRoute('hi').route).toBe('build');
  });

  it('routes simple visible UI tweaks to build', () => {
    expect(resolveAcpRoute('copy path butonunu kaldır').route).toBe('build');
  });

  it('routes planning conversation to plan', () => {
    expect(resolveAcpRoute('bu mimariyi önce konuşalım').route).toBe('plan');
    expect(resolveAcpRoute('önce planla, sadece konuşalım').route).toBe('plan');
  });

  it('routes medium coding work to codex plan when allowed', () => {
    expect(resolveAcpRoute('ACP hook routing bug var, renderer ve main tarafında testleriyle düzelt').route).toBe('codex_plan');
  });

  it('does not choose codex plan when disabled', () => {
    expect(resolveAcpRoute('ACP hook routing bug var, renderer ve main tarafında testleriyle düzelt', { allowCodexPlan: false }).route).toBe('build');
  });

  it('manual override is one-shot by resetting after send', () => {
    expect(resolveAcpRoute('hi', { selectedRoute: 'plan' })).toMatchObject({ route: 'plan', auto: false });
    expect(resetAcpRouteAfterSend('plan')).toBe('auto');
  });

  it('returns a critical question instead of running unsafe requests', () => {
    const decision = resolveAcpRoute('tümünü sil ve reset --hard yap');
    expect(decision.route).toBe('build');
    expect(decision.question).toContain('destructive');
  });
});
