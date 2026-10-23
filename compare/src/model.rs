//! The normalized view every library's output is compared in, and the oracle:
//! expected records written as JSON text (read with serde_json, independent of
//! every library under test) and an independent resolver (RFC 8428 section 4.6).

use base64::Engine as _;
use serde_json::Value as J;

/// "Now" for relative times (section 4.5.3), the same for every library.
pub const NOW: f64 = 1_700_000_000.0;

/// Times below this are relative (section 4.5.3).
const ABSOLUTE: f64 = 268_435_456.0;

#[derive(Debug, Clone, PartialEq)]
pub enum NVal {
    Num(f64),
    Str(String),
    Bool(bool),
    Data(Vec<u8>),
}

/// A record as written (base fields included); extensions are not compared.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NRaw {
    pub bn: Option<String>,
    pub bt: Option<f64>,
    pub bu: Option<String>,
    pub bv: Option<f64>,
    pub bs: Option<f64>,
    pub bver: Option<u64>,
    pub n: Option<String>,
    pub u: Option<String>,
    pub val: Option<NVal>,
    pub s: Option<f64>,
    pub t: Option<f64>,
    pub ut: Option<f64>,
}

impl NRaw {
    fn is_base_only(&self) -> bool {
        self.n.is_none()
            && self.u.is_none()
            && self.val.is_none()
            && self.s.is_none()
            && self.t.is_none()
            && self.ut.is_none()
    }
}

/// A resolved record (section 4.6).
#[derive(Debug, Clone, PartialEq)]
pub struct NRes {
    pub name: String,
    pub unit: Option<String>,
    pub val: Option<NVal>,
    pub sum: Option<f64>,
    pub time: f64,
    pub ut: Option<f64>,
    pub bver: Option<u64>,
}

/// What a decoder exposes: records as written, or only resolved records.
#[derive(Debug, Clone)]
pub enum Got {
    Raw(Vec<NRaw>),
    Resolved(Vec<NRes>),
}

impl Got {
    /// The resolved view: native for resolving libraries, else the oracle's.
    pub fn resolved(&self) -> Vec<NRes> {
        match self {
            Got::Raw(r) => resolve(r),
            Got::Resolved(r) => sorted(r.clone()),
        }
    }
}

pub fn b64url(text: &str) -> Option<Vec<u8>> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(text)
        .ok()
}

/// Expected records from JSON text: the 15 labels of Table 2, unknown labels
/// ignored. Panics on malformed oracle text (a bug in the harness).
pub fn expected(text: &str) -> Vec<NRaw> {
    let doc: J = serde_json::from_str(text).unwrap_or_else(|e| panic!("oracle JSON: {e}: {text}"));
    let J::Array(items) = doc else {
        panic!("oracle JSON is not an array: {text}")
    };
    items
        .iter()
        .map(|item| {
            let J::Object(map) = item else {
                panic!("oracle record is not an object: {text}")
            };
            let mut r = NRaw::default();
            let s = |v: &J| v.as_str().map(str::to_owned);
            let f = |v: &J| v.as_f64();
            for (k, v) in map {
                match k.as_str() {
                    "bn" => r.bn = s(v),
                    "bt" => r.bt = f(v),
                    "bu" => r.bu = s(v),
                    "bv" => r.bv = f(v),
                    "bs" => r.bs = f(v),
                    "bver" => r.bver = v.as_u64(),
                    "n" => r.n = s(v),
                    "u" => r.u = s(v),
                    "v" => r.val = f(v).map(NVal::Num),
                    "vs" => r.val = s(v).map(NVal::Str),
                    "vb" => r.val = v.as_bool().map(NVal::Bool),
                    "vd" => r.val = v.as_str().and_then(b64url).map(NVal::Data),
                    "s" => r.s = f(v),
                    "t" => r.t = f(v),
                    "ut" => r.ut = f(v),
                    _ => {}
                }
            }
            r
        })
        .collect()
}

/// The records as JSON text (serde_json's number formatting).
pub fn to_json(records: &[NRaw]) -> String {
    let items: Vec<J> = records
        .iter()
        .map(|r| {
            let mut m = serde_json::Map::new();
            let mut put = |k: &str, v: J| {
                m.insert(k.to_owned(), v);
            };
            if let Some(x) = &r.bn {
                put("bn", J::from(x.as_str()));
            }
            if let Some(x) = r.bt {
                put("bt", J::from(x));
            }
            if let Some(x) = &r.bu {
                put("bu", J::from(x.as_str()));
            }
            if let Some(x) = r.bv {
                put("bv", J::from(x));
            }
            if let Some(x) = r.bs {
                put("bs", J::from(x));
            }
            if let Some(x) = r.bver {
                put("bver", J::from(x));
            }
            if let Some(x) = &r.n {
                put("n", J::from(x.as_str()));
            }
            if let Some(x) = &r.u {
                put("u", J::from(x.as_str()));
            }
            match &r.val {
                Some(NVal::Num(x)) => put("v", J::from(*x)),
                Some(NVal::Str(x)) => put("vs", J::from(x.as_str())),
                Some(NVal::Bool(x)) => put("vb", J::from(*x)),
                Some(NVal::Data(x)) => put(
                    "vd",
                    J::from(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(x)),
                ),
                None => {}
            }
            if let Some(x) = r.s {
                put("s", J::from(x));
            }
            if let Some(x) = r.t {
                put("t", J::from(x));
            }
            if let Some(x) = r.ut {
                put("ut", J::from(x));
            }
            J::Object(m)
        })
        .collect();
    serde_json::to_string(&J::Array(items)).unwrap_or_default()
}

/// The oracle resolver (section 4.6), written from the RFC text alone:
/// - base fields apply to their record and every later one until replaced;
/// - name = bn + n; unit = u, else bu; time = bt + t, + NOW when below 2^28;
/// - value = bv + v for a numeric `v`; `vs`/`vb`/`vd` unchanged; a record with
///   no value field has no value (it must then have a sum);
/// - sum = bs + s when either is present (section 4.5.4);
/// - bver kept only when it is not 10; records of base fields only are dropped;
/// - the result is stably sorted by time ("chronological order").
pub fn resolve(raw: &[NRaw]) -> Vec<NRes> {
    let (mut bn, mut bt, mut bu, mut bv, mut bs, mut bver) = (
        None::<String>,
        None::<f64>,
        None::<String>,
        None::<f64>,
        None::<f64>,
        10u64,
    );
    let mut out = Vec::new();
    for r in raw {
        if r.bn.is_some() {
            bn.clone_from(&r.bn);
        }
        if r.bt.is_some() {
            bt = r.bt;
        }
        if r.bu.is_some() {
            bu.clone_from(&r.bu);
        }
        if r.bv.is_some() {
            bv = r.bv;
        }
        if r.bs.is_some() {
            bs = r.bs;
        }
        if let Some(v) = r.bver {
            bver = v;
        }
        if r.is_base_only() {
            continue;
        }
        let mut time = bt.unwrap_or(0.0) + r.t.unwrap_or(0.0);
        if time < ABSOLUTE {
            time += NOW;
        }
        out.push(NRes {
            name: format!(
                "{}{}",
                bn.as_deref().unwrap_or(""),
                r.n.as_deref().unwrap_or("")
            ),
            unit: r.u.clone().or_else(|| bu.clone()),
            val: r.val.clone().map(|v| match v {
                NVal::Num(x) => NVal::Num(bv.unwrap_or(0.0) + x),
                other => other,
            }),
            sum: match (bs, r.s) {
                (None, None) => None,
                (a, b) => Some(a.unwrap_or(0.0) + b.unwrap_or(0.0)),
            },
            time,
            ut: r.ut,
            bver: (bver != 10).then_some(bver),
        });
    }
    sorted(out)
}

fn sorted(mut v: Vec<NRes>) -> Vec<NRes> {
    v.sort_by(|a, b| a.time.total_cmp(&b.time));
    v
}

/// Resolved values may differ in the last bits (base addition, sindit-senml's
/// chrono round trip of times): equal within 1e-6 plus 4 ulps.
fn close(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-6 + 4.0 * f64::EPSILON * a.abs().max(b.abs())
}

fn close_opt(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => close(a, b),
        (a, b) => a.is_none() && b.is_none(),
    }
}

fn close_val(a: &Option<NVal>, b: &Option<NVal>) -> bool {
    match (a, b) {
        (Some(NVal::Num(a)), Some(NVal::Num(b))) => close(*a, *b),
        (a, b) => a == b,
    }
}

fn show(v: &Option<NVal>) -> String {
    match v {
        None => "none".into(),
        Some(NVal::Num(x)) => format!("v={x}"),
        Some(NVal::Str(x)) => format!("vs={x:?}"),
        Some(NVal::Bool(x)) => format!("vb={x}"),
        Some(NVal::Data(x)) => format!("vd={x:02x?}"),
    }
}

/// None when equal; else the first difference. Records as written compare
/// exactly (no arithmetic happened).
pub fn diff_raw(want: &[NRaw], got: &[NRaw]) -> Option<String> {
    if want.len() != got.len() {
        return Some(format!("{} records, want {}", got.len(), want.len()));
    }
    for (i, (w, g)) in want.iter().zip(got).enumerate() {
        let fields: [(&str, String, String); 12] = [
            ("bn", format!("{:?}", g.bn), format!("{:?}", w.bn)),
            ("bt", format!("{:?}", g.bt), format!("{:?}", w.bt)),
            ("bu", format!("{:?}", g.bu), format!("{:?}", w.bu)),
            ("bv", format!("{:?}", g.bv), format!("{:?}", w.bv)),
            ("bs", format!("{:?}", g.bs), format!("{:?}", w.bs)),
            ("bver", format!("{:?}", g.bver), format!("{:?}", w.bver)),
            ("n", format!("{:?}", g.n), format!("{:?}", w.n)),
            ("u", format!("{:?}", g.u), format!("{:?}", w.u)),
            ("value", show(&g.val), show(&w.val)),
            ("s", format!("{:?}", g.s), format!("{:?}", w.s)),
            ("t", format!("{:?}", g.t), format!("{:?}", w.t)),
            ("ut", format!("{:?}", g.ut), format!("{:?}", w.ut)),
        ];
        for (label, g, w) in fields {
            if g != w {
                return Some(format!("record {i}: {label} {g}, want {w}"));
            }
        }
    }
    None
}

/// None when equal (numbers within [`close`]); else the first difference.
pub fn diff_res(want: &[NRes], got: &[NRes]) -> Option<String> {
    if want.len() != got.len() {
        return Some(format!(
            "{} resolved records, want {}",
            got.len(),
            want.len()
        ));
    }
    for (i, (w, g)) in want.iter().zip(got).enumerate() {
        let d = if g.name != w.name {
            Some(format!("name {:?}, want {:?}", g.name, w.name))
        } else if g.unit != w.unit {
            Some(format!("unit {:?}, want {:?}", g.unit, w.unit))
        } else if !close_val(&g.val, &w.val) {
            Some(format!("{}, want {}", show(&g.val), show(&w.val)))
        } else if !close_opt(g.sum, w.sum) {
            Some(format!("sum {:?}, want {:?}", g.sum, w.sum))
        } else if !close(g.time, w.time) {
            Some(format!("time {}, want {}", g.time, w.time))
        } else if !close_opt(g.ut, w.ut) {
            Some(format!("ut {:?}, want {:?}", g.ut, w.ut))
        } else if g.bver != w.bver {
            Some(format!("bver {:?}, want {:?}", g.bver, w.bver))
        } else {
            None
        };
        if let Some(d) = d {
            return Some(format!("resolved record {i}: {d}"));
        }
    }
    None
}
