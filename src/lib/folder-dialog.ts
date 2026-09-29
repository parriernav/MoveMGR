import type { OpenDialogOptions } from '@tauri-apps/plugin-dialog';
import type { Project } from './types';

export function projectFolderDialogOptions(project: Project, side: 'source' | 'target'): OpenDialogOptions {
  const currentPath = side === 'source' ? project.source.root : project.target.root;
  return {
    directory: true,
    multiple: false,
    title: side === 'source' ? '소스 폴더 선택' : '타겟 폴더 선택',
    ...(currentPath.trim() ? { defaultPath: currentPath } : {})
  };
}
