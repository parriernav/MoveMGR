import type { OpenDialogOptions } from '@tauri-apps/plugin-dialog';
import type { Project } from './types';

export function projectFolderDialogOptions(project: Project, side: 'source' | 'target', sourceIndex?: number): OpenDialogOptions {
  const currentPath = side === 'source' ? project.source.roots[sourceIndex ?? project.source.roots.length - 1] ?? '' : project.target.root;
  return {
    directory: true,
    multiple: side === 'source' && sourceIndex === undefined,
    title: side === 'source' ? '소스 폴더 선택' : '타겟 폴더 선택',
    ...(currentPath.trim() ? { defaultPath: currentPath } : {})
  };
}
