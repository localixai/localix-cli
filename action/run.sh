#!/usr/bin/env bash
# The action's script (action.yml runs it): checks the inputs, installs llx, runs `llx exec` once
# and hands back the answer, the exit code and the spend. Inputs arrive as INPUT_* variables, so
# nothing from a workflow or from the model is ever part of a command's text.
set -uo pipefail

code=3 # until llx has run: not ready (apps/cli/src/exit.rs)
out='' events='' result='' error='' installed='' cmd=()

die() {
  error="llx: $*"
  echo "$error" >&2
  exit 3
}

# The EXIT trap: outputs and summary are written whatever happened, then llx's own code is ours.
finish() {
  local status delim fence answer='' last='{}'
  [ -z "$installed" ] || exit "$code" # install-only: nothing ran, nothing to report
  case "$code" in
    0) status="achieved" ;;
    1) status="not-achieved" ;;
    2) status="out-of-budget" ;;
    3) status="not-ready" ;;
    130) status="interrupted" ;;
    *) status="failed" ;;
  esac
  if [ -s "$events" ] && command -v jq >/dev/null; then
    last=$(jq -R -c 'fromjson? | select(.type == "run.completed")' "$events" | tail -n 1)
    [ -n "$last" ] || last='{}'
  fi
  field() { jq -r "$1" <<<"$last" 2>/dev/null || echo "$2"; } # a path into the event, else the default
  [ -n "$error" ] || error=$(field '.error // .data_error // empty' '')
  # Only what llx wrote with -o: with a schema that is the data, or nothing when none matched.
  [ ! -s "$result" ] || answer=$(<"$result")
  if [ "${#answer}" -gt 900000 ]; then
    echo "llx: the answer is too long for an output; it is in result-file" >&2
    answer=''
  fi
  if [ -n "${GITHUB_OUTPUT:-}" ]; then
    {
      printf '%s=%s\n' exit-code "$code" status "$status" turns "$(field '.usage.agent.calls // 0' 0)" \
        cost-usd "$(field '.usage.cost_usd // 0' 0)" result-file "$result" events-file "$events"
      # A heredoc under a delimiter the answer does not hold: the model cannot add outputs.
      delim="llx_$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n')"
      while [[ $answer == *"$delim"* ]]; do delim="${delim}x"; done
      printf 'result<<%s\n%s\n%s\n' "$delim" "$answer" "$delim"
    } >>"$GITHUB_OUTPUT"
  fi
  {
    printf '### llx: %s\n\n| model | turns | cost | time |\n|---|---|---|---|\n' "$status"
    printf '| %s | %s | $%s | %ss |\n\n' "$(field '.usage.agent.model // "-"' -)" "$(field '.usage.agent.calls // 0' 0)" \
      "$(field '.usage.cost_usd // 0' 0)" "$(($(field '.duration_ms // 0' 0) / 1000))"
    [ -z "$error" ] || printf '%s\n\n' "$error"
    if [ -n "$answer" ] && [ -n "${INPUT_SCHEMA:-}" ]; then
      fence=$'\x60\x60\x60' # three backticks: the data as a JSON block
      printf '<details><summary>Answer</summary>\n\n%sjson\n%s\n%s\n\n</details>\n' "$fence" \
        "$(jq . <<<"$answer" 2>/dev/null | head -c 20000)" "$fence"
    elif [ -n "$answer" ]; then
      printf '<details><summary>Answer</summary>\n\n%s\n\n</details>\n' "${answer:0:20000}"
    fi
  } >>"${GITHUB_STEP_SUMMARY:-/dev/null}"
  exit "$code"
}
trap finish EXIT

# The release this action's tag is: [workspace.package]'s version in the Cargo.toml beside it.
default_version() {
  sed -n '/^\[workspace\.package\]/,/^\[/{s/^version *= *"\(.*\)".*/\1/p;}' "$GITHUB_ACTION_PATH/Cargo.toml" 2>/dev/null | head -n 1
}

sha256_of() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d ' ' -f 1; else shasum -a 256 "$1" | cut -d ' ' -f 1; fi
}

# in_range VALUE MAX: a whole number from 1 to MAX, as llx takes it.
in_range() {
  [[ $1 =~ ^[0-9]{1,6}$ ]] && [ "$((10#$1))" -ge 1 ] && [ "$((10#$1))" -le "$2" ]
}

validate() {
  local tool
  for tool in jq perl; do command -v "$tool" >/dev/null || die "needs $tool on PATH"; done
  [ -n "${INPUT_INSTALL_ONLY:-}${INPUT_PROMPT:-}${INPUT_PROMPT_FILE:-}" ] || die "give prompt or prompt-file"
  in_range "${INPUT_MAX_TURNS:-50}" 10000 || die "max-turns is a number from 1 to 10000"
  in_range "${INPUT_TIMEOUT:-900}" 86400 || die "timeout is a number of seconds from 1 to 86400"
  [[ -z ${INPUT_MODE:-} || $INPUT_MODE =~ ^[A-Za-z0-9_-]+$ ]] || die "mode is a name"
  if [ -n "${INPUT_WORKING_DIRECTORY:-}" ]; then
    cd -- "$INPUT_WORKING_DIRECTORY" 2>/dev/null || die "working-directory does not exist"
  fi
  [ -z "${INPUT_PROMPT_FILE:-}" ] || [ -f "$INPUT_PROMPT_FILE" ] || die "prompt-file does not exist"
  case "${INPUT_SCHEMA:-}" in
    '') ;;
    @*) [ -f "${INPUT_SCHEMA#@}" ] || die "schema file does not exist" ;;
    *) jq -e 'type == "object"' <<<"$INPUT_SCHEMA" >/dev/null 2>&1 || die "schema is not a JSON object" ;;
  esac
}

install_llx() {
  local version=${INPUT_VERSION:-} target base archive dir sums want
  if [ "$version" = system ]; then
    command -v llx >/dev/null || die "version: system, but llx is not on PATH"
    return
  fi
  version=${version#v}
  [ -n "$version" ] || version=$(default_version)
  [[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+([.+-][0-9A-Za-z.-]+)?$ ]] || die "no llx release to install (version '$version')"
  case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) target=x86_64-unknown-linux-gnu-headless ;; # no browser engine: agents in CI need none
    Darwin-*) target=universal-apple-darwin ;;
    *) die "no llx build for $(uname -s) $(uname -m); build it and use version: system" ;;
  esac
  dir=${RUNNER_TEMP:-${TMPDIR:-/tmp}}/llx-bin/$version
  archive=llx-$target.tar.gz
  base=https://github.com/rsaryev/llx/releases/download/v$version
  if [ ! -x "$dir/llx" ]; then
    sums=$(mktemp -d)
    curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$sums/$archive" "$base/$archive" || die "could not download $base/$archive"
    curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$sums/SHA256SUMS" "$base/SHA256SUMS" || die "could not download $base/SHA256SUMS"
    want=$(awk -v name="$archive" '$2 == name || $2 == "*" name { print $1 }' "$sums/SHA256SUMS")
    [ -n "$want" ] || die "SHA256SUMS lists no $archive"
    [ "$(sha256_of "$sums/$archive")" = "$want" ] || die "checksum mismatch for $archive"
    mkdir -p "$dir"
    tar -xzf "$sums/$archive" -C "$dir" llx || die "could not unpack $archive"
    rm -rf "$sums"
  fi
  export PATH="$dir:$PATH"
  [ -z "${GITHUB_PATH:-}" ] || echo "$dir" >>"$GITHUB_PATH"
}

# $NAME and ${NAME} become the variable's value when it is set, in one pass over the text; a name
# that is not set stays as written. Perl only reads %ENV here: nothing in the text is evaluated.
expand() {
  perl -pe 's#\$(?:\{([A-Za-z_][A-Za-z0-9_]*)\}|([A-Za-z_][A-Za-z0-9_]*))#exists $ENV{$1 // $2} ? $ENV{$1 // $2} : $&#ge'
}

# rules FLAG LINES: one --allow or --deny for each non-blank line.
rules() {
  local line
  while IFS= read -r line; do
    line=${line%$'\r'}
    line=${line#"${line%%[![:space:]]*}"}
    line=${line%"${line##*[![:space:]]}"}
    [ -z "$line" ] || cmd+=("$1" "$line")
  done <<<"$2"
}

main() {
  validate
  install_llx
  if [ -n "${INPUT_INSTALL_ONLY:-}" ]; then
    installed=1 code=0
    return
  fi
  out=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/llx-run-XXXXXX")
  events=$out/events.jsonl result=$out/result.txt
  : >"$result"
  {
    if [ -n "${INPUT_PROMPT_FILE:-}" ]; then printf '%s\n\n' "$(cat -- "$INPUT_PROMPT_FILE")"; fi
    printf '%s' "${INPUT_PROMPT:-}" | expand
  } >"$out/prompt.txt"

  [ -z "${INPUT_PROVIDER:-}" ] || export LLX_PROVIDER=$INPUT_PROVIDER
  [ -z "${INPUT_MODEL:-}" ] || export LLX_MODEL=$INPUT_MODEL
  [ -z "${INPUT_REASONING:-}" ] || export LLX_REASONING=$INPUT_REASONING
  export LLX_MODELS_REFRESH=0 LLX_NO_DAEMON=1 # a run is one process, with no catalog fetch to wait for

  local extra=()
  cmd=(llx exec --json -v -o "$result" --max-turns "${INPUT_MAX_TURNS:-50}" --timeout "${INPUT_TIMEOUT:-900}")
  [ -z "${INPUT_SCHEMA:-}" ] || cmd+=(--schema "$INPUT_SCHEMA")
  [ -z "${INPUT_MODE:-}" ] || cmd+=(--mode "$INPUT_MODE")
  rules --allow "${INPUT_ALLOW:-}"
  rules --deny "${INPUT_DENY:-}"
  read -r -d '' -a extra <<<"${INPUT_ARGS:-}" # every line, split on blanks
  # The prompt is stdin (`-`), a file, so no length limit of an argument applies. The trace goes
  # to the job's log (stderr); stdout holds the events and never the answer.
  "${cmd[@]}" ${extra[@]+"${extra[@]}"} - <"$out/prompt.txt" >"$events"
  code=$?
}

main
