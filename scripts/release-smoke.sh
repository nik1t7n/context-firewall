#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 /path/to/cfw" >&2
  exit 64
fi

CFW_BIN="$1"
if [[ ! -x "$CFW_BIN" ]]; then
  echo "cfw binary is not executable: $CFW_BIN" >&2
  exit 66
fi
CFW_BIN="$(cd "$(dirname "$CFW_BIN")" && pwd -P)/$(basename "$CFW_BIN")"

WORK_DIR="$(mktemp -d)"
DATA_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$WORK_DIR" "$DATA_DIR"
}
trap cleanup EXIT

export CFW_DATA_DIR="$DATA_DIR"
export CFW_SESSION="release-smoke"

cd "$WORK_DIR"

"$CFW_BIN" --version | grep -E '^cfw [0-9]+\.[0-9]+\.[0-9]+'

first_output="$("$CFW_BIN" run -- sh -c 'seq 1 220')"
first_span="$(printf '%s\n' "$first_output" | sed -n 's#^span: cfw://span/\([[:alnum:]_-][[:alnum:]_-]*\)$#\1#p' | head -n 1)"
if [[ -z "$first_span" ]]; then
  echo "cfw run did not print a span handle" >&2
  printf '%s\n' "$first_output" >&2
  exit 1
fi

second_output="$("$CFW_BIN" run -- sh -c 'seq 1 220')"
printf '%s\n' "$second_output" | grep -F '[context-firewall: duplicate output]'

show_output="$("$CFW_BIN" show "$first_span" --lines 1:3)"
printf '%s\n' "$show_output" | grep -F '1'
printf '%s\n' "$show_output" | grep -F '2'

printf '{"items":[1,2,3],"name":"release-smoke"}\n' > payload.json
stdin_output="$("$CFW_BIN" run --stdin-file payload.json -- sh -c 'wc -c >/dev/null; seq 1 40')"
stdin_span="$(printf '%s\n' "$stdin_output" | sed -n 's#^span: cfw://span/\([[:alnum:]_-][[:alnum:]_-]*\)$#\1#p' | head -n 1)"
if [[ -z "$stdin_span" ]]; then
  echo "cfw run --stdin-file did not print a span handle" >&2
  printf '%s\n' "$stdin_output" >&2
  exit 1
fi

"$CFW_BIN" spans --json | grep -F '"id"'
"$CFW_BIN" receipt --json | grep -F '"net_estimated_saved"'
"$CFW_BIN" policy explain -- rg needle node_modules | grep -F 'action: block'

echo "release smoke passed for $("$CFW_BIN" --version)"
