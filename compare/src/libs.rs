//! One adapter per library and representation: decode to the normalized view,
//! encode from the library's own data model. Every call into a library goes
//! through [`guard`], so a panic is reported, not fatal.

use std::panic::{catch_unwind, AssertUnwindSafe};

use chrono::{DateTime, Utc};
use coapum_senml::{SenMLPack, SenMLRecord, SenMLValue};
use sindit_senml::{SenMLResolvedRecord, SenMLValueField};

use crate::model::{Got, NRaw, NRes, NVal, NOW};

/// The outcome of one call: Ok, a library error, or a panic.
#[derive(Debug, Clone)]
pub enum Call<T> {
    Ok(T),
    Err(String),
    Panic(String),
}

/// Runs `f`, catching panics (the default hook is silenced by `main`).
pub fn guard<T>(f: impl FnOnce() -> Result<T, String>) -> Call<T> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(v)) => Call::Ok(v),
        Ok(Err(e)) => Call::Err(e),
        Err(payload) => Call::Panic(
            payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic payload".into()),
        ),
    }
}

/// senml-cbor decodes into a caller-supplied array; this harness gives it 256.
pub const SENML_CBOR_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lib {
    Rsenml,
    Sindit,
    Coapum,
    SenmlCbor,
}

impl Lib {
    pub fn name(self) -> &'static str {
        match self {
            Lib::Rsenml => "rsenml",
            Lib::Sindit => "sindit-senml",
            Lib::Coapum => "coapum-senml",
            Lib::SenmlCbor => "senml-cbor",
        }
    }
}

pub const JSON_LIBS: [Lib; 3] = [Lib::Rsenml, Lib::Sindit, Lib::Coapum];
pub const CBOR_LIBS: [Lib; 3] = [Lib::Rsenml, Lib::SenmlCbor, Lib::Coapum];

// ---------------------------------------------------------------- rsenml

pub fn rsenml_raw(pack: &rsenml::Pack<'_>) -> Vec<NRaw> {
    pack.iter()
        .map(|r| NRaw {
            bn: r.base_name.as_deref().map(str::to_owned),
            bt: r.base_time,
            bu: r.base_unit.as_deref().map(str::to_owned),
            bv: r.base_value,
            bs: r.base_sum,
            bver: r.base_version,
            n: r.name.as_deref().map(str::to_owned),
            u: r.unit.as_deref().map(str::to_owned),
            val: r.value.as_ref().map(rsenml_val),
            s: r.sum,
            t: r.time,
            ut: r.update_time,
        })
        .collect()
}

fn rsenml_val(v: &rsenml::Value<'_>) -> NVal {
    match v {
        rsenml::Value::Number(x) => NVal::Num(*x),
        rsenml::Value::String(s) => NVal::Str(s.to_string()),
        rsenml::Value::Bool(b) => NVal::Bool(*b),
        rsenml::Value::Data(d) => NVal::Data(d.to_vec()),
    }
}

/// rsenml's own resolution (`Pack::resolve`), for checking it against the oracle.
pub fn rsenml_resolved(pack: &rsenml::Pack<'_>) -> Result<Vec<NRes>, String> {
    let resolved = pack
        .resolve(Some(NOW))
        .map_err(|e| format!("resolve: {e}"))?;
    Ok(resolved
        .into_iter()
        .map(|r| NRes {
            name: r.name,
            unit: r.unit.as_deref().map(str::to_owned),
            val: r.value.as_ref().map(rsenml_val),
            sum: r.sum,
            time: r.time,
            ut: r.update_time,
            bver: r.version,
        })
        .collect())
}

// ---------------------------------------------------------------- sindit-senml

fn sindit_now() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(NOW as i64, 0).unwrap_or_default()
}

pub fn sindit_decode(input: &[u8]) -> Result<Vec<SenMLResolvedRecord>, String> {
    let text = std::str::from_utf8(input).map_err(|e| format!("not UTF-8: {e}"))?;
    sindit_senml::parse_json(text, Some(sindit_now())).map_err(|e| format!("{e}"))
}

pub fn sindit_res(records: &[SenMLResolvedRecord]) -> Vec<NRes> {
    records
        .iter()
        .map(|r| NRes {
            name: r.name.clone(),
            unit: r.unit.clone(),
            val: r.value.as_ref().map(|v| match v {
                SenMLValueField::FloatingPoint(x) => NVal::Num(*x),
                SenMLValueField::StringValue(s) => NVal::Str(s.clone()),
                SenMLValueField::BooleanValue(b) => NVal::Bool(*b),
                SenMLValueField::DataValue(d) => NVal::Data(d.clone()),
            }),
            sum: r.sum,
            time: r.time.timestamp() as f64 + f64::from(r.time.timestamp_subsec_nanos()) / 1e9,
            ut: r.update_time,
            bver: r.base_version,
        })
        .collect()
}

pub fn sindit_encode(records: &[SenMLResolvedRecord]) -> Result<Vec<u8>, String> {
    serde_json::to_vec(records).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- coapum-senml

/// `from_json_bytes` followed by the crate's own `validate()`.
pub fn coapum_decode_json(input: &[u8]) -> Result<SenMLPack, String> {
    let pack = SenMLPack::from_json_bytes(input).map_err(|e| e.to_string())?;
    pack.validate().map_err(|e| format!("validate: {e}"))?;
    Ok(pack)
}

/// `from_cbor_validated`: ciborium serde decode, then `validate()`.
pub fn coapum_decode_cbor(input: &[u8]) -> Result<SenMLPack, String> {
    SenMLPack::from_cbor_validated(input).map_err(|e| e.to_string())
}

pub fn coapum_raw(pack: &SenMLPack) -> Vec<NRaw> {
    pack.records.iter().map(coapum_record).collect()
}

fn coapum_record(r: &SenMLRecord) -> NRaw {
    NRaw {
        n: r.n.clone(),
        u: r.u.clone(),
        // The crate's own reading of the value fields (it decodes `vd` itself).
        val: r.value().map(|v| match v {
            SenMLValue::Number(x) => NVal::Num(x),
            SenMLValue::String(s) => NVal::Str(s),
            SenMLValue::Boolean(b) => NVal::Bool(b),
            SenMLValue::Data(d) => NVal::Data(d),
        }),
        s: r.s,
        t: r.t,
        ut: r.ut,
        ..NRaw::default()
    }
}

pub fn coapum_encode_json(pack: &SenMLPack) -> Result<Vec<u8>, String> {
    pack.to_json_bytes().map_err(|e| e.to_string())
}

pub fn coapum_encode_cbor(pack: &SenMLPack) -> Result<Vec<u8>, String> {
    pack.to_cbor().map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- senml-cbor

pub fn senml_cbor_decode(input: &[u8]) -> Result<Vec<senml_cbor::Record<'_>>, String> {
    let mut buf = vec![senml_cbor::Record::empty(); SENML_CBOR_CAPACITY];
    let n = senml_cbor::cbor::decode(input, &mut buf).map_err(|e| e.to_string())?;
    buf.truncate(n);
    Ok(buf)
}

pub fn senml_cbor_raw(records: &[senml_cbor::Record<'_>]) -> Vec<NRaw> {
    records
        .iter()
        .map(|r| NRaw {
            bn: r.base_name.map(str::to_owned),
            bt: r.base_time,
            n: r.name.map(str::to_owned),
            u: r.unit.map(str::to_owned),
            val: r
                .value
                .map(NVal::Num)
                .or_else(|| r.string_value.map(|s| NVal::Str(s.to_owned())))
                .or_else(|| r.bool_value.map(NVal::Bool)),
            t: r.time,
            ..NRaw::default()
        })
        .collect()
}

/// senml-cbor records from rsenml's decoded pack: the 8 fields it models
/// (bn, bt, n, u, v, vs, vb, t); the others are dropped.
pub fn senml_cbor_from_rsenml<'a>(pack: &'a rsenml::Pack<'a>) -> Vec<senml_cbor::Record<'a>> {
    pack.iter()
        .map(|r| senml_cbor::Record {
            base_name: r.base_name.as_deref(),
            base_time: r.base_time,
            name: r.name.as_deref(),
            time: r.time,
            value: r.value.as_ref().and_then(rsenml::Value::as_number),
            string_value: match &r.value {
                Some(rsenml::Value::String(s)) => Some(s),
                _ => None,
            },
            bool_value: match &r.value {
                Some(rsenml::Value::Bool(b)) => Some(*b),
                _ => None,
            },
            unit: r.unit.as_deref(),
        })
        .collect()
}

pub fn senml_cbor_encode(records: &[senml_cbor::Record<'_>]) -> Result<Vec<u8>, String> {
    let mut out = vec![0u8; 64 * 1024];
    let n = senml_cbor::cbor::encode(records, &mut out).map_err(|e| e.to_string())?;
    out.truncate(n);
    Ok(out)
}

// ---------------------------------------------------------------- dispatch

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fmt {
    Json,
    Cbor,
    /// SenSML CBOR (indefinite-length array): only rsenml has an API for it.
    CborSensml,
}

/// Decodes with `lib` into the normalized view; `None` when `lib` has no API
/// for `fmt`.
pub fn decode(lib: Lib, fmt: Fmt, input: &[u8]) -> Option<Call<Got>> {
    let call = match (lib, fmt) {
        (Lib::Rsenml, Fmt::Json) => guard(|| {
            rsenml::json::decode(input)
                .map(|p| Got::Raw(rsenml_raw(&p)))
                .map_err(|e| e.to_string())
        }),
        (Lib::Rsenml, Fmt::Cbor) => guard(|| {
            rsenml::cbor::decode(input)
                .map(|p| Got::Raw(rsenml_raw(&p)))
                .map_err(|e| e.to_string())
        }),
        (Lib::Rsenml, Fmt::CborSensml) => guard(|| {
            rsenml::cbor::decode_sensml(input)
                .map(|p| Got::Raw(rsenml_raw(&p)))
                .map_err(|e| e.to_string())
        }),
        (Lib::Sindit, Fmt::Json) => {
            guard(|| sindit_decode(input).map(|r| Got::Resolved(sindit_res(&r))))
        }
        (Lib::Coapum, Fmt::Json) => {
            guard(|| coapum_decode_json(input).map(|p| Got::Raw(coapum_raw(&p))))
        }
        (Lib::Coapum, Fmt::Cbor) => {
            guard(|| coapum_decode_cbor(input).map(|p| Got::Raw(coapum_raw(&p))))
        }
        (Lib::SenmlCbor, Fmt::Cbor) => {
            guard(|| senml_cbor_decode(input).map(|r| Got::Raw(senml_cbor_raw(&r))))
        }
        _ => return None,
    };
    Some(call)
}

/// rsenml's resolution of the same input, when it decodes.
pub fn rsenml_resolve_input(fmt: Fmt, input: &[u8]) -> Call<Vec<NRes>> {
    guard(|| {
        let pack = match fmt {
            Fmt::Json => rsenml::json::decode(input).map_err(|e| e.to_string())?,
            Fmt::Cbor => rsenml::cbor::decode(input)
                .map_err(|e| e.to_string())?
                .into_owned(),
            Fmt::CborSensml => rsenml::cbor::decode_sensml(input)
                .map_err(|e| e.to_string())?
                .into_owned(),
        };
        rsenml_resolved(&pack)
    })
}

/// Encodes the RFC JSON example `json` with `lib`, starting from `lib`'s own
/// reading of it (senml-cbor, which reads no JSON, starts from rsenml's).
pub fn transcode(lib: Lib, fmt: Fmt, json: &[u8]) -> Call<Vec<u8>> {
    guard(|| match (lib, fmt) {
        (Lib::Rsenml, Fmt::Json) => rsenml::json::decode(json)
            .and_then(|p| rsenml::json::encode(&p))
            .map_err(|e| e.to_string()),
        (Lib::Rsenml, Fmt::Cbor) => rsenml::json::decode(json)
            .and_then(|p| rsenml::cbor::encode(&p))
            .map_err(|e| e.to_string()),
        (Lib::Sindit, Fmt::Json) => sindit_encode(&sindit_decode(json)?),
        (Lib::Coapum, Fmt::Json) => coapum_encode_json(&coapum_decode_json(json)?),
        (Lib::Coapum, Fmt::Cbor) => coapum_encode_cbor(&coapum_decode_json(json)?),
        (Lib::SenmlCbor, Fmt::Cbor) => {
            let pack = rsenml::json::decode(json).map_err(|e| format!("rsenml: {e}"))?;
            senml_cbor_encode(&senml_cbor_from_rsenml(&pack))
        }
        _ => Err("no encoder for this representation".into()),
    })
}
