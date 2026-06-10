import type { ProjectRecord } from '../../../shared/types';

export const savedMessageOwnerProjectId = (projects: ProjectRecord[], project: ProjectRecord): number => {
  if (!project.isWorktree || !project.repoRoot) return project.id;
  const root = projects.find((candidate) => !candidate.isWorktree && candidate.path === project.repoRoot);
  return root?.id ?? project.id;
};

export const addSavedMessage = (messages: string[], message: string): string[] => {
  const trimmed = message.trim();
  if (!trimmed || messages.includes(trimmed)) return messages;
  return [...messages, trimmed];
};

export const updateSavedMessage = (messages: string[], index: number, message: string): string[] => {
  if (index < 0 || index >= messages.length) return messages;
  const next = [...messages];
  next[index] = message;
  return next;
};

export const removeSavedMessage = (messages: string[], index: number): string[] => {
  if (index < 0 || index >= messages.length) return messages;
  return messages.filter((_, candidateIndex) => candidateIndex !== index);
};

export const replaceProjectSavedMessages = (projects: ProjectRecord[], ownerProjectId: number, messages: string[]): ProjectRecord[] => {
  return projects.map((project) => {
    if (savedMessageOwnerProjectId(projects, project) !== ownerProjectId) return project;
    return { ...project, savedMessages: messages };
  });
};
