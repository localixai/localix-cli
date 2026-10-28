import { expect, test } from 'bun:test';
import { parseCsv } from './csv';
import { money, report } from './report';

const csv = (rows: string[]) => parseCsv(['date,category,amount,description', ...rows].join('\n'));

test('one month, categories by name', () => {
  const [march] = report(csv(['2026-03-04,transport,18.00,b', '2026-03-02,groceries,12.50,a']));
  expect(march?.month).toBe('2026-03');
  expect(march?.categories.map(([name]) => name)).toEqual(['groceries', 'transport']);
});

test('cents add up exactly and show two decimals', () => {
  const [march] = report(csv(['2026-03-01,coffee,0.10,a', '2026-03-02,coffee,0.20,b', '2026-03-03,coffee,30.00,c']));
  expect(money(march?.total ?? 0)).toBe('30.30');
});
