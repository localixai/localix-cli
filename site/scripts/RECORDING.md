# Recording the terminal demo

The landing's demo (`public/demo-llx.cast`) is a real llx run, recorded with asciinema and played
into a read-only xterm.js terminal (`src/demo/player.ts`), so it scrolls back and copies like
one. A session picks its provider and model with `/model`, switches to bypass with Shift+Tab and
hands a fix to two agents; then, from the shell as CI calls it, `llx exec --schema` reads the
diff and answers with data.

1. A clean home with the release build and the `fix-test` fixture as a repository, with the
   CI check's schema committed:

   ```bash
   cargo build --release
   D=/tmp/llx-demo; rm -rf $D && mkdir -p $D/bin $D/.config/llx
   ln -s "$PWD/target/release/llx" $D/bin/llx
   cp -R evals/fixtures/fix-test $D/stats && git -C $D/stats init -q
   printf '%s\n' '{"type": "object", "required": ["ship", "bump", "why"], "properties": {' \
     '  "ship": {"type": "boolean"}, "bump": {"enum": ["patch", "minor", "major"]},' \
     '  "why": {"type": "string", "description": "One line."}}}' > $D/stats/ship.json
   git -C $D/stats add -A && git -C $D/stats commit -qm start
   ```

2. Record it on a ChatGPT sign-in (no key is spent): copy `openai.json` into the demo's config
   for the take, and delete it after. The other providers' keys are placeholders, there only so
   `/model` shows their tabs; nothing is sent to them.

   ```bash
   cp ~/.config/llx/openai.json $D/.config/llx/
   env -i PATH=$D/bin:/usr/bin:/bin HOME=$D TERM=xterm-256color LANG=en_US.UTF-8 \
     LLX_NO_DAEMON=1 PYTHONDONTWRITEBYTECODE=1 \
     OPENROUTER_API_KEY=x ANTHROPIC_API_KEY=x DEEPSEEK_API_KEY=x XAI_API_KEY=x \
     asciinema rec --headless --window-size 100x30 \
     -c "expect site/scripts/record-llx.exp $D/stats" $D/raw.cast
   rm $D/.config/llx/openai.json
   ```

   A take with a good run is kept. A part that went wrong is recorded again alone, from the
   same shell prompt, and spliced in after the last good prompt; nothing else is recorded twice.

3. Polish the take: the setup cut, typed commands played a key at a time, the waits trimmed, the
   agents' work played faster, read pauses and chapters added, the demo's home shown as `~` and
   the account's name hidden. Then check that nothing private is left, and that the screen is
   the one recorded:

   ```bash
   python3 site/scripts/polish-cast.py $D/raw.cast site/public/demo-llx.cast $D/stats
   grep -cE '/Users/|/tmp/|sk-or-|@[a-z0-9-]+\.' site/public/demo-llx.cast   # 0
   asciinema convert -f txt site/public/demo-llx.cast /dev/stdout
   ```
