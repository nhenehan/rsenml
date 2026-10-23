//! Writes a seed corpus for every fuzz target: each XML fixture of
//! `crates/rsenml-tests/fixtures/exificient`, encoded in all four
//! representations, plus the EXIficient EXI streams as they are.
//!
//!   make_seeds <fixtures dir> <corpus dir>

use std::{env, fs, path::Path, process::ExitCode};

use rsenml::MediaType;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let [_, fixtures, corpus] = args.as_slice() else {
        eprintln!("usage: make_seeds <fixtures dir> <corpus dir>");
        return ExitCode::from(2);
    };
    match run(Path::new(fixtures), Path::new(corpus)) {
        Ok(count) => {
            println!("{count} seeds in {corpus}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("make_seeds: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(fixtures: &Path, corpus: &Path) -> Result<usize, String> {
    let targets = ["decode_json", "decode_cbor", "decode_xml", "decode_exi"];
    for target in targets.iter().chain(["stream_decoders"].iter()) {
        fs::create_dir_all(corpus.join(target)).map_err(|e| e.to_string())?;
    }
    let mut entries: Vec<_> = fs::read_dir(fixtures)
        .map_err(|e| format!("{}: {e}", fixtures.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    entries.sort();
    let mut count = 0;
    for path in entries {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if name.ends_with(".exi") {
            write(&corpus.join("decode_exi").join(&name), &bytes)?;
            count += 1;
            continue;
        }
        if !name.ends_with(".xml") {
            continue;
        }
        let pack =
            rsenml::decode(MediaType::SenmlXml, &bytes).map_err(|e| format!("{name}: {e:?}"))?;
        let stem = name.trim_end_matches(".xml");
        for (target, media) in targets.iter().zip([
            MediaType::SenmlJson,
            MediaType::SenmlCbor,
            MediaType::SenmlXml,
            MediaType::SenmlExi,
        ]) {
            let encoded = rsenml::encode(media, &pack).map_err(|e| format!("{name}: {e:?}"))?;
            write(&corpus.join(target).join(stem), &encoded)?;
            // The stream target reads its first byte as the piece size.
            let mut stream = vec![7u8];
            stream.extend_from_slice(&encoded);
            write(
                &corpus
                    .join("stream_decoders")
                    .join(format!("{stem}.{target}")),
                &stream,
            )?;
            count += 2;
        }
    }
    Ok(count)
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}
