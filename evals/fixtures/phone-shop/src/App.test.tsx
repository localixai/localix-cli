import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { App } from './App';
import { PHONES } from './phones';

test('every phone is on the page', () => {
  const html = renderToStaticMarkup(<App />);
  for (const phone of PHONES) expect(html).toContain(phone.name);
});
