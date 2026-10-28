// The reference task's hidden tests: what docs/SPEC.md asks and the visible tests do not check.
// Copied into src/ only after the agent is done, so it cannot fit the code to them.
import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { App } from './App';
import { MonthReport } from './components/MonthReport';
import { parseCsv, parseLine } from './lib/csv';
import { money, report } from './lib/report';
import { SAMPLE } from './sample';

const text = (html: string) => html.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ');

test('a doubled quote inside a quoted description', () => {
  expect(parseLine('2026-03-04,gifts,9.99,"He said ""thanks"", twice"').description).toBe('He said "thanks", twice');
});

// The spec says what a DD.MM.YYYY date means, not how parseLine stores it: the report is checked.
test('a DD.MM.YYYY date counts in its own month', () => {
  const csv = 'date,category,amount,description\n11.03.2026,groceries,11.70,Market\n2026-03-02,groceries,1.00,Bread';
  const months = report(parseCsv(csv));
  expect(months.map((m) => m.month)).toEqual(['2026-03']);
  expect(money(months[0]?.total ?? 0)).toBe('12.70');
});

test('a refund counts against its own category', () => {
  const [march] = report(parseCsv(SAMPLE));
  expect(march?.categories.map(([name, amount]) => [name, money(amount)])).toEqual([
    ['groceries', '37.10'],
    ['transport', '18.00'],
  ]);
  expect(money(march?.total ?? 0)).toBe('55.10');
});

test('a refund alone shows as a negative amount, two decimals', () => {
  const csv = 'date,category,amount,description\n2026-05-03,groceries,-5.00,"Refund, eggs"';
  const shown = text(renderToStaticMarkup(<MonthReport transactions={parseCsv(csv)} />));
  expect(shown).toContain('2026-05 groceries -5.00 total -5.00');
});

test('many cents add up exactly', () => {
  const rows = Array.from({ length: 1000 }, (_, i) => `2026-03-${String((i % 28) + 1).padStart(2, '0')},coffee,0.01,c`);
  const [march] = report(parseCsv(['date,category,amount,description', ...rows].join('\n')));
  expect(money(march?.total ?? 0)).toBe('10.00');
});

test('the whole sample, month by month', () => {
  const shown = text(renderToStaticMarkup(<MonthReport transactions={parseCsv(SAMPLE)} />));
  expect(shown).toContain('2026-03 groceries 37.10 transport 18.00 total 55.10');
  expect(shown).toContain('2026-04 transport 2.90 total 2.90');
  expect(shown.indexOf('2026-03')).toBeLessThan(shown.indexOf('2026-04'));
});

test('one month alone', () => {
  const shown = text(renderToStaticMarkup(<MonthReport transactions={parseCsv(SAMPLE)} month="2026-04" />));
  expect(shown).toContain('2026-04');
  expect(shown).not.toContain('2026-03');
});

test('a month with no transactions', () => {
  const shown = text(renderToStaticMarkup(<MonthReport transactions={parseCsv(SAMPLE)} month="2026-05" />));
  expect(shown).toContain('2026-05');
  expect(shown).toContain('0.00');
  expect(shown).not.toContain('2026-03');
});

test('the month picker', () => {
  const html = renderToStaticMarkup(<App />);
  expect(html).toMatch(/<select/);
  expect(/<label[^>]*>[^]*?Month[^]*?<\/label>/.test(html) || /aria-label="Month"/.test(html)).toBe(true);
  const options = [...html.matchAll(/<option[^>]*>([^<]*)<\/option>/g)].map((m) => m[1]);
  expect(options).toEqual(['All months', '2026-03', '2026-04']);
});
