//! Section 1: every vector through every library that reads its representation.

use crate::libs::{self, Call, Fmt, Lib, CBOR_LIBS, JSON_LIBS};
use crate::model::{diff_raw, diff_res, resolve, Got};
use crate::vectors::{Expect, Vector};

#[derive(Debug, Clone)]
pub enum Outcome {
    Pass(String),
    Fail(String),
    Panic(String),
    /// The library has no API for this representation.
    NotApplicable,
}

pub struct Row {
    pub vector: Vector,
    /// One per library of the vector's representation (JSON_LIBS or CBOR_LIBS).
    pub outcomes: Vec<(Lib, Outcome)>,
}

pub fn libs_for(fmt: Fmt) -> [Lib; 3] {
    match fmt {
        Fmt::Json => JSON_LIBS,
        Fmt::Cbor | Fmt::CborSensml => CBOR_LIBS,
    }
}

pub fn run(vectors: &[Vector]) -> Vec<Row> {
    vectors
        .iter()
        .map(|v| Row {
            vector: v.clone(),
            outcomes: libs_for(v.fmt)
                .into_iter()
                .map(|lib| (lib, judge(v, lib)))
                .collect(),
        })
        .collect()
}

fn judge(v: &Vector, lib: Lib) -> Outcome {
    let Some(call) = libs::decode(lib, v.fmt, &v.input) else {
        return Outcome::NotApplicable;
    };
    match (call, &v.expect) {
        (Call::Panic(m), _) => Outcome::Panic(m),
        (Call::Err(e), Expect::Reject) => Outcome::Pass(format!("rejected: {e}")),
        (Call::Ok(_), Expect::Reject) => Outcome::Fail("accepted".into()),
        (Call::Err(e), Expect::Accept(_)) => Outcome::Fail(format!("rejected: {e}")),
        (Call::Ok(got), Expect::Accept(want)) => {
            let diff = match &got {
                Got::Raw(raw) => diff_raw(want, raw),
                // Resolved records are compared in time order (section 4.6).
                Got::Resolved(_) => diff_res(&resolve(want), &got.resolved()),
            };
            if let Some(d) = diff {
                return Outcome::Fail(format!("wrong data: {d}"));
            }
            // rsenml also exposes resolution: check it against the oracle.
            if lib == Lib::Rsenml {
                match libs::rsenml_resolve_input(v.fmt, &v.input) {
                    Call::Ok(res) => {
                        if let Some(d) = diff_res(&resolve(want), &res) {
                            return Outcome::Fail(format!("resolve: {d}"));
                        }
                    }
                    Call::Err(e) => return Outcome::Fail(format!("resolve failed: {e}")),
                    Call::Panic(m) => return Outcome::Panic(format!("resolve: {m}")),
                }
            }
            Outcome::Pass(String::new())
        }
    }
}

/// Pass / fail / panic counts for `lib` over vectors of the given representations.
pub fn tally(rows: &[Row], lib: Lib, fmts: &[Fmt]) -> (usize, usize, usize) {
    let mut t = (0, 0, 0);
    for row in rows.iter().filter(|r| fmts.contains(&r.vector.fmt)) {
        for (l, o) in &row.outcomes {
            if *l == lib {
                match o {
                    Outcome::Pass(_) => t.0 += 1,
                    Outcome::Fail(_) => t.1 += 1,
                    Outcome::Panic(_) => t.2 += 1,
                    Outcome::NotApplicable => {}
                }
            }
        }
    }
    t
}
