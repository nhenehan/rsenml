#!/usr/bin/env bash
# Builds the server and every client whose toolchain is present, and says which
# it skipped. Nothing here is part of the rsenml build or gate.
#
#   ./build.sh                 build what can be built
#   ./build.sh --only rust,c   build just these clients (rust, java, c, csharp);
#                              the server is always built
#
# Needs: cargo (server, Rust client); java 25 + mvn (Java client); cmake + a C
# compiler + git (C client: exip and libcoap from scripts/fetch-third-party.sh);
# the .NET 10 SDK + svn (C# client: Nagasena for .NET from Subversion). dotnet is
# looked up on PATH, then in $DOTNET_ROOT, then in ~/.dotnet.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ONLY="rust,java,c,csharp"
case "${1:-}" in
    --only) ONLY="${2:?--only needs a list}" ;;
    "") ;;
    *) echo "usage: $0 [--only rust,java,c,csharp]" >&2; exit 2 ;;
esac
wanted() { [[ ",$ONLY," == *",$1,"* ]]; }
have() { command -v "$1" >/dev/null; }

built=()
skipped=()
skip() { skipped+=("$1 ($2)"); echo "skipping $1: $2" >&2; }

have cargo || { echo "cargo is required for the server" >&2; exit 1; }
echo "building the server and the Rust client (cargo)" >&2
cargo build -q --release --manifest-path "$HERE/Cargo.toml"
built+=(server)
wanted rust && built+=(rust)

if wanted java; then
    if have java && have mvn; then
        echo "building the Java client (maven)" >&2
        mvn -q -f "$HERE/java-client/pom.xml" package
        built+=(java)
    else
        skip java "needs java and mvn"
    fi
fi

if wanted c; then
    if have cmake && have cc && have git; then
        "$HERE/scripts/fetch-third-party.sh" --only exip,libcoap >/dev/null
        echo "building the C client (cmake: libcoap, exip)" >&2
        cmake -S "$HERE/c-client" -B "$HERE/build/c-client" \
            -DTODO_DEPS_DIR="$("$HERE/scripts/fetch-third-party.sh" --print-dir)" >/dev/null
        cmake --build "$HERE/build/c-client" -j >/dev/null
        built+=(c)
    else
        skip c "needs cmake, a C compiler and git"
    fi
fi

if wanted csharp; then
    DOTNET="$(command -v dotnet || true)"
    for d in "${DOTNET_ROOT:-}" "$HOME/.dotnet"; do
        [[ -z "$DOTNET" && -n "$d" && -x "$d/dotnet" ]] && DOTNET="$d/dotnet"
    done
    if [[ -n "$DOTNET" ]] && have svn; then
        "$HERE/scripts/fetch-third-party.sh" --only nagasenacs >/dev/null
        echo "building the C# client (dotnet: Com.AugustCellars.CoAP, Nagasena for .NET)" >&2
        DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1 "$DOTNET" build -c Release --nologo -v q \
            "$HERE/csharp-client/todo-csharp.csproj" \
            -p:NagasenaCsDir="$("$HERE/scripts/fetch-third-party.sh" --print-dir)/nagasenacs" \
            -o "$HERE/build/csharp-client/bin" >/dev/null
        built+=(csharp)
    else
        skip csharp "needs the .NET 10 SDK and svn"
    fi
fi

echo "built: ${built[*]}"
if [[ ${#skipped[@]} -gt 0 ]]; then
    echo "skipped: ${skipped[*]}"
fi
