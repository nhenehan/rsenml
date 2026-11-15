//! What decoding errors look like.

// ANCHOR: main
fn main() {
    let inputs: [(&str, &[u8]); 4] = [
        (
            "cut off",
            br#"[{"n":"urn:dev:mac:0024befffe804ff1:room","v":21"#,
        ),
        (
            "two values",
            br#"[{"n":"urn:dev:mac:0024befffe804ff1:room","v":21,"vb":true}]"#,
        ),
        ("bad name", br#"[{"n":"room temp","v":21}]"#),
        (
            "newer version",
            br#"[{"bver":11,"n":"urn:dev:mac:0024befffe804ff1:room","v":21}]"#,
        ),
    ];
    for (what, input) in inputs {
        match rsenml::json::decode(input) {
            Ok(pack) => println!("{what}: ok, {} records", pack.len()),
            Err(e) => {
                println!("{what}: {e}");
                println!("    {e:?}");
            }
        }
    }
}
// ANCHOR_END: main
