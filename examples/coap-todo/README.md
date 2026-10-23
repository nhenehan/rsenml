# coap-todo: a todo list over CoAP, with SenML EXI messages

A small CoAP server keeps a todo list. Four clients read and change it. Each
client is in a different language and uses a different EXI library. Every message
body is a SenML pack ([RFC 8428](https://www.rfc-editor.org/rfc/rfc8428)). The
clients send it as `application/senml-exi`, which is SenML's XML form coded as EXI.

| program | language | CoAP | SenML EXI |
|---|---|---|---|
| `coap-todo-server` | Rust | [coap-lite](https://crates.io/crates/coap-lite) 0.13 over `std::net::UdpSocket` | [rsenml](https://github.com/nhenehan/rsenml) (EXI by [rexi](https://github.com/nhenehan/rexi)), plus JSON, CBOR and XML |
| `todo-rust` | Rust | coap-lite 0.13 | rsenml |
| `todo-java` | Java 25 | [Eclipse Californium](https://eclipse.dev/californium/) 3.14.0 | [EXIficient](https://github.com/EXIficient/exificient) 1.0.7 |
| `todo-c` | C11 | [libcoap](https://libcoap.net/) v4.3.5b | [exip](https://github.com/nhenehan/exip) (pinned) |
| `todo-csharp` | C# (.NET 10) | [Com.AugustCellars.CoAP](https://www.nuget.org/packages/Com.AugustCellars.CoAP) 1.10.0 | Nagasena for .NET ([OpenEXI](https://sourceforge.net/projects/openexi/)'s C# port, svn r1188) |

This example is not part of the rsenml build. The root `Cargo.toml` leaves out
`examples/`, and `scripts/gate.sh` does not look at it.

## Running it

```bash
./build.sh          # builds the server and every client whose tools are present
./demo.sh           # starts the server on a free port; all four clients change one list
```

`build.sh` says which clients it skipped and why. Both scripts take
`--only rust,java` to limit the clients. What each client needs:

- **Rust:** cargo. Cargo gets rexi from GitHub at the tag rsenml names.
- **Java:** Java 25 and Maven. Californium and EXIficient come from Maven Central.
- **C:** CMake, a C compiler and git. `scripts/fetch-third-party.sh` checks out
  exip and libcoap at their pins in `third-party.lock`, into
  `~/.cache/rsenml/examples/coap-todo`. CMake builds both as external projects.
- **C#:** the .NET 10 SDK (`dotnet` on `PATH`, in `$DOTNET_ROOT` or in
  `~/.dotnet`) and Subversion. Nagasena for .NET is only in OpenEXI's SourceForge
  Subversion repository, so `csharp-client/OpenEXI.Nagasena.csproj` builds it from
  source. The program finds .NET through `DOTNET_ROOT` (`demo.sh` sets it).

By hand:

```bash
target/release/coap-todo-server --port 5683 &          # --verbose prints request bodies in hex
target/release/todo-rust add "Buy milk"                 # prints the new id: 1
java -jar java-client/target/todo-java.jar done 1
build/c-client/todo-c list                              # 1 [x] Buy milk
build/csharp-client/bin/todo-csharp remove 1
```

Every client takes `--host` and `--port` (default `127.0.0.1:5683`) and one command:

| command | prints |
|---|---|
| `list` | one line per item, by id: `<id> [x] <title>` (done) or `<id> [ ] <title>` |
| `add <title>` | the new item's id |
| `done <id>` | nothing |
| `remove <id>` | nothing |

A client exits with status 1 when the server answers with an error (such as 4.04
for an unknown id) or does not answer. The Rust client also takes
`--format exi|json|cbor|xml` to use the server's other formats.

## The protocol

| request | body | answer |
|---|---|---|
| `GET /todos` | none | 2.05 Content, the list |
| `POST /todos` | a `title` record | 2.01 Created, `Location-Path: todos/<id>` |
| `PUT /todos/<id>` | a `done` record | 2.04 Changed, or 4.04 Not Found |
| `DELETE /todos/<id>` | none | 2.02 Deleted, or 4.04 Not Found |

All requests are Confirmable, and the answer comes back in the ACK
([RFC 7252 section 5.2.1](https://www.rfc-editor.org/rfc/rfc7252#section-5.2.1)).
The server remembers its last 256 answers. So if a request arrives twice (because
the client did not see the first answer), it is not applied twice
([section 4.5](https://www.rfc-editor.org/rfc/rfc7252#section-4.5)).

**Formats.** The server reads and writes all eight SenML CoAP Content-Formats
([RFC 8428 section 12.3](https://www.rfc-editor.org/rfc/rfc8428#section-12.3)): 110
`senml+json`, 112 `senml+cbor`, 114 `senml-exi`, 310 `senml+xml`, and their SenSML
twins 111, 113, 115 and 311. Content-Format names the format of the request body,
and Accept names the format of the answer
([RFC 7252 section 5.10](https://www.rfc-editor.org/rfc/rfc7252#section-5.10)).
Without Accept, the answer uses the body's format, or EXI when there is no body.
The clients send and ask for 114, `application/senml-exi`.

**EXI settings.** These follow RFC 8428
[section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8): schema-informed by
the RFC's XSD (`../../spec/senml.xsd`), strict, bit-packed, the EXI options in the
header with schemaId `"a"`, and no `$EXI` cookie. Each client sets these in its own
library's terms. For the same pack, all four write the same bytes (you can check
with `--verbose`). For example, `add "Same title"` gives the same 41-byte stream
from rsenml, EXIficient, exip and Nagasena for .NET.

### The SenML mapping

Every pack starts with the base name `urn:example:todo:`, so a record's full name is
`urn:example:todo:` + its `n`
([section 4.5.1](https://www.rfc-editor.org/rfc/rfc8428#section-4.5.1)).

The list (`GET /todos`), shown in SenML JSON:

```json
[
  {"bn": "urn:example:todo:", "n": "count", "v": 2},
  {"n": "1/title", "vs": "Buy milk"},
  {"n": "1/done", "vb": true},
  {"n": "2/title", "vs": "Walk the dog"},
  {"n": "2/done", "vb": false}
]
```

- `count` (a number) is the number of items. It also keeps an empty list a valid
  pack in every format, since the XML schema (and so EXI) needs at least one
  record.
- Each item is two records, because a SenML record carries one value: `<id>/title`
  with a string value (`vs`) and `<id>/done` with a boolean value (`vb`).
- Records have no time, so each one means "now"
  ([section 4.5.3](https://www.rfc-editor.org/rfc/rfc8428#section-4.5.3)).

The bodies of `POST` and `PUT` are one record each:

```json
[{"bn": "urn:example:todo:", "n": "title", "vs": "Buy milk"}]
[{"bn": "urn:example:todo:", "n": "done", "vb": true}]
```

The server looks for the record whose full name is `urn:example:todo:title` (or
`...:done`). It answers 4.00 Bad Request if there is no such record, or if its
value is the wrong kind.

## Schemas for the C and C# clients

Two clients cannot read the XSD as it is:

- exip loads a schema from the schema's own EXI form: `schemas/senml.xsd.exi`.
- Nagasena for .NET can read a compiled schema, but it has no schema compiler.
  `schemas/senml.xsc` is the SenML schema compiled by Java Nagasena
  (`EXISchemaFactory`, then `EXISchema.writeOut`). The C# port's
  `EXISchema.readIn` reads it back.

`scripts/gen-schemas.sh` writes both. It needs Java 25 and Maven, and takes its
tools from this example's pins (run `scripts/fetch-third-party.sh` first):

- `senml.xsd.exi` comes from EXIficient 1.0.7, through exip's `utils/xsd2exip`
  tool. Maven builds it on first use.
- `senml.xsc` comes from Java Nagasena. Its jars are in the
  [W3C EXI test suite](https://github.com/w3c/exi-testsuite)
  (`candidates/openexi/lib`). Set `NAGASENA_LIB` to use jars from somewhere else.

`scripts/gen-schemas.sh --check` compares the output with the committed files.

## Layout

| path | contents |
|---|---|
| `Cargo.toml` | the Rust workspace: `server/`, `rust-client/` |
| `java-client/` | Maven project, `target/todo-java.jar` (all dependencies inside) |
| `c-client/` | CMake project; libcoap and exip built as external projects |
| `csharp-client/` | `todo-csharp.csproj`, plus `OpenEXI.Nagasena.csproj` (from [rexi-interop](https://github.com/nhenehan/rexi-interop)), which builds Nagasena for .NET from the svn checkout |
| `schemas/` | compiled SenML schemas for exip and Nagasena for .NET (generated) |
| `third-party.lock`, `scripts/fetch-third-party.sh` | pins of exip, libcoap, nagasenacs and exi-testsuite; `--check`, `--update NAME`, `--print-dir` |
| `build.sh`, `demo.sh` | build what can be built; run the four-client demo |

## Notes

- The C# CoAP library (Com.AugustCellars.CoAP 1.10.0) depends on PeterO.Cbor 4.0.0,
  which has a known denial-of-service bug
  ([GHSA-6r92-cgxc-r5fg](https://github.com/advisories/GHSA-6r92-cgxc-r5fg)).
  `todo-csharp.csproj` asks for PeterO.Cbor 4.5.5, which fixes it.
- exip writes one extra byte when a stream ends exactly on a byte boundary (seen in
  [rexi-interop](https://github.com/nhenehan/rexi-interop)). Decoders ignore it.
  The C client zeroes its buffer, so the byte is always 0.
- The Java client turns off Californium's `Californium3.properties` file
  (`Configuration.createStandardWithoutFile`). Otherwise Californium would write it
  into the current directory.
