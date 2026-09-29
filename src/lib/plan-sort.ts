import type { PlannedItem } from './types';

export type PlanSortKey = 'project' | 'source' | 'key' | 'target' | 'decision';
export type PlanSort = { key: PlanSortKey | null; direction: 'asc' | 'desc' };

const collator = new Intl.Collator('ko-KR', { numeric: true, sensitivity: 'base' });

function sortValue(item: PlannedItem, key: PlanSortKey): string {
  switch (key) {
    case 'project': return item.projectName;
    case 'source': return item.sourcePath;
    case 'key': return item.extractedKey ?? '';
    case 'target': return item.proposedTargetPath ?? '';
    case 'decision': return item.decision === 'move' ? '이동' : item.reasonText ?? item.decision;
  }
}

export function nextPlanSort(current: PlanSort, key: PlanSortKey): PlanSort {
  return { key, direction: current.key === key && current.direction === 'asc' ? 'desc' : 'asc' };
}

export function sortedPlanItems(items: PlannedItem[], sort: PlanSort): PlannedItem[] {
  if (!sort.key) return items;
  const key = sort.key;
  const multiplier = sort.direction === 'asc' ? 1 : -1;
  return items
    .map((item, index) => ({ item, index }))
    .sort((a, b) => multiplier * collator.compare(sortValue(a.item, key), sortValue(b.item, key)) || a.index - b.index)
    .map(({ item }) => item);
}
