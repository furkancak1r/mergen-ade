import { describe, expect, it } from 'vitest';
import type { DirectoryNode } from '../../../shared/types';
import {
  DIRECTORY_NO_MATCHING_MESSAGE,
  collectLoadedDirectoryPaths,
  directoryNodeContextActionLabel,
  directoryNodeContextActions,
  directoryTreeHasCollapsedFolders,
} from './directoryTree';

function node(partial: Partial<DirectoryNode> & Pick<DirectoryNode, 'name' | 'path'>): DirectoryNode {
  return {
    isDirectory: true,
    isDeferred: false,
    isSymlink: false,
    isExpanded: false,
    isLoading: false,
    ...partial,
  };
}

describe('directoryTree helpers', () => {
  it('collects loaded directory paths without descending into symlink directories', () => {
    const root = node({
      name: 'repo',
      path: '/repo',
      children: [
        node({ name: 'src', path: '/repo/src', children: [node({ name: 'ui', path: '/repo/src/ui' })] }),
        node({ name: 'linked', path: '/repo/linked', isSymlink: true, children: [node({ name: 'hidden', path: '/repo/linked/hidden' })] }),
        node({ name: 'file.ts', path: '/repo/file.ts', isDirectory: false }),
      ],
    });

    expect(collectLoadedDirectoryPaths(root)).toEqual(['/repo', '/repo/src', '/repo/src/ui']);
  });

  it('detects collapsed loaded folders', () => {
    const root = node({
      name: 'repo',
      path: '/repo',
      children: [node({ name: 'src', path: '/repo/src' })],
    });

    expect(directoryTreeHasCollapsedFolders(root, new Set(['/repo']))).toBe(true);
    expect(directoryTreeHasCollapsedFolders(root, new Set(['/repo', '/repo/src']))).toBe(false);
  });

  it('matches Rust directory row context actions', () => {
    expect(directoryNodeContextActions(node({ name: 'src', path: '/repo/src' }))).toEqual(['copyPath']);
    expect(directoryNodeContextActions(node({ name: 'main.ts', path: '/repo/main.ts', isDirectory: false }))).toEqual([
      'openInEditor',
      'openWithDefaultApp',
      'revealInFolder',
      'copyPath',
    ]);
  });

  it('matches Rust directory empty-state and context menu copy', () => {
    expect(DIRECTORY_NO_MATCHING_MESSAGE).toBe('No matching files or folders');
    expect(directoryNodeContextActionLabel('openInEditor')).toBe('<> Open in Editor');
    expect(directoryNodeContextActionLabel('openWithDefaultApp')).toBe('Open with Default App');
    expect(directoryNodeContextActionLabel('revealInFolder')).toBe('📂 Reveal in Folder');
    expect(directoryNodeContextActionLabel('copyPath')).toBe('⧉ Copy Path');
  });
});
