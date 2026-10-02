import type { Project, RuleTag, SourceRuleSnapshot, TargetRuleSnapshot } from './types';

export type RuleKind = RuleTag['kind'];

export function snapshotRules(project: Project, kind: 'source'): SourceRuleSnapshot;
export function snapshotRules(project: Project, kind: 'target'): TargetRuleSnapshot;
export function snapshotRules(project: Project, kind: RuleKind): SourceRuleSnapshot | TargetRuleSnapshot {
  if (kind === 'source') {
    const { recursive, includeHidden, moveUnit = 'file', extensions, nameFilters } = project.source;
    return structuredClone({ recursive, includeHidden, moveUnit, extensions, nameFilters });
  }
  return structuredClone({
    key: project.key,
    comparisonOptions: project.comparisonOptions,
    destination: project.target.destination,
    conflict: project.conflict
  });
}

export function applyRuleTag(project: Project, tag: RuleTag): Project {
  const next = structuredClone(project);
  if (tag.kind === 'source') {
    Object.assign(next.source, { moveUnit: 'file', ...structuredClone(tag.rules) });
  } else {
    const rules = structuredClone(tag.rules);
    next.key = rules.key;
    next.comparisonOptions = rules.comparisonOptions;
    next.target.destination = rules.destination;
    next.conflict = rules.conflict;
  }
  return next;
}

export function normalizedTagName(value: string): string {
  return value.trim();
}
