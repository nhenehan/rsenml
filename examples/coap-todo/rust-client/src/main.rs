//! The Rust todo client: rsenml for SenML EXI, coap-lite for CoAP.
//!
//! ```text
//! todo-rust [--host H] [--port N] [--format F] list | add <title> | done <id> | remove <id>
//! ```
//!
//! `list` prints one line per item, `<id> [x] <title>` or `<id> [ ] <title>`,
//! sorted by id. `add` prints the new item's id. The other commands print nothing.
//!
//! `--format` picks the SenML format of the bodies sent and asked for: `exi` (the
//! default, `application/senml-exi`), `json`, `cbor` or `xml`. The other clients
//! speak EXI only; this one can try the server's other formats.

use std::collections::BTreeMap;
use std::net::UdpSocket;
use std::process::ExitCode;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use coap_lite::{
    CoapOption, MessageClass, MessageType, Packet, RequestType as Method, ResponseType as Status,
};
use rsenml::{MediaType, Pack, Record, Value};

const BASE_NAME: &str = "urn:example:todo:";
static FORMAT: OnceLock<MediaType> = OnceLock::new();

/// The SenML format in use (`--format`), EXI unless set.
fn format() -> MediaType {
    *FORMAT.get().unwrap_or(&MediaType::SenmlExi)
}

/// How long to wait for an answer before sending again, and how many times to send
/// ([RFC 7252 section 4.8](https://www.rfc-editor.org/rfc/rfc7252#section-4.8):
/// ACK_TIMEOUT 2 s, MAX_RETRANSMIT 4, the wait doubling each time).
const ACK_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_RETRANSMIT: u32 = 4;

struct Answer {
    status: Status,
    location: Vec<String>,
    payload: Vec<u8>,
}

/// Sends one Confirmable request and waits for the piggybacked answer, the one
/// carried in the ACK ([RFC 7252 section 5.2.1](https://www.rfc-editor.org/rfc/rfc7252#section-5.2.1)).
fn request(
    addr: &str,
    method: Method,
    path: &[&str],
    body: Option<Vec<u8>>,
) -> Result<Answer, String> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
    socket.connect(addr).map_err(|e| format!("{addr}: {e}"))?;

    // A message id and token that differ from run to run.
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0) as u64;
    let mut packet = Packet::new();
    packet.header.set_version(1);
    packet.header.set_type(MessageType::Confirmable);
    packet.header.code = MessageClass::Request(method);
    packet.header.message_id = (seed ^ (seed >> 16)) as u16;
    packet.set_token(seed.to_be_bytes()[4..].to_vec());
    for segment in path {
        packet.add_option(CoapOption::UriPath, segment.as_bytes().to_vec());
    }
    let id = format().coap_content_format();
    if let Some(body) = body {
        packet.add_option(CoapOption::ContentFormat, uint_bytes(id));
        packet.payload = body;
    }
    packet.add_option(CoapOption::Accept, uint_bytes(id));
    let bytes = packet.to_bytes().map_err(|e| format!("{e:?}"))?;

    let mut wait = ACK_TIMEOUT;
    let mut buf = [0u8; 1500];
    for _ in 0..=MAX_RETRANSMIT {
        socket.send(&bytes).map_err(|e| e.to_string())?;
        socket
            .set_read_timeout(Some(wait))
            .map_err(|e| e.to_string())?;
        loop {
            let n = match socket.recv(&mut buf) {
                Ok(n) => n,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    break;
                }
                Err(e) => return Err(e.to_string()),
            };
            let Ok(answer) = Packet::from_bytes(&buf[..n]) else {
                continue;
            };
            if answer.header.message_id != packet.header.message_id
                || answer.get_token() != packet.get_token()
            {
                continue; // an answer to something else
            }
            let MessageClass::Response(status) = answer.header.code else {
                return Err("the server sent an empty answer".into());
            };
            let location = answer
                .get_option(CoapOption::LocationPath)
                .map(|l| {
                    l.iter()
                        .map(|s| String::from_utf8_lossy(s).into_owned())
                        .collect()
                })
                .unwrap_or_default();
            return Ok(Answer {
                status,
                location,
                payload: answer.payload,
            });
        }
        wait *= 2;
    }
    Err(format!("no answer from {addr}"))
}

/// An unsigned CoAP option value: big-endian, no leading zero bytes
/// ([RFC 7252 section 3.2](https://www.rfc-editor.org/rfc/rfc7252#section-3.2)).
fn uint_bytes(n: u16) -> Vec<u8> {
    let bytes = n.to_be_bytes();
    let skip = bytes.iter().take_while(|b| **b == 0).count();
    bytes[skip..].to_vec()
}

/// A one-record pack: `urn:example:todo:<field>` with the given value.
fn field_pack(field: &str, record: Record<'static>) -> Result<Vec<u8>, String> {
    let pack: Pack = [record
        .with_base_name(BASE_NAME)
        .with_name(field.to_string())]
    .into_iter()
    .collect();
    rsenml::encode(format(), &pack).map_err(|e| e.to_string())
}

fn expect(answer: &Answer, ok: Status) -> Result<(), String> {
    if answer.status == ok {
        Ok(())
    } else {
        Err(format!(
            "{:?}: {}",
            answer.status,
            String::from_utf8_lossy(&answer.payload)
        ))
    }
}

/// Prints the list. Item records are named `<id>/title` and `<id>/done` after the
/// base name; the `count` record is skipped.
fn print_list(payload: &[u8]) -> Result<(), String> {
    let pack = rsenml::decode(format(), payload).map_err(|e| e.to_string())?;
    let mut items: BTreeMap<u32, (String, bool)> = BTreeMap::new();
    for r in pack.resolve(Some(now())).map_err(|e| e.to_string())? {
        let Some(rest) = r.name.strip_prefix(BASE_NAME) else {
            continue;
        };
        let Some((id, field)) = rest.split_once('/') else {
            continue;
        };
        let Ok(id) = id.parse::<u32>() else { continue };
        let item = items.entry(id).or_default();
        match (field, r.value) {
            ("title", Some(Value::String(s))) => item.0 = s.into_owned(),
            ("done", Some(Value::Bool(b))) => item.1 = b,
            _ => {}
        }
    }
    for (id, (title, done)) in items {
        println!("{id} [{}] {title}", if done { 'x' } else { ' ' });
    }
    Ok(())
}

fn run(addr: &str, command: &[String]) -> Result<(), String> {
    match command {
        [c] if c == "list" => {
            let answer = request(addr, Method::Get, &["todos"], None)?;
            expect(&answer, Status::Content)?;
            print_list(&answer.payload)
        }
        [c, title] if c == "add" => {
            let body = field_pack("title", Record::new().with_string_value(title.clone()))?;
            let answer = request(addr, Method::Post, &["todos"], Some(body))?;
            expect(&answer, Status::Created)?;
            println!("{}", answer.location.last().cloned().unwrap_or_default());
            Ok(())
        }
        [c, id] if c == "done" => {
            let body = field_pack("done", Record::new().with_bool_value(true))?;
            let answer = request(addr, Method::Put, &["todos", id], Some(body))?;
            expect(&answer, Status::Changed)
        }
        [c, id] if c == "remove" => {
            let answer = request(addr, Method::Delete, &["todos", id], None)?;
            expect(&answer, Status::Deleted)
        }
        _ => Err(
            "usage: todo-rust [--host H] [--port N] [--format exi|json|cbor|xml] list | add <title> | done <id> | remove <id>"
                .into(),
        ),
    }
}

fn main() -> ExitCode {
    let mut host = String::from("127.0.0.1");
    let mut port = String::from("5683");
    let mut command = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--host" => host = args.next().unwrap_or_default(),
            "--port" => port = args.next().unwrap_or_default(),
            "--format" => {
                let media = match args.next().as_deref() {
                    Some("exi") => MediaType::SenmlExi,
                    Some("json") => MediaType::SenmlJson,
                    Some("cbor") => MediaType::SenmlCbor,
                    Some("xml") => MediaType::SenmlXml,
                    _ => {
                        eprintln!("todo-rust: --format is exi, json, cbor or xml");
                        return ExitCode::from(2);
                    }
                };
                let _ = FORMAT.set(media);
            }
            _ => command.push(arg),
        }
    }
    match run(&format!("{host}:{port}"), &command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("todo-rust: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Seconds since the Unix epoch. Resolving a pack needs a "now" to turn relative
/// times into absolute ones; these packs have no times, so any value works
/// ([RFC 8428 section 4.5.3](https://www.rfc-editor.org/rfc/rfc8428#section-4.5.3)).
fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}
