//! Base64 with the URL-safe alphabet and no padding
//! ([RFC 4648 section 5](https://www.rfc-editor.org/rfc/rfc4648#section-5)). It is
//! the text form of the Data Value in JSON and XML (RFC 8428 sections
//! [5](https://www.rfc-editor.org/rfc/rfc8428#section-5) and
//! [7](https://www.rfc-editor.org/rfc/rfc8428#section-7)).
//!
//! Decoding is strict. It rejects padding, characters outside the alphabet, and
//! non-zero bits after the last octet
//! ([RFC 4648 section 3.5](https://www.rfc-editor.org/rfc/rfc4648#section-3.5)
//! allows this).

use alloc::string::String;
use alloc::vec::Vec;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Length of the encoding of `n` octets. Each full group of 3 octets gives 4
/// characters. A last group of 1 or 2 octets gives 2 or 3 characters, since there is
/// no padding.
pub const fn encoded_len(n: usize) -> usize {
    n / 3 * 4 + [0, 2, 3][n % 3]
}

/// Appends the encoding of `data` to `out`.
pub fn encode_into(data: &[u8], out: &mut String) {
    out.reserve(encoded_len(data.len()));
    // Writing to a String cannot fail.
    let _ = encode_to(data, out);
}

/// Writes the encoding of `data` to any `core::fmt::Write` (writing to a fixed
/// buffer may fail).
pub fn encode_to<W: core::fmt::Write>(data: &[u8], out: &mut W) -> core::fmt::Result {
    // Pack up to 3 octets into a 24-bit number (missing octets are zero), then
    // write it as 6-bit groups, high group first. A chunk of k octets needs k + 1
    // groups.
    for chunk in data.chunks(3) {
        let b = [
            chunk.first().copied().unwrap_or(0),
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..=chunk.len() {
            let sextet = (n >> (18 - 6 * i)) & 0x3f;
            out.write_char(char::from(ALPHABET[sextet as usize]))?;
        }
    }
    Ok(())
}

/// The encoding of `data`.
pub fn encode(data: &[u8]) -> String {
    let mut s = String::new();
    encode_into(data, &mut s);
    s
}

fn sextet(c: u8) -> Option<u32> {
    Some(u32::from(match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'-' => 62,
        b'_' => 63,
        _ => return None,
    }))
}

/// Decodes base64url without padding. Returns `None` if `text` is not exactly that.
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    // A last group of 1 character holds only 6 bits, less than one octet, so it can
    // never be valid.
    if bytes.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3 + 2);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for &c in chunk {
            n = (n << 6) | sextet(c)?;
        }
        match chunk.len() {
            4 => {
                out.extend_from_slice(&[(n >> 16) as u8, (n >> 8) as u8, n as u8]);
            }
            // 3 characters = 18 bits: 2 octets plus 2 spare bits, which must be zero.
            3 => {
                if n & 0x3 != 0 {
                    return None;
                }
                out.extend_from_slice(&[(n >> 10) as u8, (n >> 2) as u8]);
            }
            // 2 characters = 12 bits: 1 octet plus 4 spare bits, which must be zero.
            2 => {
                if n & 0xf != 0 {
                    return None;
                }
                out.push((n >> 4) as u8);
            }
            _ => return None,
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc4648_vectors_without_padding() {
        let vectors: [(&[u8], &str); 7] = [
            (b"", ""),
            (b"f", "Zg"),
            (b"fo", "Zm8"),
            (b"foo", "Zm9v"),
            (b"foob", "Zm9vYg"),
            (b"fooba", "Zm9vYmE"),
            (b"foobar", "Zm9vYmFy"),
        ];
        for (data, text) in vectors {
            assert_eq!(encode(data), text);
            assert_eq!(decode(text).as_deref(), Some(data));
            assert_eq!(encoded_len(data.len()), text.len());
        }
    }

    #[test]
    fn rfc8428_example_and_url_alphabet() {
        // RFC 8428 section 5.1.5: "aGkgCg" is "hi \n".
        assert_eq!(decode("aGkgCg").as_deref(), Some(&b"hi \n"[..]));
        assert_eq!(encode(&[0xfb, 0xff]), "-_8");
        assert_eq!(decode("-_8").as_deref(), Some(&[0xfb, 0xff][..]));
    }

    #[test]
    fn rejects_padding_standard_alphabet_and_stray_bits() {
        assert_eq!(decode("Zg=="), None);
        assert_eq!(decode("+/8"), None);
        assert_eq!(decode("Zh"), None);
        assert_eq!(decode("Z"), None);
    }
}
