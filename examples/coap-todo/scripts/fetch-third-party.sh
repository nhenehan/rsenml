#!/usr/bin/env bash
# Fetch the libraries pinned in ../third-party.lock into a cache outside the
# working tree. The C client (CMake) and the C# client (.NET) build them from there,
# and scripts/gen-schemas.sh runs its tools from there.
#
#   scripts/fetch-third-party.sh                 check out every pin
#   scripts/fetch-third-party.sh --check         exit 1 unless every checkout is
#                                                present, clean and at its pin
#   scripts/fetch-third-party.sh --update NAME   re-pin NAME to its remote HEAD
#   scripts/fetch-third-party.sh --print-dir     print the cache directory
#   ... --only NAME[,NAME]                       limit fetch/check to these names
#
# Env: RSENML_EXAMPLES_CACHE  cache root
#      (default ${XDG_CACHE_HOME:-$HOME/.cache}/rsenml/examples); this example
#      uses <root>/coap-todo
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/third-party.lock"
DIR="${RSENML_EXAMPLES_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/rsenml/examples}/coap-todo"

MODE=fetch
UPDATE=""
ONLY=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --check) MODE=check ;;
        --update) MODE=update; UPDATE="${2:?--update needs a name}"; shift ;;
        --only) ONLY="${2:?--only needs names}"; shift ;;
        --print-dir) echo "$DIR"; exit 0 ;;
        -h|--help) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
    shift
done

require() { command -v "$1" >/dev/null || { echo "missing required command: $1" >&2; return 1; }; }

# Lock entries as "name url rev [path]" lines, limited by --only.
entries() {
    awk -v only=",$ONLY," '!/^[[:space:]]*(#|$)/ && (only == ",," || index(only, "," $1 ",")) {
        print $1, $2, $3, $4 }' "$LOCK"
}

# A path relative to the example directory becomes an absolute local path.
resolve_url() {
    case "$1" in
        *://*|git@*|/*) echo "$1" ;;
        *) (cd "$ROOT" && cd "$1" && pwd) ;;
    esac
}

git_dirty() { [[ -n "$(git -C "$1" status --porcelain --untracked-files=no)" ]]; }

git_checkout() { # name url sha [sparse]
    local name="$1" url sha="$3" sparse="${4:-}" path="$DIR/$1"
    url="$(resolve_url "$2")"
    if [[ ! -d "$path/.git" ]]; then
        [[ -e "$path" ]] && { echo "$path exists but is not a git checkout" >&2; return 1; }
        mkdir -p "$path"
        git -C "$path" init -q
        git -C "$path" remote add origin "$url"
        if [[ -n "$sparse" ]]; then
            git -C "$path" sparse-checkout set "$sparse"
        fi
    fi
    [[ "$(git -C "$path" remote get-url origin)" == "$url" ]] || git -C "$path" remote set-url origin "$url"
    if git_dirty "$path"; then
        echo "$name: $path has local changes; refusing to touch it" >&2
        return 1
    fi
    if [[ "$(git -C "$path" rev-parse -q --verify HEAD || true)" == "$sha" ]]; then
        echo "$name: at $sha"
        return 0
    fi
    echo "$name: fetching $sha"
    # A sparse checkout fetches file contents only when they are needed.
    local filter=()
    [[ -n "$sparse" ]] && filter=(--filter=blob:none)
    # A bare commit can be fetched from GitHub; a local repository may refuse, so
    # fall back to fetching all of its branches.
    if ! git -C "$path" fetch -q --depth 1 "${filter[@]}" origin "$sha" 2>/dev/null; then
        git -C "$path" fetch -q "${filter[@]}" origin '+refs/heads/*:refs/remotes/origin/*'
    fi
    git -C "$path" checkout -q --detach "$sha"
    # libcoap keeps a few things in submodules; none is needed without DTLS.
}

git_check() { # name sha
    local name="$1" sha="$2" path="$DIR/$1" head
    [[ -d "$path/.git" ]] || { echo "MISSING  $name ($path)" >&2; return 1; }
    head="$(git -C "$path" rev-parse -q --verify HEAD || true)"
    [[ "$head" == "$sha" ]] || { echo "MOVED    $name: at ${head:-nothing}, pinned $sha" >&2; return 1; }
    git_dirty "$path" && { echo "DIRTY    $name" >&2; return 1; }
    echo "ok       $name $sha"
}

svn_checkout() { # name url rev path
    local name="$1" url="$2" rev="$3" path="$DIR/$1"
    require svn || return 1
    if [[ -e "$path" && ! -d "$path/.svn" ]]; then
        echo "$path exists but is not a Subversion checkout" >&2
        return 1
    fi
    if [[ -d "$path/.svn" ]]; then
        if [[ -n "$(svn status -q "$path")" ]]; then
            echo "$name: $path has local changes; refusing to touch it" >&2
            return 1
        fi
        if [[ "$(svn info --show-item revision "$path")" == "$rev" ]]; then
            echo "$name: at r$rev"
            return 0
        fi
        echo "$name: updating to r$rev"
        svn update -q -r "$rev" "$path"
    else
        echo "$name: checking out r$rev"
        mkdir -p "$DIR"
        svn checkout -q -r "$rev" "$url/$4" "$path"
    fi
}

svn_check() { # name rev
    local name="$1" rev="$2" path="$DIR/$1" at
    [[ -d "$path/.svn" ]] || { echo "MISSING  $name ($path)" >&2; return 1; }
    at="$(svn info --show-item revision "$path")"
    [[ "$at" == "$rev" ]] || { echo "MOVED    $name: at r$at, pinned r$rev" >&2; return 1; }
    [[ -z "$(svn status -q "$path")" ]] || { echo "DIRTY    $name" >&2; return 1; }
    echo "ok       $name r$rev"
}

require git
[[ -f "$LOCK" ]] || { echo "missing $LOCK" >&2; exit 1; }
status=0
case "$MODE" in
    check)
        while read -r name url rev _path; do
            if [[ "$url" == svn:* ]]; then svn_check "$name" "$rev" || status=1
            else git_check "$name" "$rev" || status=1; fi
        done < <(entries)
        ;;
    update)
        line="$(awk -v n="$UPDATE" '!/^[[:space:]]*(#|$)/ && $1 == n' "$LOCK")"
        [[ -n "$line" ]] || { echo "no lock entry named $UPDATE" >&2; exit 2; }
        read -r name url old path <<<"$line"
        if [[ "$url" == svn:* ]]; then
            new="$(svn info --show-item last-changed-revision "${url#svn:}/$path")"
            [[ "$new" =~ ^[0-9]+$ ]] || { echo "could not resolve HEAD of $url" >&2; exit 1; }
        else
            new="$(git ls-remote "$(resolve_url "$url")" HEAD | awk '$2 == "HEAD" { print $1 }')"
            [[ ${#new} -eq 40 ]] || { echo "could not resolve HEAD of $url" >&2; exit 1; }
        fi
        awk -v n="$name" -v s="$new" '!/^[[:space:]]*#/ && $1 == n { $3 = s; print; next } { print }' \
            "$LOCK" >"$LOCK.tmp" && mv "$LOCK.tmp" "$LOCK"
        echo "$name: $old -> $new (run the script again to fetch it)"
        ;;
    fetch)
        mkdir -p "$DIR"
        while read -r name url rev path; do
            if [[ "$url" == svn:* ]]; then svn_checkout "$name" "${url#svn:}" "$rev" "$path" || status=1
            else git_checkout "$name" "$url" "$rev" "$path" || status=1; fi
        done < <(entries)
        ;;
esac
exit $status
