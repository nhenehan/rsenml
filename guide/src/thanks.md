# Thanks

SenML is a small standard that does one job well, and rsenml would not exist
without the people who wrote it and the libraries it stands on.

- **Cullen Jennings, Zach Shelby, Jari Arkko, Ari Keränen and Carsten Bormann**
  wrote [RFC 8428](https://www.rfc-editor.org/rfc/rfc8428). Its examples, hex
  dumps and schema are the first tests rsenml must pass.
- **Daniel Peintner** wrote [EXIficient](https://exificient.github.io/), with
  Richard Kuntschke at Siemens. rsenml's EXI bytes are checked against
  EXIficient's.
- **Toralf Wittner** wrote [minicbor](https://crates.io/crates/minicbor), which
  reads and writes rsenml's CBOR.
- **Yevhenii Reizner** wrote [xmlparser](https://crates.io/crates/xmlparser), and
  **Gronis** wrote [nanojson](https://crates.io/crates/nanojson): the small
  `no_std` parsers under rsenml's XML and JSON.

Thank you.
