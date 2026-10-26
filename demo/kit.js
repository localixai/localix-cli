// What the demos share: the header, the theme a person pins, one toast, and seeded data, so the
// same page always holds the same rows and a task has one right answer.

/** The header: the demo's name, the way back to all of them, and the theme. */
function header(name) {
  const home = (location.pathname.split("/").pop() || "index.html") === "index.html";
  const top = document.createElement("header");
  top.className = "top";
  top.innerHTML = `<span class="brand">${name}</span>
    ${home ? "" : '<nav aria-label="Demos"><a href="index.html">All demos</a></nav>'}
    <button type="button" class="icon quiet" id="theme" aria-label="Theme: system"></button>`;
  document.body.prepend(top);
  const button = top.querySelector("#theme");
  const themes = ["system", "light", "dark"];
  const icons = { system: "◐", light: "☀", dark: "☾" };
  const show = (theme) => {
    if (theme === "system") document.documentElement.removeAttribute("data-theme");
    else document.documentElement.dataset.theme = theme;
    button.textContent = icons[theme];
    button.setAttribute("aria-label", `Theme: ${theme}`);
    button.title = `Theme: ${theme}`;
  };
  let theme = "system";
  try {
    theme = localStorage.getItem("demo-theme") || "system";
  } catch {}
  show(theme);
  button.addEventListener("click", () => {
    theme = themes[(themes.indexOf(theme) + 1) % themes.length];
    try {
      localStorage.setItem("demo-theme", theme);
    } catch {}
    show(theme);
  });
}

/** A short message at the bottom of the page, read out by screen readers. */
function toast(text, ms = 2600) {
  let box = document.getElementById("toast");
  if (!box) {
    box = document.createElement("div");
    box.id = "toast";
    box.className = "toast";
    box.setAttribute("role", "status");
    box.hidden = true;
    document.body.append(box);
  }
  box.textContent = text;
  box.hidden = false;
  clearTimeout(box.timer);
  box.timer = setTimeout(() => (box.hidden = true), ms);
}

/** A seeded random generator (mulberry32): the same seed, the same numbers, every load. */
function seeded(seed) {
  return () => {
    seed |= 0;
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** One of `items`, by the generator. */
function pick(random, items) {
  return items[Math.floor(random() * items.length)];
}

const money = new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" });

/** Text escaped for HTML. */
function esc(text) {
  return String(text).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}
