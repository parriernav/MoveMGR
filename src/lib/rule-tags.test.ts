import { describe, expect, it } from 'vitest';
import { newProject } from './defaults';
import { applyRuleTag, snapshotRules } from './rule-tags';
import type { RuleTag } from './types';

describe('rule tags', () => {
  it('applies only source conditions and preserves the project paths', () => {
    const project = newProject();
    project.source.root = 'source';
    project.target.root = 'target';
    project.source.moveUnit = 'sameNameGroup';
    const tag: RuleTag = {
      id: crypto.randomUUID(),
      name: '사진',
      kind: 'source',
      rules: { recursive: true, includeHidden: false, extensions: { mode: 'only', values: ['jpg'], includeExtensionless: false }, nameFilters: [] }
    };
    const applied = applyRuleTag(project, tag);
    expect(applied.source.root).toBe('source');
    expect(applied.target.root).toBe('target');
    expect(applied.source.extensions).toEqual(tag.rules.extensions);
    expect(applied.source.moveUnit).toBe('file');
    expect(project.source.extensions).toEqual({ mode: 'all' });
  });

  it('saves and applies the source move unit without changing target rules', () => {
    const project = newProject();
    project.source.root = 'source';
    project.target.root = 'target';
    project.source.moveUnit = 'sameNameGroup';
    const saved = snapshotRules(project, 'source');
    project.source.moveUnit = 'file';
    expect(saved.moveUnit).toBe('sameNameGroup');
    const applied = applyRuleTag(project, { id: crypto.randomUUID(), name: '파일명 묶음', kind: 'source', rules: saved });
    expect(applied.source.moveUnit).toBe('sameNameGroup');
    expect(applied.source.root).toBe('source');
    expect(applied.target).toEqual(project.target);
    expect(project.source.moveUnit).toBe('file');
  });

  it('snapshots target rules independently of later edits', () => {
    const project = newProject();
    project.target.destination = { mode: 'matchSubfolder', searchBase: '', folderExtractor: { kind: 'whole' }, comparison: { kind: 'equals' }, noMatch: 'skip', multipleMatches: 'roundRobin' };
    const saved = snapshotRules(project, 'target');
    project.target.destination = { mode: 'root' };
    expect(saved.destination.mode).toBe('matchSubfolder');
    const applied = applyRuleTag(project, { id: crypto.randomUUID(), name: '균등 배분', kind: 'target', rules: saved });
    expect(applied.target.destination).toEqual(saved.destination);
    if (applied.target.destination.mode === 'matchSubfolder') {
      expect(applied.target.destination.multipleMatches).toBe('roundRobin');
    }
  });
});
