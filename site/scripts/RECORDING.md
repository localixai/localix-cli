# Recording the terminal demo

The landing's first demo (`public/demo-llx.cast`) is a real llx session, recorded with asciinema
and replayed by asciinema-player.

1. A clean home with the release build and the `fix-test` fixture as a repository:

   ```bash
   cargo build --release
   rm -rf /tmp/llx-demo && mkdir -p /tmp/llx-demo/bin
   ln -s "$PWD/target/release/llx" /tmp/llx-demo/bin/llx
   cp -R evals/fixtures/fix-test /tmp/llx-demo/stats && git -C /tmp/llx-demo/stats init -q
   git -C /tmp/llx-demo/stats add -A && git -C /tmp/llx-demo/stats commit -qm start
   ```

2. Record it, with an OpenRouter key in the environment (never in the cast):

   ```bash
   env -i PATH=/tmp/llx-demo/bin:/usr/bin:/bin HOME=/tmp/llx-demo TERM=xterm-256color \
     LANG=en_US.UTF-8 OPENROUTER_API_KEY=... \
     asciinema rec --headless --window-size 100x30 \
     -c "expect site/scripts/record-llx.exp /tmp/llx-demo/stats" /tmp/llx-demo/raw.cast
   ```

   The script picks the provider and model with `/login`, cycles the modes with Shift+Tab into
   bypass (the TUI redraws in place, so expect cannot wait on an approval's text) and gives the
   task.

3. Trim the waits, add the read pauses and chapters, and check that no key is in it:

   ```bash
   python3 site/scripts/polish-cast.py /tmp/llx-demo/raw.cast site/public/demo-llx.cast
   grep -c 'sk-or-' site/public/demo-llx.cast   # 0
   ```
