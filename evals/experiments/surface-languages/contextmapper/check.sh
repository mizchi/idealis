#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "$0")"
cm="${CONTEXT_MAPPER:-cm}"

validate() {
  local file="$1" output
  # Context Mapper CLI 6.12.0 can exit zero even on validation errors.
  if ! output=$("$cm" validate -i "$file" 2>&1) ||
    ! grep -Fq "The CML file '$file' has been validated without errors." <<< "$output"; then
    printf '%s\n' "$output" >&2
    return 1
  fi
  printf '%s\n' "$output"
}

if (( $# > 0 )); then
  validate "$1"
else
  for file in permissions.cml bank.cml order.cml sessions.cml service.cml; do
    validate "$file"
  done
fi
