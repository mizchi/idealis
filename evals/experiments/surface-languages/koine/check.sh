#!/usr/bin/env bash
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(cd -- "$here/../../../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf -- "$tmp"' EXIT

"${KOINE:-koine}" build "$here" --warnings-as-errors --target csharp --out "$tmp/cs"
"${KOINE:-koine}" build "$here" --warnings-as-errors --target typescript --out "$tmp/ts"

cd -- "$root"
pnpm exec tsc -p "$tmp/ts"

dotnet new console --no-restore --output "$tmp/cs"
cp -- "$here/Checks.cs" "$tmp/cs/Program.cs"
dotnet run --project "$tmp/cs"
