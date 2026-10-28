// The phone-shop task's hidden tests: what docs/BRIEF.md asks and the visible test does not check.
// Copied into src/ only after the agent is done, so it cannot fit the code to them.
import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { App } from './App';
import { PHONES } from './phones';

const html = renderToStaticMarkup(<App />);
const unescape = (s: string) =>
  s.replace(/&amp;/g, '&').replace(/&#x27;|&#39;/g, "'").replace(/&quot;/g, '"').replace(/&lt;/g, '<').replace(/&gt;/g, '>');
const text = unescape(html.replace(/<[^>]+>/g, '')).replace(/\s+/g, ' ');
const hrefs = [...html.matchAll(/href="([^"]*)"/g)].map((m) => unescape(m[1] ?? ''));
const orders = hrefs
  .filter((h) => h.startsWith('https://wa.me/35799123456?'))
  .map((h) => new URL(h).searchParams.get('text'));
const message = (p: (typeof PHONES)[number]) => `Hi! I'd like to order the ${p.name} (${p.storage}).`;

test('each price has the euro sign first and a comma between thousands', () => {
  for (const price of ['€1,299', '€949', '€1,449', '€429', '€799', '€289']) expect(text).toContain(price);
});

test('each phone shows its brand and storage', () => {
  for (const phone of PHONES) {
    expect(text).toContain(phone.brand);
    expect(text).toContain(phone.storage);
  }
});

test('a phone in stock is ordered on WhatsApp with its own message', () => {
  for (const phone of PHONES.filter((p) => p.stock > 0)) expect(orders).toContain(message(phone));
});

test('a phone out of stock is sold out, not ordered', () => {
  expect(text).toContain('Sold out');
  for (const phone of PHONES.filter((p) => p.stock === 0)) expect(orders).not.toContain(message(phone));
});

test('a phone with 1 to 3 left says how many', () => {
  expect(text).toContain('Only 2 left');
  expect(text).not.toContain('Only 4 left');
});

test('the brand filter: All, then each brand in the order it first appears', () => {
  const brands = ['All', 'Apple', 'Samsung', 'Google', 'Xiaomi'];
  const buttons = [...html.matchAll(/<button[^>]*>([\s\S]*?)<\/button>/g)]
    .map((m) => unescape((m[1] ?? '').replace(/<[^>]+>/g, '')).trim())
    // A brand's button may say more after its name (a count, an icon's label): `Apple 2`, `Apple2`.
    .flatMap((t) => brands.filter((b) => new RegExp(`^${b}(?![A-Za-z])`).test(t)));
  expect(buttons).toEqual(brands);
});

test('the contact block: the address, the hours and the number to call', () => {
  expect(text).toContain('28 Anexartisias St');
  expect(text).toContain('10:00');
  expect(text).toContain('+357 99 123456');
  expect(hrefs).toContain('tel:+35799123456');
});
