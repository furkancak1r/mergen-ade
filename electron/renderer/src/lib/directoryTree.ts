import type { DirectoryNode } from '../../../shared/types';

export type DirectoryNodeContextAction = 'openInEditor' | 'openWithDefaultApp' | 'revealInFolder' | 'copyPath';

export function collectLoadedDirectoryPaths(root: DirectoryNode): string[] {
  const paths: string[] = [];

  function visit(node: DirectoryNode) {
    if (!node.isDirectory || node.isSymlink) return;
    paths.push(node.path);
    if (!node.children) return;
    for (const child of node.children) {
      visit(child);
    }
  }

  visit(root);
  return paths;
}

export function directoryTreeHasCollapsedFolders(root: DirectoryNode, expandedPaths: ReadonlySet<string>): boolean {
  return collectLoadedDirectoryPaths(root).some((path) => !expandedPaths.has(path));
}

export function directoryNodeContextActions(node: Pick<DirectoryNode, 'isDirectory'>): DirectoryNodeContextAction[] {
  if (node.isDirectory) {
    return ['copyPath'];
  }
  return ['openInEditor', 'openWithDefaultApp', 'revealInFolder', 'copyPath'];
}
