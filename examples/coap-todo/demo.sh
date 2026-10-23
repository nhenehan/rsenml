#!/usr/bin/env bash
# Starts the server on a free port and lets every client change the same todo
# list: each adds an item, marks another client's item done, and some remove
# items. After every round each client lists the items, and all lists must match
# what the round should have left. Exits 1 on any difference, then stops the server.
#
#   ./demo.sh                     all four clients (run ./build.sh first)
#   ./demo.sh --only rust,java    just these (rust, java, c, csharp)
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ONLY="rust,java,c,csharp"
case "${1:-}" in
    --only) ONLY="${2:?--only needs a list}" ;;
    "") ;;
    *) echo "usage: $0 [--only rust,java,c,csharp]" >&2; exit 2 ;;
esac

# The .NET launcher finds the runtime through DOTNET_ROOT.
if [[ -z "${DOTNET_ROOT:-}" ]]; then
    if command -v dotnet >/dev/null; then
        DOTNET_ROOT="$(dirname "$(readlink -f "$(command -v dotnet)")")"
    else
        DOTNET_ROOT="$HOME/.dotnet"
    fi
fi
export DOTNET_ROOT

declare -A CMD=(
    [rust]="$HERE/target/release/todo-rust"
    [java]="java -jar $HERE/java-client/target/todo-java.jar"
    [c]="$HERE/build/c-client/todo-c"
    [csharp]="$HERE/build/csharp-client/bin/todo-csharp"
)
declare -A FILE=(
    [rust]="$HERE/target/release/todo-rust"
    [java]="$HERE/java-client/target/todo-java.jar"
    [c]="$HERE/build/c-client/todo-c"
    [csharp]="$HERE/build/csharp-client/bin/todo-csharp"
)
IFS=, read -r -a CLIENTS <<<"$ONLY"
for c in "${CLIENTS[@]}"; do
    [[ -n "${FILE[$c]:-}" ]] || { echo "unknown client $c" >&2; exit 2; }
    [[ -e "${FILE[$c]}" ]] || { echo "$c client not built (${FILE[$c]}): run ./build.sh" >&2; exit 1; }
done
SERVER="$HERE/target/release/coap-todo-server"
[[ -x "$SERVER" ]] || { echo "server not built: run ./build.sh" >&2; exit 1; }

# A free UDP port: bind port 0 and see what the system picked.
PORT="$(python3 -c 'import socket; s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')"
LOG="$(mktemp)"
"$SERVER" --port "$PORT" >"$LOG" 2>&1 &
SERVER_PID=$!
trap 'kill $SERVER_PID 2>/dev/null || true; rm -f "$LOG"' EXIT
for _ in $(seq 50); do
    grep -q listening "$LOG" && break
    sleep 0.1
done
grep -q listening "$LOG" || { cat "$LOG" >&2; exit 1; }
echo "server on coap://127.0.0.1:$PORT/todos; clients: ${CLIENTS[*]}"

run() { # client args...
    local c="$1"
    shift
    # shellcheck disable=SC2086 # CMD holds a command line
    ${CMD[$c]} --port "$PORT" "$@"
}

failures=0
declare -A TITLE=() MARK=()

expected() {
    local id
    for id in $(printf '%s\n' "${!TITLE[@]}" | sort -n); do
        printf '%s [%s] %s\n' "$id" "${MARK[$id]}" "${TITLE[$id]}"
    done
}

check() { # round name
    local want got c
    want="$(expected)"
    for c in "${CLIENTS[@]}"; do
        if ! got="$(run "$c" list)"; then
            echo "  FAIL $c: list failed" >&2
            failures=$((failures + 1))
        elif [[ "$got" != "$want" ]]; then
            echo "  FAIL $c after $1:" >&2
            diff <(echo "$want") <(echo "$got") >&2 || true
            failures=$((failures + 1))
        else
            echo "  ok   $c sees ${#TITLE[@]} item(s)"
        fi
    done
}

echo "round 1: every client adds an item"
declare -a IDS=()
for c in "${CLIENTS[@]}"; do
    title="Item from $c: ünïcode & <markup>"
    id="$(run "$c" add "$title")"
    echo "  $c added $id"
    IDS+=("$id")
    TITLE[$id]="$title"
    MARK[$id]=" "
done
check "round 1"
expected | sed 's/^/    /'

n=${#CLIENTS[@]}
echo "round 2: every client marks the next client's item done"
for i in "${!CLIENTS[@]}"; do
    id="${IDS[$(((i + 1) % n))]}"
    run "${CLIENTS[$i]}" "done" "$id"
    MARK[$id]="x"
    echo "  ${CLIENTS[$i]} marked $id done"
done
check "round 2"

echo "round 3: every other client removes the next client's item"
for ((i = 0; i + 1 < n; i += 2)); do
    id="${IDS[$((i + 1))]}"
    run "${CLIENTS[$i]}" remove "$id"
    unset "TITLE[$id]" "MARK[$id]"
    echo "  ${CLIENTS[$i]} removed $id"
done
check "round 3"
expected | sed 's/^/    /'

if [[ " ${CLIENTS[*]} " == *" rust "* ]]; then
    echo "round 3b: the server answers the same list in all four SenML formats"
    want="$(expected)"
    for f in json cbor xml exi; do
        if [[ "$(run rust --format "$f" list)" == "$want" ]]; then
            echo "  ok   $f"
        else
            echo "  FAIL the $f list differs" >&2
            failures=$((failures + 1))
        fi
    done
fi

echo "round 4: removing a removed item fails (4.04 Not Found) in every client"
gone="${IDS[1]:-}"
if [[ -n "$gone" ]]; then
    for c in "${CLIENTS[@]}"; do
        if run "$c" remove "$gone" 2>/dev/null; then
            echo "  FAIL $c: removing $gone again succeeded" >&2
            failures=$((failures + 1))
        else
            echo "  ok   $c refused"
        fi
    done
fi

if [[ $failures -ne 0 ]]; then
    echo "demo FAILED: $failures difference(s)" >&2
    exit 1
fi
echo "demo passed: ${CLIENTS[*]} agree after every round"
