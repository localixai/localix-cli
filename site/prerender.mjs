// Puts the rendered page into dist/index.html, so the text is in the HTML before any script runs
// (for crawlers, link previews and visitors without JavaScript); the client then hydrates it.
// Runs after `vite build` and `vite build --ssr`, then removes the server bundle.
import { readFile, rm, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const dist = fileURLToPath(new URL('./dist/', import.meta.url));
const server = fileURLToPath(new URL('./dist-ssr/', import.meta.url));

const { render } = await import(new URL('entry-server.js', `file://${server}`).href);
const page = `${dist}index.html`;
const html = await readFile(page, 'utf8');
const mount = '<div id="root"></div>';
if (!html.includes(mount)) throw new Error(`${mount} is missing from dist/index.html`);
await writeFile(page, html.replace(mount, `<div id="root">${render()}</div>`));
await rm(server, { recursive: true, force: true });
console.log('prerendered dist/index.html');
