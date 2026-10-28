import { expect, test } from 'bun:test';
import { parseLine } from './csv';

test('a plain line', () => {
  const t = parseLine('2026-03-02,groceries,12.50,Corner shop');
  expect([t.date, t.category, t.amount, t.description]).toEqual(['2026-03-02', 'groceries', 12.5, 'Corner shop']);
});

test('a quoted description with a comma', () => {
  const t = parseLine('2026-03-04,transport,18.00,"Taxi, airport"');
  expect(t.amount).toBe(18);
  expect(t.description).toBe('Taxi, airport');
});
