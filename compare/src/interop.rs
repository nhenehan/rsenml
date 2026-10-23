//! Section 2: each RFC JSON example encoded by each library, decoded by each.

use crate::libs::{self, Call, Fmt, Lib};
use crate::model::{diff_res, expected, resolve};
use crate::rfc::JsonExample;

#[derive(Debug, Clone)]
pub enum Cell {
    Ok,
    /// The encoder could not produce this example.
    NoInput(String),
    Rejected(String),
    WrongData(String),
    Panic(String),
}

pub struct Matrix {
    pub fmt: Fmt,
    pub libs: [Lib; 3],
    /// (example, encoder, decoder, cell)
    pub cells: Vec<(String, Lib, Lib, Cell)>,
    /// Encoded size per (example, encoder), when it encoded.
    pub sizes: Vec<(String, Lib, usize)>,
}

pub fn run(fmt: Fmt, libs: [Lib; 3], examples: &[JsonExample]) -> Matrix {
    let mut m = Matrix {
        fmt,
        libs,
        cells: Vec::new(),
        sizes: Vec::new(),
    };
    for ex in examples {
        let want = resolve(&expected(&ex.json));
        for enc in libs {
            let encoded = libs::transcode(enc, fmt, ex.json.as_bytes());
            for dec in libs {
                let cell = match &encoded {
                    Call::Err(e) => Cell::NoInput(e.clone()),
                    Call::Panic(p) => Cell::NoInput(format!("encoder panicked: {p}")),
                    Call::Ok(bytes) => match libs::decode(dec, fmt, bytes) {
                        None => Cell::NoInput("no decoder".into()),
                        Some(Call::Panic(p)) => Cell::Panic(p),
                        Some(Call::Err(e)) => Cell::Rejected(e),
                        Some(Call::Ok(got)) => match diff_res(&want, &got.resolved()) {
                            None => Cell::Ok,
                            Some(d) => Cell::WrongData(d),
                        },
                    },
                };
                m.cells.push((ex.name.clone(), enc, dec, cell));
            }
            if let Call::Ok(bytes) = &encoded {
                m.sizes.push((ex.name.clone(), enc, bytes.len()));
            }
        }
    }
    m
}
