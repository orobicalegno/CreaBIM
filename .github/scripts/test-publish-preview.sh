#!/usr/bin/env bash
# Checks publish-preview.sh against a stand-in for GitHub (test/gh): the
# rolling preview is never deleted, and a cut or corrupted upload, an
# interruption in the middle of the swap, or a build overtaken by a newer
# commit never leaves it without a complete, consistent set of files.
#
#   bash .github/scripts/test-publish-preview.sh
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
export PATH="$here/test:$PATH" GH_REPO=o/r GH_TOKEN=x STATE=$work/state
mkdir -p "$STATE"
publish="$here/publish-preview.sh"
fails=0

build() { # the files of build $1, its RPM dated $2
  rm -rf "$work/dist"; mkdir -p "$work/dist"
  for f in Volumen-windows-x64.zip Volumen-linux-x64.tar.gz "Volumen-1.1.2_$2-x86_64.rpm" SHA256SUMS.txt; do
    echo "$f of build $1" > "$work/dist/$f"
  done
}
files() { cut -f2 "$STATE"/assets/* 2>/dev/null | sort | tr '\n' ' '; }
# the size of the published zip tells which build it is
zip_of() { echo "Volumen-windows-x64.zip of build $1" | wc -c; }
zip_now() { awk -F'\t' '$2 == "Volumen-windows-x64.zip" { print $3 }' "$STATE"/assets/* 2>/dev/null; }
run() { rm -f "$STATE"/count.*; HEAD_SHA=$1 bash "$publish" "$work/dist" "$2" 13 >"$work/out" 2>&1; echo $?; }
check() { # name, condition
  if eval "$2"; then echo "ok   $1"; else echo "FAIL $1 (files: $(files); tag: $(cat "$STATE/tag" 2>/dev/null))"; sed 's/^/       | /' "$work/out"; fails=$((fails+1)); fi
}
complete() { # exactly the four files of a build, no leftovers
  [ "$(files)" = "SHA256SUMS.txt Volumen-1.1.2_$1-x86_64.rpm Volumen-linux-x64.tar.gz Volumen-windows-x64.zip " ]
}

build A 20261120; e=$(run aaaa aaaa)
check "first publish makes the release" "[ $e = 0 ] && complete 20261120 && [ \"\$(cat \$STATE/tag)\" = aaaa ]"

build B 20261123; e=$(run bbbb bbbb)
check "an update replaces every file and moves the tag" "[ $e = 0 ] && complete 20261123 && [ \$(zip_now) = \$(zip_of B) ] && [ \"\$(cat \$STATE/tag)\" = bbbb ]"

build C 20261124
e=$(FAIL_UPLOAD_AT=3 run cccc cccc)
check "a cut upload fails and leaves build B" "[ $e != 0 ] && [ \$(zip_now) = \$(zip_of B) ] && [ \"\$(cat \$STATE/tag)\" = bbbb ]"

e=$(TRUNCATE=1 run cccc cccc)
check "a corrupted upload fails and leaves build B" "[ $e != 0 ] && [ \$(zip_now) = \$(zip_of B) ] && [ \"\$(cat \$STATE/tag)\" = bbbb ]"

e=$(FAIL_RENAME_AT=2 run cccc cccc)
check "an interruption right after a file stepped aside fails" "[ $e != 0 ]"
e=$(run cccc cccc)
check "the next run puts back and completes build C" "[ $e = 0 ] && complete 20261124 && [ \$(zip_now) = \$(zip_of C) ] && [ \"\$(cat \$STATE/tag)\" = cccc ]"

build D 20261125
e=$(FAIL_RENAME_AT=5 run dddd dddd)
check "an interruption later in the swap fails" "[ $e != 0 ]"
check "  ...and every download link still answers" "for f in Volumen-windows-x64.zip Volumen-linux-x64.tar.gz SHA256SUMS.txt; do files | grep -q \"\$f \" || exit 1; done"
e=$(run dddd dddd)
check "the next run completes build D" "[ $e = 0 ] && complete 20261125 && [ \$(zip_now) = \$(zip_of D) ]"

build OLD 20261101; e=$(run eeee oldsha)
check "a build overtaken by a newer commit publishes nothing" "[ $e = 0 ] && complete 20261125 && [ \$(zip_now) = \$(zip_of D) ]"

check "the release was never deleted or made again" "[ \$(grep -c created \$STATE/log) = 1 ]"

# the kept copies (intervento 2): one per published build, never deleted
kept() { for d in "$STATE"/kept/*/; do [ -d "$d" ] && [ ! -f "$d/draft" ] && cat "$d/tag"; done | wc -l; }
drafts() { for d in "$STATE"/kept/*/; do [ -f "$d/draft" ] && echo x; done | wc -l; }
check "every published build left its kept copy (A, B, C, D)" "[ \$(kept) = 4 ]"
check "no half-made copy is left behind" "[ \$(drafts) = 0 ]"

build F 20261126
e=$(TRUNCATE_KEEP=1 run ffff ffff)
check "a corrupted kept copy stops the publish" "[ $e != 0 ] && [ \$(zip_now) = \$(zip_of D) ] && [ \$(kept) = 4 ] && [ \$(drafts) = 0 ]"
e=$(FAIL_KEEP_AT=2 run ffff ffff)
check "a cut upload of the kept copy stops the publish" "[ $e != 0 ] && [ \$(zip_now) = \$(zip_of D) ] && [ \$(kept) = 4 ] && [ \$(drafts) = 0 ]"
e=$(run ffff ffff)
check "then it publishes, and keeps its copy" "[ $e = 0 ] && complete 20261126 && [ \$(kept) = 5 ]"

[ $fails = 0 ] && echo "all checks passed" || { echo "$fails check(s) failed"; exit 1; }
