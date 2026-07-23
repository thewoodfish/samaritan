#!/usr/bin/env bash
#
# Purity check — the pipeline's dependency direction, enforced by the build.
#
# Adapted from Meredith's `pnpm purity`: an architectural rule a machine
# checks is worth ten stated in prose. The pure layer (schema, graph,
# registry) must never depend on the model (planning) or orchestration
# (dispatch). If it does, the boundary in PIPELINE.md is a fiction.
#
# Fails the build on any forbidden edge. Uses `cargo metadata` so it sees the
# real resolved graph, not just what Cargo.toml files happen to say.
#
# Written for portability — no associative arrays (macOS ships bash 3.2).

set -euo pipefail

cd "$(dirname "$0")/.."

# One rule per line: "<crate> <crate-it-must-not-depend-on> ...".
# The pure layer flows one way only; each crate lists everything downstream
# of it that it must never reach back into.
RULES="
samaritan-schema samaritan-graph samaritan-registry samaritan-planning samaritan-dispatch samaritan
samaritan-graph samaritan-registry samaritan-planning samaritan-dispatch samaritan
samaritan-registry samaritan-planning samaritan-dispatch samaritan
"

metadata="$(cargo metadata --format-version 1 --no-deps)"

violations=0
while read -r crate banned_list; do
  [ -z "$crate" ] && continue
  for banned in $banned_list; do
    if MDATA="$metadata" CRATE="$crate" BANNED="$banned" python3 - <<'PY'
import json, os, sys
md = json.loads(os.environ["MDATA"])
crate = os.environ["CRATE"]
banned = os.environ["BANNED"]
for pkg in md["packages"]:
    if pkg["name"] == crate:
        deps = {d["name"] for d in pkg["dependencies"]}
        sys.exit(0 if banned in deps else 1)
sys.exit(1)   # crate not found — no violation
PY
    then
      echo "PURITY VIOLATION: $crate depends on $banned"
      violations=$((violations + 1))
    fi
  done
done <<EOF
$RULES
EOF

if [ "$violations" -ne 0 ]; then
  echo ""
  echo "The pure layer must not depend on the model or dispatch. See PIPELINE.md."
  exit 1
fi

echo "purity: ok — the pure layer depends on nothing downstream"
