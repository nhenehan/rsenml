//! Sections 3 and 4: encoded sizes and encode/decode times, per library and
//! representation, on the RFC 5.1.3 example and a 200-record synthetic pack.

use std::hint::black_box;
use std::time::{Duration, Instant};

use crate::libs::{self, Call};
use crate::model::{diff_res, expected, resolve, to_json, Got, NRaw, NRes, NVal};

pub struct Dataset {
    pub name: &'static str,
    /// JSON text every library starts from.
    pub json: String,
    pub want: Vec<NRes>,
}

/// 200 records in resolved form (no base fields, absolute times), so that every
/// library, including those without base fields, can carry it unchanged:
/// three numeric sensors with units and one boolean, times 30 s apart from
/// 1.7e9 (integers >= 2^16).
pub fn synthetic() -> Vec<NRaw> {
    let kinds = [("temp", "Cel"), ("hum", "%RH"), ("pres", "Pa")];
    (0..200)
        .map(|i: u32| {
            let t = 1_700_000_000.0 + f64::from(i) * 30.0;
            if i % 4 == 3 {
                NRaw {
                    n: Some("urn:dev:ow:10e2073a01080063:door".into()),
                    val: Some(NVal::Bool(i % 8 == 3)),
                    t: Some(t),
                    ..NRaw::default()
                }
            } else {
                let (n, u) = kinds[(i % 4) as usize];
                NRaw {
                    n: Some(format!("urn:dev:ow:10e2073a01080063:{n}")),
                    u: Some(u.into()),
                    val: Some(NVal::Num(f64::from((i * 37) % 1000) / 10.0)),
                    t: Some(t),
                    ..NRaw::default()
                }
            }
        })
        .collect()
}

pub fn datasets(rfc_513: &str) -> Vec<Dataset> {
    let synth = synthetic();
    vec![
        Dataset {
            name: "RFC 5.1.3",
            json: rfc_513.to_owned(),
            want: resolve(&expected(rfc_513)),
        },
        Dataset {
            name: "synthetic 200",
            json: to_json(&synth),
            want: resolve(&synth),
        },
    ]
}

/// One library/representation: how it encodes the dataset (from its own
/// reading of the JSON) and reads its own output back.
pub struct Codec {
    pub label: &'static str,
    pub encode_from_json: fn(&[u8]) -> Result<Vec<u8>, String>,
    pub decode: fn(&[u8]) -> Result<Got, String>,
    pub note: &'static str,
}

fn e<E: std::fmt::Display>(x: E) -> String {
    x.to_string()
}

fn exi_byte() -> rsenml::exi::EncodeOptions {
    rsenml::exi::EncodeOptions::new().with_alignment(rsenml::exi::Alignment::ByteAligned)
}

pub fn codecs() -> Vec<Codec> {
    vec![
        Codec {
            label: "rsenml JSON",
            encode_from_json: |j| {
                rsenml::json::decode(j)
                    .and_then(|p| rsenml::json::encode(&p))
                    .map_err(e)
            },
            decode: |b| {
                rsenml::json::decode(b)
                    .map(|p| Got::Raw(libs::rsenml_raw(&p)))
                    .map_err(e)
            },
            note: "",
        },
        Codec {
            label: "rsenml CBOR",
            encode_from_json: |j| {
                rsenml::json::decode(j)
                    .and_then(|p| rsenml::cbor::encode(&p))
                    .map_err(e)
            },
            decode: |b| {
                rsenml::cbor::decode(b)
                    .map(|p| Got::Raw(libs::rsenml_raw(&p)))
                    .map_err(e)
            },
            note: "",
        },
        Codec {
            label: "rsenml XML",
            encode_from_json: |j| {
                rsenml::json::decode(j)
                    .and_then(|p| rsenml::xml::encode(&p))
                    .map_err(e)
            },
            decode: |b| {
                rsenml::xml::decode(b)
                    .map(|p| Got::Raw(libs::rsenml_raw(&p)))
                    .map_err(e)
            },
            note: "",
        },
        Codec {
            label: "rsenml EXI bit-packed",
            encode_from_json: |j| {
                rsenml::json::decode(j)
                    .and_then(|p| rsenml::exi::encode(&p))
                    .map_err(e)
            },
            decode: |b| {
                rsenml::exi::decode(b)
                    .map(|p| Got::Raw(libs::rsenml_raw(&p)))
                    .map_err(e)
            },
            note: "options header with schemaId \"a\", no cookie",
        },
        Codec {
            label: "rsenml EXI byte-aligned",
            encode_from_json: |j| {
                rsenml::json::decode(j)
                    .and_then(|p| rsenml::exi::encode_with(&p, &exi_byte()))
                    .map_err(e)
            },
            decode: |b| {
                rsenml::exi::decode(b)
                    .map(|p| Got::Raw(libs::rsenml_raw(&p)))
                    .map_err(e)
            },
            note: "options header with schemaId \"a\", no cookie",
        },
        Codec {
            label: "sindit-senml JSON",
            encode_from_json: |j| libs::sindit_encode(&libs::sindit_decode(j)?),
            decode: |b| libs::sindit_decode(b).map(|r| Got::Resolved(libs::sindit_res(&r))),
            note: "writes resolved records only",
        },
        Codec {
            label: "coapum-senml JSON",
            encode_from_json: |j| libs::coapum_encode_json(&libs::coapum_decode_json(j)?),
            decode: |b| libs::coapum_decode_json(b).map(|p| Got::Raw(libs::coapum_raw(&p))),
            note: "no base fields in its model",
        },
        Codec {
            label: "coapum-senml CBOR",
            encode_from_json: |j| libs::coapum_encode_cbor(&libs::coapum_decode_json(j)?),
            decode: |b| libs::coapum_decode_cbor(b).map(|p| Got::Raw(libs::coapum_raw(&p))),
            note: "text keys: not RFC 8428 CBOR",
        },
        Codec {
            label: "senml-cbor CBOR",
            encode_from_json: |j| {
                let pack = rsenml::json::decode(j).map_err(|x| format!("rsenml: {x}"))?;
                libs::senml_cbor_encode(&libs::senml_cbor_from_rsenml(&pack))
            },
            decode: |b| libs::senml_cbor_decode(b).map(|r| Got::Raw(libs::senml_cbor_raw(&r))),
            note: "fed rsenml's reading of the JSON (it has no JSON reader); 8 of 15 labels",
        },
    ]
}

pub struct SizeRow {
    pub dataset: &'static str,
    pub codec: &'static str,
    pub note: &'static str,
    /// Encoded size, or why it could not encode.
    pub size: Result<usize, String>,
    /// None when its own decoder reads the output back to the expected
    /// resolved records; else the difference.
    pub faithful: Option<String>,
}

pub fn sizes(sets: &[Dataset]) -> Vec<SizeRow> {
    let mut out = Vec::new();
    for set in sets {
        for c in codecs() {
            let enc = libs::guard(|| (c.encode_from_json)(set.json.as_bytes()));
            let (size, faithful) = match enc {
                Call::Ok(bytes) => {
                    let back = libs::guard(|| (c.decode)(&bytes));
                    let faithful = match back {
                        Call::Ok(got) => diff_res(&set.want, &got.resolved()),
                        Call::Err(x) => Some(format!("own decoder rejects it: {x}")),
                        Call::Panic(x) => Some(format!("own decoder panics: {x}")),
                    };
                    (Ok(bytes.len()), faithful)
                }
                Call::Err(x) => (Err(x), None),
                Call::Panic(x) => (Err(format!("panic: {x}")), None),
            };
            out.push(SizeRow {
                dataset: set.name,
                codec: c.label,
                note: c.note,
                size,
                faithful,
            });
        }
    }
    out
}

/// Median time of one call, in nanoseconds: calls are batched so one sample
/// lasts at least 200 µs; 51 samples after warm-up.
pub fn median_ns(mut f: impl FnMut()) -> f64 {
    for _ in 0..20 {
        f();
    }
    let mut batch = 1u32;
    loop {
        let t = Instant::now();
        for _ in 0..batch {
            f();
        }
        if t.elapsed() >= Duration::from_micros(200) || batch >= 1 << 20 {
            break;
        }
        batch *= 2;
    }
    let mut samples: Vec<f64> = (0..51)
        .map(|_| {
            let t = Instant::now();
            for _ in 0..batch {
                f();
            }
            t.elapsed().as_nanos() as f64 / f64::from(batch)
        })
        .collect();
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

pub struct SpeedRow {
    pub dataset: &'static str,
    pub codec: &'static str,
    pub decode_ns: Option<f64>,
    pub encode_ns: Option<f64>,
    pub note: &'static str,
}

/// Each library decodes its own encoding of the dataset and encodes from its
/// own in-memory model. Decoders validate where the library offers it.
pub fn speeds(sets: &[Dataset]) -> Vec<SpeedRow> {
    let mut out = Vec::new();
    for set in sets {
        let json = set.json.as_bytes();
        let Ok(pack) = rsenml::json::decode(json) else {
            continue;
        };
        let own = |c: &str| {
            codecs()
                .into_iter()
                .find(|x| x.label == c)
                .and_then(|x| (x.encode_from_json)(json).ok())
                .unwrap_or_default()
        };
        let mut row = |codec, note, d: Option<f64>, en: Option<f64>| {
            out.push(SpeedRow {
                dataset: set.name,
                codec,
                decode_ns: d,
                encode_ns: en,
                note,
            });
        };

        let b = own("rsenml JSON");
        row(
            "rsenml JSON",
            "decode validates",
            Some(median_ns(|| {
                let _ = black_box(rsenml::json::decode(black_box(&b)));
            })),
            Some(median_ns(|| {
                let _ = black_box(rsenml::json::encode(black_box(&pack)));
            })),
        );
        row(
            "rsenml JSON + resolve",
            "decode, validate and resolve (what sindit-senml's parse_json does)",
            Some(median_ns(|| {
                let p = rsenml::json::decode(black_box(&b));
                let _ = black_box(p.map(|p| p.resolve(Some(crate::model::NOW)).map(|r| r.len())));
            })),
            None,
        );
        let b = own("rsenml CBOR");
        row(
            "rsenml CBOR",
            "decode validates",
            Some(median_ns(|| {
                let _ = black_box(rsenml::cbor::decode(black_box(&b)));
            })),
            Some(median_ns(|| {
                let _ = black_box(rsenml::cbor::encode(black_box(&pack)));
            })),
        );
        let b = own("rsenml XML");
        row(
            "rsenml XML",
            "decode validates",
            Some(median_ns(|| {
                let _ = black_box(rsenml::xml::decode(black_box(&b)));
            })),
            Some(median_ns(|| {
                let _ = black_box(rsenml::xml::encode(black_box(&pack)));
            })),
        );
        let b = own("rsenml EXI bit-packed");
        row(
            "rsenml EXI bit-packed",
            "decode validates",
            Some(median_ns(|| {
                let _ = black_box(rsenml::exi::decode(black_box(&b)));
            })),
            Some(median_ns(|| {
                let _ = black_box(rsenml::exi::encode(black_box(&pack)));
            })),
        );
        let b = own("rsenml EXI byte-aligned");
        let opts = exi_byte();
        row(
            "rsenml EXI byte-aligned",
            "decode validates",
            Some(median_ns(|| {
                let _ = black_box(rsenml::exi::decode(black_box(&b)));
            })),
            Some(median_ns(|| {
                let _ = black_box(rsenml::exi::encode_with(black_box(&pack), &opts));
            })),
        );

        let b = own("sindit-senml JSON");
        if let Ok(records) = libs::sindit_decode(&b) {
            row(
                "sindit-senml JSON",
                "parse_json: decode, validate, resolve",
                Some(median_ns(|| {
                    let _ = black_box(libs::sindit_decode(black_box(&b)));
                })),
                Some(median_ns(|| {
                    let _ = black_box(libs::sindit_encode(black_box(&records)));
                })),
            );
        }
        let b = own("coapum-senml JSON");
        if let Ok(p) = libs::coapum_decode_json(&b) {
            row(
                "coapum-senml JSON",
                "from_json_bytes + validate()",
                Some(median_ns(|| {
                    let _ = black_box(libs::coapum_decode_json(black_box(&b)));
                })),
                Some(median_ns(|| {
                    let _ = black_box(libs::coapum_encode_json(black_box(&p)));
                })),
            );
        }
        let b = own("coapum-senml CBOR");
        if let Ok(p) = libs::coapum_decode_cbor(&b) {
            row(
                "coapum-senml CBOR",
                "from_cbor_validated",
                Some(median_ns(|| {
                    let _ = black_box(libs::coapum_decode_cbor(black_box(&b)));
                })),
                Some(median_ns(|| {
                    let _ = black_box(libs::coapum_encode_cbor(black_box(&p)));
                })),
            );
        }
        let b = own("senml-cbor CBOR");
        let records = libs::senml_cbor_from_rsenml(&pack);
        let mut slots = vec![senml_cbor::Record::empty(); libs::SENML_CBOR_CAPACITY];
        let mut buf = vec![0u8; 64 * 1024];
        if senml_cbor::cbor::decode(&b, &mut slots).is_ok() {
            row(
                "senml-cbor CBOR",
                "no validation; caller-supplied record array and output buffer",
                Some(median_ns(|| {
                    let _ = black_box(senml_cbor::cbor::decode(black_box(&b), &mut slots));
                })),
                Some(median_ns(|| {
                    let _ = black_box(senml_cbor::cbor::encode(black_box(&records), &mut buf));
                })),
            );
        }
    }
    out
}
