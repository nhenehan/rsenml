//! Writes every test case as SenML XML (`<dir>/<case>.xml`) and lists the cases in
//! `<dir>/cases.txt`, so that `scripts/gen-exi-fixtures.sh` can encode them with
//! EXIficient (another EXI implementation, whose output serves as the reference EXI
//! bytes for each case).

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(dir) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: fixture-xml <dir>");
        return ExitCode::from(2);
    };
    let mut names = String::new();
    for (name, pack) in rsenml_tests::all_cases() {
        let xml = match rsenml::xml::encode(&pack) {
            Ok(xml) => xml,
            Err(e) => {
                eprintln!("{name}: {e}");
                return ExitCode::FAILURE;
            }
        };
        if let Err(e) = std::fs::write(dir.join(format!("{name}.xml")), xml) {
            eprintln!("{name}: {e}");
            return ExitCode::FAILURE;
        }
        names.push_str(&name);
        names.push('\n');
    }
    match std::fs::write(dir.join("cases.txt"), names) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("cases.txt: {e}");
            ExitCode::FAILURE
        }
    }
}
