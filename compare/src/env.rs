//! Sections 5 and 6 and the machine description: dependency trees via
//! `cargo tree`, `no_std` from each crate's source, and `.text` sizes of the
//! thumbv7em footprint binaries read from their ELF section headers.

use std::path::{Path, PathBuf};
use std::process::Command;

fn compare_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(cmd)
        .args(args)
        .current_dir(compare_dir())
        .output()
        .map_err(|e| format!("{cmd}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "{cmd} {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .last()
                .unwrap_or("failed")
        ))
    }
}

pub struct Machine {
    pub arch: String,
    pub cpu: String,
    pub rustc: String,
    pub cargo: String,
}

pub fn machine() -> Machine {
    let cpu = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split_once(':'))
                .map(|(_, v)| v.trim().to_owned())
        })
        .unwrap_or_else(|| "unknown".into());
    let one = |c: &str, a: &[&str]| run(c, a).map(|s| s.trim().to_owned()).unwrap_or_else(|e| e);
    Machine {
        arch: one("uname", &["-m"]),
        cpu,
        rustc: one("rustc", &["-V"]),
        cargo: one("cargo", &["-V"]),
    }
}

pub struct DepRow {
    pub label: &'static str,
    pub probe: &'static str,
    /// Unique packages in the normal dependency tree, the library itself included.
    pub total: Result<usize, String>,
    /// Of which path (non-registry) packages.
    pub local: usize,
    pub no_std: String,
}

/// (label, probe package, crate source dir relative to compare/ or registry name)
const PROBES: [(&str, &str, &str); 8] = [
    (
        "rsenml, all features (default)",
        "probe-rsenml-all",
        "rsenml",
    ),
    ("rsenml, `json` only", "probe-rsenml-json", "rsenml-json"),
    ("rsenml, `cbor` only", "probe-rsenml-cbor", "rsenml-cbor"),
    ("rsenml, `xml` only", "probe-rsenml-xml", "rsenml-xml"),
    ("rsenml, `exi` only", "probe-rsenml-exi", "rsenml-exi"),
    (
        "sindit-senml 0.3 (default)",
        "probe-sindit-senml",
        "sindit-senml",
    ),
    (
        "senml-cbor 0.1 (no features)",
        "probe-senml-cbor",
        "senml-cbor",
    ),
    (
        "coapum-senml 0.1 (default: json, cbor)",
        "probe-coapum-senml",
        "coapum-senml",
    ),
];

pub fn deps() -> Vec<DepRow> {
    let metadata: Option<serde_json::Value> = run("cargo", &["metadata", "--format-version", "1"])
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());
    PROBES
        .iter()
        .map(|&(label, probe, krate)| {
            let tree = run(
                "cargo",
                &[
                    "tree", "-e", "normal", "-p", probe, "--prefix", "none", "--format", "{p}",
                ],
            );
            let (total, local) = match tree {
                Ok(text) => {
                    let mut pkgs: Vec<String> = text
                        .lines()
                        .map(|l| l.trim_end_matches(" (*)").trim().to_owned())
                        .filter(|l| !l.is_empty() && !l.starts_with("probe-"))
                        .collect();
                    pkgs.sort();
                    pkgs.dedup();
                    let local = pkgs.iter().filter(|p| p.contains(" (/")).count();
                    (Ok(pkgs.len()), local)
                }
                Err(e) => (Err(e), 0),
            };
            DepRow {
                label,
                probe,
                total,
                local,
                no_std: no_std(metadata.as_ref(), krate),
            }
        })
        .collect()
}

/// Reads the crate's `src/lib.rs`: `#![no_std]`, and whether it uses `alloc`.
fn no_std(metadata: Option<&serde_json::Value>, krate: &str) -> String {
    let manifest = metadata
        .and_then(|m| m["packages"].as_array())
        .and_then(|ps| ps.iter().find(|p| p["name"] == krate))
        .and_then(|p| p["manifest_path"].as_str())
        .map(PathBuf::from);
    let Some(dir) = manifest.as_deref().and_then(Path::parent) else {
        return "unknown".into();
    };
    let lib = std::fs::read_to_string(dir.join("src/lib.rs")).unwrap_or_default();
    let attr = lib.lines().any(|l| {
        let l = l.trim();
        l == "#![no_std]" || (l.starts_with("#![cfg_attr(") && l.contains("no_std"))
    });
    if !attr {
        return "no".into();
    }
    let uses_alloc = walk(&dir.join("src"))
        .iter()
        .any(|f| std::fs::read_to_string(f).is_ok_and(|s| s.contains("extern crate alloc")));
    if uses_alloc {
        "yes, needs `alloc`".into()
    } else {
        "yes, no `alloc`".into()
    }
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    out
}

pub const FOOTPRINT_TARGET: &str = "thumbv7em-none-eabihf";

pub struct FootprintRow {
    pub label: &'static str,
    pub what: &'static str,
    /// (.text, .rodata, .data, .bss) or why not.
    pub sections: Result<[u64; 4], String>,
}

const FOOTPRINTS: [(&str, &str, &str); 5] = [
    (
        "baseline",
        "footprint-baseline",
        "runtime + bump allocator, one allocation, no codec",
    ),
    (
        "rsenml `cbor`",
        "footprint-rsenml-cbor",
        "decode + validate + encode the RFC section 6 dump; bump allocator",
    ),
    (
        "rsenml `json`",
        "footprint-rsenml-json",
        "decode + validate + encode the 2nd example of 5.1.2; bump allocator",
    ),
    (
        "rsenml `exi`",
        "footprint-rsenml-exi",
        "decode + validate + encode the RFC section 8 bit-packed dump; bump allocator",
    ),
    (
        "senml-cbor",
        "footprint-senml-cbor",
        "decode + encode the RFC section 6 dump; no allocator (fixed arrays)",
    ),
];

pub fn footprints() -> Vec<FootprintRow> {
    FOOTPRINTS
        .iter()
        .map(|&(label, pkg, what)| {
            let sections = run(
                "cargo",
                &[
                    "build",
                    "-q",
                    "-p",
                    pkg,
                    "--target",
                    FOOTPRINT_TARGET,
                    "--profile",
                    "footprint",
                ],
            )
            .and_then(|_| {
                let elf = compare_dir()
                    .join("target")
                    .join(FOOTPRINT_TARGET)
                    .join("footprint")
                    .join(pkg);
                let bytes = std::fs::read(&elf).map_err(|e| format!("{}: {e}", elf.display()))?;
                elf32_sections(&bytes)
            });
            FootprintRow {
                label,
                what,
                sections,
            }
        })
        .collect()
}

/// Sizes of .text, .rodata, .data, .bss from a little-endian ELF32 file.
fn elf32_sections(b: &[u8]) -> Result<[u64; 4], String> {
    let u16_at = |o: usize| {
        b.get(o..o + 2)
            .map(|s| u16::from_le_bytes([s[0], s[1]]) as usize)
    };
    let u32_at = |o: usize| {
        b.get(o..o + 4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize)
    };
    if b.get(0..6) != Some(&[0x7f, b'E', b'L', b'F', 1, 1][..]) {
        return Err("not a little-endian ELF32 file".into());
    }
    let bad = || "truncated ELF".to_owned();
    let shoff = u32_at(0x20).ok_or_else(bad)?;
    let shentsize = u16_at(0x2e).ok_or_else(bad)?;
    let shnum = u16_at(0x30).ok_or_else(bad)?;
    let shstrndx = u16_at(0x32).ok_or_else(bad)?;
    let sh = |i: usize, field: usize| u32_at(shoff + i * shentsize + field).ok_or_else(bad);
    let strtab = sh(shstrndx, 16)?;
    let mut out = [0u64; 4];
    for i in 0..shnum {
        let name_off = strtab + sh(i, 0)?;
        let name: Vec<u8> = b
            .get(name_off..)
            .ok_or_else(bad)?
            .iter()
            .take_while(|&&c| c != 0)
            .copied()
            .collect();
        let size = sh(i, 20)? as u64;
        match name.as_slice() {
            b".text" => out[0] += size,
            b".rodata" => out[1] += size,
            b".data" => out[2] += size,
            b".bss" => out[3] += size,
            _ => {}
        }
    }
    Ok(out)
}
