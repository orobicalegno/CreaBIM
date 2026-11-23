#!/usr/bin/env bash
# Publish the Volumen builds of a pull request as the rolling pre-release
# "volumen-anteprima", without ever deleting it first.
#
#   publish-preview.sh <dist-dir> <commit-sha> <pr-number>
#
# Needs GH_TOKEN and GH_REPO (owner/name). The order keeps the last good
# preview downloadable whatever goes wrong:
#
#   1. a newer commit of the PR has come meanwhile: stop, it will publish;
#   1b. a permanent copy, named for its date and commit, is made first:
#      as a hidden draft, its files uploaded and checked, then shown; if
#      anything fails the draft goes and nothing else is touched. Copies are
#      never deleted here (a keeping policy is for later);
#   2. leftovers of an interrupted run are put right (see `tidy`);
#   3. the new files are uploaded beside the old ones, under "next-" names,
#      and checked (every file there, at its size);
#   4. only then each old file steps aside ("prev-") and the new one takes
#      its name; then the tag moves to the commit and the notes change;
#   5. last, the old files go.
#
# A failure in 1-3 leaves the preview exactly as it was. One in 4 leaves it
# part old, part new for a moment: the next run puts it right before
# anything else.
set -euo pipefail

dist=$1
sha=$2
pr=$3
tag=volumen-anteprima
api="repos/$GH_REPO"

log() { printf '%s\n' "$*"; }

# 1. still the newest commit of the PR?
head=$(gh api "$api/pulls/$pr" --jq .head.sha)
if [ "$head" != "$sha" ]; then
  log "The PR has a newer commit (${head:0:7}) than this build (${sha:0:7}): it publishes, not this one."
  exit 0
fi

notes=$(mktemp)
cat > "$notes" <<NOTES
Anteprima di Volumen dalla PR #${pr} (commit ${sha:0:7}), compilata il $(date -u +%Y-%m-%d).

- **Windows:** scarica \`Volumen-windows-x64.zip\`, estrailo e avvia \`Volumen.exe\`. Se Windows avvisa di un editore sconosciuto: *Ulteriori informazioni* › *Esegui comunque*.
- **Fedora:** \`sudo dnf install ./Volumen-*.rpm\`, poi avvia Volumen dal menu delle applicazioni.
- **Altri Linux:** estrai \`Volumen-linux-x64.tar.gz\` e avvia \`./volumen\`.

Versione di prova, sostituita a ogni aggiornamento riuscito: un aggiornamento con controlli falliti non la sostituisce.
NOTES
title="Volumen – anteprima (PR #${pr})"

# the files of release $1 (its id): id, name and size, one per line
assets_of() { gh api --paginate "$api/releases/$1/assets" --jq '.[] | [.id, .name, .size] | @tsv'; }

# 1b. the permanent copy of this build
# a build already kept (a re-run after a failed swap) is not kept twice
kept_already=$(gh api --paginate "$api/releases" --jq ".[] | select(.draft | not) | select(.tag_name | startswith(\"$tag-\")) | select(.tag_name | contains(\"-${sha:0:7}\")) | .tag_name")
kept_already=${kept_already%%$'\n'*}
if [ -n "$kept_already" ]; then
  log "Already kept as $kept_already."
else
  # the copy's name: its date and commit, and a number when (a re-run) that
  # name is taken already
  release_of() { gh api --paginate "$api/releases" --jq ".[] | select(.tag_name == \"$1\") | .id"; }
  keep_base="$tag-$(date -u +%Y%m%d-%H%M)-${sha:0:7}"
  keep_tag=$keep_base
  k=1
  while [ -n "$(release_of "$keep_tag")" ]; do
    k=$((k + 1))
    keep_tag="$keep_base-$k"
  done
  keep_notes=$(mktemp)
  cat > "$keep_notes" <<NOTES
Copia conservata dell'anteprima di Volumen dalla PR #${pr} (commit ${sha:0:7}), compilata il $(date -u +%Y-%m-%d). Non viene sostituita: serve a riprendere una versione precedente o a capire da quando c'è un problema.

L'anteprima più recente è sempre in **volumen-anteprima**.
NOTES
  gh release create "$keep_tag" "$dist"/* --draft --prerelease --latest=false --target "$sha" \
    --title "Volumen – anteprima ${sha:0:7} ($(date -u +%Y-%m-%d))" --notes-file "$keep_notes" >/dev/null || {
    log "The permanent copy could not be made: nothing published."
    kid=$(release_of "$keep_tag")
    [ -z "$kid" ] || gh api -X DELETE "$api/releases/$kid" >/dev/null || true
    exit 1
  }
  kid=$(release_of "$keep_tag")
  kept_ok=1
  for f in "$dist"/*; do
    n=$(basename "$f")
    got=$(assets_of "$kid" | awk -F'\t' -v n="$n" '$2 == n { print $3 }')
    [ "$got" = "$(stat -c %s "$f")" ] || { log "Copy check failed for $n"; kept_ok=0; }
  done
  if [ $kept_ok -ne 1 ] || ! gh api -X PATCH "$api/releases/$kid" -F draft=false >/dev/null; then
    log "The permanent copy failed its check: its draft goes, nothing published."
    gh api -X DELETE "$api/releases/$kid" >/dev/null || true
    exit 1
  fi
  log "Kept as $keep_tag."
fi

# the release, made empty the first time (never deleted)
if ! gh release view "$tag" >/dev/null 2>&1; then
  log "No $tag yet: making it."
  gh release create "$tag" --prerelease --target "$sha" --title "$title" --notes-file "$notes"
fi
rid=$(gh api "$api/releases/tags/$tag" --jq .id)

# the release's files: id, name and size, one per line
assets() { gh api --paginate "$api/releases/$rid/assets" --jq '.[] | [.id, .name, .size] | @tsv'; }
asset_id() { assets | awk -F'\t' -v n="$1" '$2 == n { print $1 }'; }
rename() { gh api -X PATCH "$api/releases/assets/$1" -f name="$2" >/dev/null; }
drop() { gh api -X DELETE "$api/releases/assets/$1" >/dev/null; }

# 2. an interrupted run: a file that stepped aside with no new one in its
#    place comes back; the rest of the "next-" and "prev-" files go
tidy() {
  local id name
  while IFS=$'\t' read -r id name _; do
    case $name in
      prev-*)
        if [ -z "$(asset_id "${name#prev-}")" ]; then
          log "Putting back $name"
          rename "$id" "${name#prev-}"
        else
          drop "$id"
        fi
        ;;
    esac
  done < <(assets)
  while IFS=$'\t' read -r id name _; do
    case $name in next-*) log "Removing leftover $name"; drop "$id" ;; esac
  done < <(assets)
}
tidy

# 3. the new files, beside the old ones
stage=$(mktemp -d)
names=()
for f in "$dist"/*; do
  n=$(basename "$f")
  # the checksums last of all, so they match the files as long as can be
  [ "$n" = SHA256SUMS.txt ] || names+=("$n")
  cp "$f" "$stage/next-$n"
done
[ -f "$dist/SHA256SUMS.txt" ] && names+=(SHA256SUMS.txt)
[ ${#names[@]} -gt 0 ] || { log "No files to publish"; exit 1; }
gh release upload "$tag" "$stage"/next-*

bad=0
for n in "${names[@]}"; do
  want=$(stat -c %s "$dist/$n")
  got=$(assets | awk -F'\t' -v n="next-$n" '$2 == n { print $3 }')
  if [ "$got" != "$want" ]; then
    log "Upload check failed for $n: ${got:-missing} bytes, expected $want"
    bad=1
  fi
done
if [ $bad -ne 0 ]; then
  log "The preview stays as it was."
  exit 1
fi

# 4. the swap
for n in "${names[@]}"; do
  old=$(asset_id "$n")
  [ -z "$old" ] || rename "$old" "prev-$n"
  rename "$(asset_id "next-$n")" "$n"
done
if gh api "$api/git/refs/tags/$tag" >/dev/null 2>&1; then
  gh api -X PATCH "$api/git/refs/tags/$tag" -f sha="$sha" -F force=true >/dev/null
else
  gh api -X POST "$api/git/refs" -f ref="refs/tags/$tag" -f sha="$sha" >/dev/null
fi
gh release edit "$tag" --prerelease --title "$title" --notes-file "$notes" >/dev/null

# 5. the old files: those stepped aside, and those the new build no longer
#    has (the RPM's name carries its date)
while IFS=$'\t' read -r id name _; do
  keep=0
  for n in "${names[@]}"; do [ "$name" = "$n" ] && keep=1; done
  if [ $keep -eq 0 ]; then
    log "Removing old $name"
    drop "$id"
  fi
done < <(assets)

log "Published ${sha:0:7} as $tag:"
assets | cut -f2-
