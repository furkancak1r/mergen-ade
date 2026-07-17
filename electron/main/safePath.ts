import path from 'path';

export function resolveChildPath(root: string, ...segments: string[]): string {
  const base = path.resolve(root); // nosemgrep: javascript.lang.security.audit.path-traversal.path-join-resolve-traversal.path-join-resolve-traversal -- containment is enforced below
  const candidate = path.resolve(base, ...segments); // nosemgrep: javascript.lang.security.audit.path-traversal.path-join-resolve-traversal.path-join-resolve-traversal -- containment is enforced below
  const relative = path.relative(base, candidate);
  if (relative === '..' || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    throw new Error('Path escapes its configured root');
  }
  return candidate;
}
