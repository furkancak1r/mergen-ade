import type { DirectoryNode } from '../../../shared/types';

export type DirectoryNodeContextAction = 'openInEditor' | 'openWithDefaultApp' | 'revealInFolder' | 'copyPath';

export const DIRECTORY_NO_MATCHING_MESSAGE = 'No matching files or folders';

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

export function directoryNodeContextActionLabel(action: DirectoryNodeContextAction): string {
  switch (action) {
    case 'openInEditor':
      return '<> Open in Editor';
    case 'openWithDefaultApp':
      return 'Open with Default App';
    case 'revealInFolder':
      return '📂 Reveal in Folder';
    case 'copyPath':
      return '⧉ Copy Path';
  }
}
