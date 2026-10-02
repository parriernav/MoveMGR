import { describe, expect, it } from 'vitest';
import { emptyState, newProject } from './defaults';
import { mergeSourceFolders, migrateSourceFolders } from './source-folders';
import type { LocalState } from './types';

describe('source folders', () => {
  it('migrates cached single folders and leaves empty registrations empty', () => {
    const projects = ['E:\\source', ''].map((root) => ({ ...newProject(), source: { ...newProject().source, roots: undefined, root } }));
    const legacy = { ...emptyState, projects } as unknown as LocalState;
    const restored = migrateSourceFolders(legacy);
    expect(restored.projects.map((project) => project.source.roots)).toEqual([['E:\\source'], []]);
    expect(restored.projects[0].source).not.toHaveProperty('root');
    expect(legacy.projects[0].source).toHaveProperty('root');
  });

  it('does not restore a stale legacy folder when the saved list is explicitly empty', () => {
    const project = newProject();
    Object.assign(project.source, { root: 'stale folder' });
    expect(migrateSourceFolders({ ...emptyState, projects: [project] }).projects[0].source.roots).toEqual([]);
  });

  it('appends multiple selections in order and removes equivalent Windows duplicates', () => {
    const current = ['E:\\source'];
    expect(mergeSourceFolders(current, ['F:\\new', 'e:/SOURCE/', 'F:\\new\\', ''])).toEqual(['E:\\source', 'F:\\new']);
    expect(current).toEqual(['E:\\source']);
  });

  it('replaces one folder without losing the other registrations', () => {
    expect(mergeSourceFolders(['E:\\one', 'F:\\two', 'G:\\three'], ['H:\\replacement'], 1)).toEqual(['E:\\one', 'H:\\replacement', 'G:\\three']);
    expect(mergeSourceFolders(['E:\\one', 'F:\\two'], ['e:/ONE'], 1)).toEqual(['E:\\one']);
  });

  it('deduplicates Windows drive roots and UNC folders while preserving POSIX case', () => {
    expect(mergeSourceFolders(['E:\\', '\\\\server\\share'], ['e:/', '//SERVER/SHARE/'])).toEqual(['E:\\', '\\\\server\\share']);
    expect(mergeSourceFolders(['/Source'], ['/source'])).toEqual(['/Source', '/source']);
  });
});
