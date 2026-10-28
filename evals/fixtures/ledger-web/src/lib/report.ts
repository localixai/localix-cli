/** The monthly report: what was spent per category, month by month. */
import type { Transaction } from './csv';

export type Month = { month: string; categories: [string, number][]; total: number };

export function monthOf(t: Transaction): string {
  return t.date.slice(0, 7);
}

export function report(transactions: Transaction[]): Month[] {
  const months = new Map<string, Map<string, number>>();
  for (const t of transactions) {
    if (t.amount <= 0) continue;
    const month = months.get(monthOf(t)) ?? new Map<string, number>();
    month.set(t.category, (month.get(t.category) ?? 0) + t.amount);
    months.set(monthOf(t), month);
  }
  return [...months.keys()].sort().map((month) => {
    const categories = [...(months.get(month) ?? new Map<string, number>())].sort(([a], [b]) => a.localeCompare(b));
    return { month, categories, total: categories.reduce((sum, [, amount]) => sum + amount, 0) };
  });
}

export function money(amount: number): string {
  return String(amount);
}
