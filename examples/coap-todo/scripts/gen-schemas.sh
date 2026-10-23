#!/usr/bin/env bash
# Regenerates the compiled forms of the SenML schema (../../spec/senml.xsd) that
# two clients need, into schemas/ (committed):
#
#   senml.xsd.exi  the schema itself as an EXI stream, which exip loads at run time
#                  (C client); written by EXIficient 1.0.7 through exip's
#                  utils/xsd2exip tool
#   senml.xsc      Nagasena's compiled schema (C# client); written by Java Nagasena
#
#   scripts/gen-schemas.sh           regenerate
#   scripts/gen-schemas.sh --check   exit 1 if the committed files differ
#
# Needs Java 25 and Maven. Both tools come from this example's pins in
# third-party.lock (run scripts/fetch-third-party.sh --only exip,exi-testsuite):
# exip's utils/xsd2exip (Maven builds target/xsd2exip-cli.jar if it is missing), and
# the Nagasena jars in the W3C exi-testsuite (candidates/openexi/lib).
#
# Env: RSENML_ORACLE_JAR  use this xsd2exip-cli.jar instead of building one
#      NAGASENA_LIB       use the Nagasena jars in this directory
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
XSD="$ROOT/../../spec/senml.xsd"
DEPS="$("$ROOT/scripts/fetch-third-party.sh" --print-dir)"
ORACLE_DIR="$DEPS/exip/utils/xsd2exip"
JAR="${RSENML_ORACLE_JAR:-$ORACLE_DIR/target/xsd2exip-cli.jar}"
LIB="${NAGASENA_LIB:-$DEPS/exi-testsuite/candidates/openexi/lib}"

CHECK=0
case "${1:-}" in
    --check) CHECK=1 ;;
    "") ;;
    *) echo "usage: $0 [--check]" >&2; exit 2 ;;
esac

if [[ ! -f "$JAR" ]]; then
    [[ -d "$ORACLE_DIR" ]] || { echo "run scripts/fetch-third-party.sh first" >&2; exit 1; }
    mvn -q -f "$ORACLE_DIR/pom.xml" -DskipTests package >&2
fi
for j in nagasena.jar nagasena-rta.jar apache/xerces/2.11.0/xercesImpl.jar apache/xerces/2.11.0/xml-apis.jar; do
    [[ -f "$LIB/$j" ]] || { echo "missing $LIB/$j (run scripts/fetch-third-party.sh or set NAGASENA_LIB)" >&2; exit 1; }
done
CP="$LIB/nagasena.jar:$LIB/nagasena-rta.jar:$LIB/apache/xerces/2.11.0/xercesImpl.jar:$LIB/apache/xerces/2.11.0/xml-apis.jar"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
java -jar "$JAR" encode-xsd --xsd "$XSD" --xsd-exi-out "$WORK/senml.xsd.exi" 2> >(grep -v SLF4J >&2)
java -cp "$CP" "$ROOT/scripts/CompileXsc.java" "$XSD" "$WORK/senml.xsc"

status=0
for f in senml.xsd.exi senml.xsc; do
    if [[ $CHECK -eq 1 ]]; then
        cmp -s "$WORK/$f" "$ROOT/schemas/$f" || { echo "schemas/$f is out of date" >&2; status=1; }
    else
        mkdir -p "$ROOT/schemas"
        cp "$WORK/$f" "$ROOT/schemas/$f"
        echo "wrote schemas/$f ($(wc -c <"$WORK/$f") bytes)"
    fi
done
exit $status
