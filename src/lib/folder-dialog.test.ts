import { describe, expect, it } from 'vitest';
import { newProject } from './defaults';
import { projectFolderDialogOptions } from './folder-dialog';

describe('project folder dialog', () => {
  it('starts each picker in its own previously selected folder', () => {
    const project = newProject();
    project.source.roots = ['E:\\source'];
    project.target.root = 'E:\\target';

    expect(projectFolderDialogOptions(project, 'source').defaultPath).toBe('E:\\source');
    expect(projectFolderDialogOptions(project, 'target').defaultPath).toBe('E:\\target');
  });

  it('leaves the start path unset when that side has no saved folder', () => {
    const project = newProject();
    project.target.root = 'E:\\target';

    expect(projectFolderDialogOptions(project, 'source')).not.toHaveProperty('defaultPath');
  });

  it('allows multiple folders when adding sources and one when replacing a folder', () => {
    const project = newProject();
    project.source.roots = ['E:\\first', 'F:\\second'];
    expect(projectFolderDialogOptions(project, 'source')).toMatchObject({ multiple: true, defaultPath: 'F:\\second' });
    expect(projectFolderDialogOptions(project, 'source', 0)).toMatchObject({ multiple: false, defaultPath: 'E:\\first' });
    expect(projectFolderDialogOptions(project, 'target').multiple).toBe(false);
  });
});
