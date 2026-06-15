export type AcpRouteMode = 'auto' | 'build' | 'plan' | 'codex_plan';
export type ResolvedAcpRouteMode = Exclude<AcpRouteMode, 'auto'>;

export interface AcpRouteDecision {
  route: ResolvedAcpRouteMode;
  auto: boolean;
  label: string;
  question?: string;
}

const labels: Record<ResolvedAcpRouteMode, string> = {
  build: 'Build',
  plan: 'Plan',
  codex_plan: 'Codex Plan',
};

export function acpRouteLabel(route: string | undefined): string | undefined {
  if (route === 'auto') return 'Auto';
  if (route === 'build') return labels.build;
  if (route === 'plan') return labels.plan;
  if (route === 'codex_plan') return labels.codex_plan;
  return route;
}

export function resetAcpRouteAfterSend(route: AcpRouteMode): AcpRouteMode {
  return route === 'auto' ? 'auto' : 'auto';
}

export function resolveAcpRoute(
  prompt: string,
  opts: { selectedRoute?: string; allowCodexPlan?: boolean; attachmentCount?: number } = {},
): AcpRouteDecision {
  const selectedRoute = normalizeAcpRouteMode(opts.selectedRoute);
  const allowCodexPlan = opts.allowCodexPlan !== false;
  if (selectedRoute !== 'auto') {
    const route = selectedRoute === 'codex_plan' && !allowCodexPlan ? 'build' : selectedRoute;
    return { route, auto: false, label: labels[route] };
  }

  const text = normalizeText(prompt);
  const critical = criticalQuestionFor(text);
  if (critical) return { route: 'build', auto: true, label: labels.build, question: critical };
  if (!text || isGreetingOrAck(text) || isTinyDirectTask(text)) {
    return { route: 'build', auto: true, label: labels.build };
  }
  if (isPlanningConversation(text)) {
    return { route: 'plan', auto: true, label: labels.plan };
  }
  if (allowCodexPlan && isMediumComplexCodingWork(text, opts.attachmentCount ?? 0)) {
    return { route: 'codex_plan', auto: true, label: labels.codex_plan };
  }
  return { route: 'build', auto: true, label: labels.build };
}

export function normalizeAcpRouteMode(route: string | undefined): AcpRouteMode {
  return route === 'build' || route === 'plan' || route === 'codex_plan' ? route : 'auto';
}

function normalizeText(prompt: string): string {
  return prompt.toLocaleLowerCase('tr-TR').replace(/\s+/g, ' ').trim();
}

function isGreetingOrAck(text: string): boolean {
  return /^(hi|hello|hey|selam|merhaba|sa|tamam|ok|okay|evet|hayır|devam|thanks|teşekkürler)[.!? ]*$/.test(text);
}

function isTinyDirectTask(text: string): boolean {
  if (text.length > 120) return false;
  if (/\b(acp|hook|routing|provider|integration|workflow|state|queue|parser|protocol|bug|debug|refactor|feature|test|vitest|electron|renderer|main|shared)\b/.test(text)) {
    return false;
  }
  return /\b(buton|button|renk|color|text|label|yazı|typo|copy path|padding|margin)\b/.test(text)
    || /\b(kaldır|sil|ekle|değiştir|düzelt|fix|remove|add|change)\b/.test(text);
}

function isPlanningConversation(text: string): boolean {
  const asksToDiscuss = /\b(konuşalım|tartışalım|nasıl bir yöntem|nasıl yapalım|approach|strategy|mimari|architecture|tasarım|design)\b/.test(text);
  const asksToPlan = /\b(önce plan|planla|plan yap|plan mode|sadece konuşalım)\b/.test(text);
  const asksToImplement = /\b(uygula|implement|kodla|değiştir|düzelt|fix|ekle|kaldır|build)\b/.test(text);
  return (asksToDiscuss || asksToPlan) && !asksToImplement;
}

function isMediumComplexCodingWork(text: string, attachmentCount: number): boolean {
  if (attachmentCount > 0 && text.length > 80) return true;
  const matches = [
    /\b(acp|hook|routing|provider|integration|workflow|state|queue|parser|protocol)\b/,
    /\b(bug|debug|hata|regression|refactor|feature|özellik|implement|uygula)\b/,
    /\b(test|vitest|build|electron|renderer|main|shared)\b/,
    /\b(çok dosya|multi[- ]?file|birden fazla|karmaşık|riskli|complex)\b/,
  ].filter((rx) => rx.test(text)).length;
  return text.length > 180 || matches >= 2;
}

function criticalQuestionFor(text: string): string | undefined {
  if (/\b(reset --hard|force push|rm -rf|format|wipe|tümünü sil|hepsini sil)\b/.test(text)) {
    return 'This looks destructive. Which exact target should be changed, and should anything be backed up first?';
  }
  if (/\b(kill|terminate|process öldür|süreci öldür|durdur|stop process)\b/.test(text)) {
    return 'This may stop a running process. Which exact process is safe to stop?';
  }
  if (/\b(api key|secret|token|şifre|password|credential)\b/.test(text)) {
    return 'This touches credentials. Which secret source should be used without exposing or committing the value?';
  }
  return undefined;
}
