import { describe, expect, it } from 'vitest';
import { cloneProject, emptyState, newProject } from './defaults';

describe('project defaults', () => {
  it('starts new installations in English', () => {
    expect(emptyState.preferences.language).toBe('en');
  });

  it('creates a safe unselected project', () => {
    const project = newProject();
    expect(project.checked).toBe(false);
    expect(project.source.recursive).toBe(false);
    expect(project.source.extensions).toEqual({ mode: 'all' });
    expect(project.target.destination).toEqual({ mode: 'root' });
    expect(project.conflict).toBe('skip');
  });

  it('duplicates a project with a new id and clears selection', () => {
    const source = newProject();
    source.name = '문서 정리';
    source.checked = true;
    const duplicate = cloneProject(source);

    expect(duplicate.id).not.toBe(source.id);
    expect(duplicate.name).toBe('문서 정리 복사본');
    expect(duplicate.checked).toBe(false);
    expect(duplicate.source).toEqual(source.source);
  });
});
