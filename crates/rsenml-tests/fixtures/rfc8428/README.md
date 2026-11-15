# RFC 8428 excerpts

Short parts of RFC 8428 that the tests read: its examples and the XML schema of
section 8. Each file is quoted as printed, with only the page breaks and the
RFC's left margin taken out.

Cullen Jennings, Zach Shelby, Jari Arkko, Ari Keränen, and Carsten Bormann. 2018.
Sensor Measurement Lists (SenML). RFC 8428. RFC Editor.
https://doi.org/10.17487/RFC8428

| File | Quoted from | What it is |
|---|---|---|
| `5.1.1.json` | [Section 5.1.1](https://www.rfc-editor.org/rfc/rfc8428#section-5.1.1) | Single data point |
| `5.1.2a.json` | [Section 5.1.2](https://www.rfc-editor.org/rfc/rfc8428#section-5.1.2) | Multiple data points, first example |
| `5.1.2b.json` | [Section 5.1.2](https://www.rfc-editor.org/rfc/rfc8428#section-5.1.2) | Multiple data points with a base time |
| `5.1.2c.txt` | [Section 5.1.2](https://www.rfc-editor.org/rfc/rfc8428#section-5.1.2) | SenSML stream, left unfinished (`...`) by the RFC |
| `5.1.3.json` | [Section 5.1.3](https://www.rfc-editor.org/rfc/rfc8428#section-5.1.3) | Multiple measurements |
| `5.1.4.json` | [Section 5.1.4](https://www.rfc-editor.org/rfc/rfc8428#section-5.1.4) | Section 5.1.3 resolved |
| `5.1.5.json` | [Section 5.1.5](https://www.rfc-editor.org/rfc/rfc8428#section-5.1.5) | Multiple data types |
| `5.1.6.json` | [Section 5.1.6](https://www.rfc-editor.org/rfc/rfc8428#section-5.1.6) | Collection of resources |
| `5.1.7a.json`, `5.1.7b.json`, `5.1.7c.json` | [Section 5.1.7](https://www.rfc-editor.org/rfc/rfc8428#section-5.1.7) | Setting an actuator |
| `6.hex` | [Section 6](https://www.rfc-editor.org/rfc/rfc8428#section-6) | CBOR hex dump of section 5.1.2's second example |
| `7.xml` | [Section 7](https://www.rfc-editor.org/rfc/rfc8428#section-7) | XML of the same example |
| `8a.xml`, `8a.hex` | [Section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8) | XML example and its EXI (bit-packed) hex dump |
| `8b.xml`, `8b.hex` | [Section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8) | XML example and its EXI (byte-aligned) hex dump |
| `senml.xsd` | [Section 8](https://www.rfc-editor.org/rfc/rfc8428#section-8) | The SenML XML schema |

`tests/extract.rs` checks that every file is still exactly what the RFC says. It
cuts each excerpt out of the full text again (`scripts/fetch-standards.sh`
downloads it, checked by sha256) and compares. `RSENML_WRITE_EXCERPTS=1` writes
the files again.

## License

These excerpts are not covered by rsenml's license. Copyright (c) 2018 IETF Trust
and the persons identified as the document authors. All rights reserved. RFC 8428
is subject to BCP 78 and the IETF Trust's Legal Provisions Relating to IETF
Documents (https://trustee.ietf.org/license-info).

The schema (`senml.xsd`), the JSON examples and the CBOR dump are Code Components:

> Copyright (c) 2018 IETF Trust and the persons identified as authors of the code.
> All rights reserved. Redistribution and use in source and binary forms, with or
> without modification, is permitted pursuant to, and subject to the license terms
> contained in, the Revised BSD License set forth in Section 4.c of the IETF
> Trust's Legal Provisions Relating to IETF Documents
> (https://trustee.ietf.org/license-info).
