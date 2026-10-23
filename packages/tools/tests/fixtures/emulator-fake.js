// Stands in for the terminal emulator: it keeps what it is told, so a check can read it back.
var Terminal = class {
  constructor(options) { window.made = options; this.options = options; }
  loadAddon() {}
  open(element) { window.opened = !!element; }
  write(text) { window.drawn = (window.drawn || "") + text; }
};
