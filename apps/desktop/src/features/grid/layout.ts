export type Layout = 'auto' | '1' | '2' | '3' | '4';
export function columnsFor(count: number, layout: Layout): number {
  return layout === 'auto' ? Math.max(1, Math.ceil(Math.sqrt(count))) : Number(layout);
}
export function reorder(ids: string[], source: string, target: string): string[] {
  const from = ids.indexOf(source), to = ids.indexOf(target);
  if (from < 0 || to < 0 || from === to) return [...ids];
  const next = [...ids];
  next.splice(from, 1); next.splice(to, 0, source);
  return next;
}
