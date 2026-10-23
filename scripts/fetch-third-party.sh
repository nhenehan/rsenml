#!/usr/bin/env bash
# Fetch the tools pinned in third-party.lock (rexi's schema generator, exip's EXIficient
# oracle) into a cache outside the working tree. The build itself does not need them:
# Cargo gets rexi from GitHub.
#
#   scripts/fetch-third-party.sh                 check out every pinned commit
#   scripts/fetch-third-party.sh --check         exit 1 unless every checkout is present,
#                                                clean and at its pin
#   scripts/fetch-third-party.sh --update NAME   re-pin NAME to its remote HEAD
#   scripts/fetch-third-party.sh --print-dir     print the cache directory
#
# Env: RSENML_THIRD_PARTY  cache directory
#      (default ${XDG_CACHE_HOME:-$HOME/.cache}/rsenml/third_party)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCK="$ROOT/third-party.lock"
DIR="${RSENML_THIRD_PARTY:-${XDG_CACHE_HOME:-$HOME/.cache}/rsenml/third_party}"

MODE=fetch
UPDATE=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --check) MODE=check ;;
        --update) MODE=update; UPDATE="${2:?--update needs a name}"; shift ;;
        --print-dir) echo "$DIR"; exit 0 ;;
        -h|--help) sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
    shift
done

command -v git >/dev/null || { echo "missing required command: git" >&2; exit 1; }
[[ -f "$LOCK" ]] || { echo "missing $LOCK" >&2; exit 1; }

# Lock entries as "name url sha [sparse]" lines.
entries() { awk '!/^[[:space:]]*(#|$)/ { print $1, $2, $3, $4 }' "$LOCK"; }

# A path relative to this repository becomes an absolute local path.
resolve_url() {
    case "$1" in
        *://*|git@*) echo "$1" ;;
        /*) echo "$1" ;;
        *) (cd "$ROOT" && cd "$1" && pwd) ;;
    esac
}

dirty() { [[ -n "$(git -C "$1" status --porcelain --untracked-files=no)" ]]; }

checkout() { # name url sha [sparse]
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
    # Keep the remote in step with the lock, so an old cache never fetches elsewhere.
    git -C "$path" remote set-url origin "$url"
    if dirty "$path"; then
        echo "$name: $path has local changes; refusing to touch it" >&2
        return 1
    fi
    if [[ "$(git -C "$path" rev-parse -q --verify HEAD || true)" != "$sha" ]]; then
        echo "$name: fetching $sha"
        local filter=()
        [[ -n "$sparse" ]] && filter=(--filter=blob:none)
        # Fetching a bare commit needs uploadpack.allowReachableSHA1InWant (GitHub
        # allows it); a local repository may not, so fall back to fetching its refs.
        if ! git -C "$path" fetch -q --depth 1 "${filter[@]}" origin "$sha" 2>/dev/null; then
            git -C "$path" fetch -q "${filter[@]}" origin '+refs/heads/*:refs/remotes/origin/*'
        fi
        git -C "$path" checkout -q --detach "$sha"
    fi
    echo "$name: at $sha"
}

check() { # name url sha
    local name="$1" sha="$3" path="$DIR/$1"
    if [[ ! -d "$path/.git" ]]; then
        echo "MISSING  $name ($path)" >&2
        return 1
    fi
    local head
    head="$(git -C "$path" rev-parse -q --verify HEAD || true)"
    if [[ "$head" != "$sha" ]]; then
        echo "MOVED    $name: at ${head:-nothing}, pinned $sha" >&2
        return 1
    fi
    if dirty "$path"; then
        echo "DIRTY    $name" >&2
        return 1
    fi
    echo "ok       $name $sha"
}

status=0
case "$MODE" in
    check)
        while read -r name url sha _sparse; do
            check "$name" "$url" "$sha" || status=1
        done < <(entries)
        exit $status
        ;;
    update)
        line="$(entries | awk -v n="$UPDATE" '$1 == n')"
        [[ -n "$line" ]] || { echo "no lock entry named $UPDATE" >&2; exit 2; }
        read -r name url old sparse <<<"$line"
        new="$(git ls-remote "$(resolve_url "$url")" HEAD | awk '{ print $1 }')"
        [[ ${#new} -eq 40 ]] || { echo "could not resolve HEAD of $url" >&2; exit 1; }
        awk -v n="$name" -v s="$new" \
            '!/^[[:space:]]*#/ && $1 == n { $3 = s; print; next } { print }' \
            "$LOCK" >"$LOCK.tmp" && mv "$LOCK.tmp" "$LOCK"
        echo "$name: $old -> $new"
        checkout "$name" "$url" "$new" "$sparse"
        ;;
    fetch)
        mkdir -p "$DIR"
        while read -r name url sha sparse; do
            checkout "$name" "$url" "$sha" "$sparse" || status=1
        done < <(entries)
        ;;
esac
echo "third-party cache: $DIR"
exit $status
