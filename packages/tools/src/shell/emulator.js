"use strict";
// The session's screen in a terminal emulator. A frame is the whole screen as a terminal draws
// it, made by llx from what the program printed (text, styles and the cursor, nothing else), and
// it is written over the last. The grid is the session's; the font is fitted to the window. No
// input is read: nothing typed here goes anywhere.
const base = document.title;
const style = () => getComputedStyle(document.documentElement);
const css = (name) => style().getPropertyValue(name).trim();
const DARK = ["#484f58", "#ff7b72", "#3fb950", "#d29922", "#58a6ff", "#bc8cff", "#39c5cf",
  "#b1bac4", "#6e7681", "#ffa198", "#56d364", "#e3b341", "#79c0ff", "#d2a8ff", "#56d4dd", "#f0f6fc"];
const LIGHT = ["#24292e", "#d73a49", "#28a745", "#b08800", "#0366d6", "#6f42c1", "#1b7c83",
  "#6a737d", "#959da5", "#cb2431", "#22863a", "#b08800", "#005cc5", "#5a32a3", "#1b7c83", "#24292e"];
const NAMES = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];
// Whether a #rrggbb colour reads as light.
function light(hex) {
  const n = parseInt(hex.slice(1), 16);
  return 0.2126 * (n >> 16) + 0.7152 * ((n >> 8) & 255) + 0.0722 * (n & 255) >= 127.5;
}
// The page's colours; the ground is the window's, so the terminal paints none.
function theme() {
  const fg = css("--fg"), bg = css("--bg");
  const colours = {
    background: "rgba(0,0,0,0)", foreground: fg, cursor: css("--cursor"), cursorAccent: bg,
    selectionBackground: fg + "47",
  };
  (light(bg) ? LIGHT : DARK).forEach((colour, i) => {
    const name = NAMES[i % 8];
    colours[i < 8 ? name : "bright" + name[0].toUpperCase() + name.slice(1)] = colour;
  });
  return colours;
}
const COLS = +css("--cols"), ROWS = +css("--rows");
const term = new Terminal({
  cols: COLS, rows: ROWS, scrollback: 0, allowTransparency: true, disableStdin: true,
  cursorBlink: false, cursorStyle: "block", cursorInactiveStyle: "block",
  fontFamily: css("--mono"), theme: theme(),
});
const fit = new FitAddon.FitAddon();
term.loadAddon(fit);
term.open(document.getElementById("t"));
// The space around the grid, as the page's style leaves it.
const PAD = 14, LINE = 1.1;
// The largest font at which the whole grid fits the window.
function size() {
  if (!innerWidth || !innerHeight) return;
  term.options.lineHeight = LINE;
  let low = 6, high = 16;
  for (let step = 0; step < 16; step++) {
    const mid = (low + high) / 2;
    term.options.fontSize = mid;
    const fits = fit.proposeDimensions();
    if (fits && fits.cols >= COLS && fits.rows >= ROWS) low = mid; else high = mid;
  }
  term.options.fontSize = Math.floor(low * 100) / 100;
}
// A cell as the emulator lays it out (the fit addon reads the same).
const cell = () => term._core._renderService.dimensions.css.cell;
// For checks: a cell, and the grid with its space around it.
function cells() {
  const { width, height } = cell();
  return [width, height, COLS * width + 2 * PAD, ROWS * height + 2 * PAD];
}
// The window this grid wants: its size at a comfortable font. The window keeps that shape.
function shape() {
  term.options.fontSize = 13;
  term.options.lineHeight = LINE;
  const { width, height } = cell();
  const wanted = [COLS * width + 2 * PAD, ROWS * height + 2 * PAD];
  size();
  return wanted;
}
size();
addEventListener("resize", size);
document.fonts.ready.then(size);
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
  term.options.theme = theme();
});
function F(lines, row, column, hidden, end, drawn) {
  term.write("\x1b[H\x1b[2J" + drawn);
  document.title = end ? base + end : base;
}
