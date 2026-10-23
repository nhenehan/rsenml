//! A todo list served over CoAP, with SenML packs as the payloads.
//!
//! ```text
//! coap-todo-server [--port 5683] [--bind 127.0.0.1] [--verbose]
//! ```
//!
//! Resources (see the README for the payloads):
//!
//! - `GET /todos`: the whole list
//! - `POST /todos`: add an item; answers 2.01 Created with `Location-Path: todos/<id>`
//! - `PUT /todos/<id>`: mark an item done or not done
//! - `DELETE /todos/<id>`: remove an item
//!
//! The server reads and writes all eight SenML content formats
//! ([RFC 8428 section 12.3](https://www.rfc-editor.org/rfc/rfc8428#section-12.3)).
//! A request picks the format of its body with Content-Format and the format of
//! the answer with Accept
//! ([RFC 7252 section 5.10.4](https://www.rfc-editor.org/rfc/rfc7252#section-5.10.4)).
//! Without Accept, the answer uses the request's format, and EXI if the request
//! had no body.

use std::collections::{BTreeMap, VecDeque};
use std::net::{SocketAddr, UdpSocket};
use std::process::ExitCode;

use coap_lite::{
    CoapOption, MessageClass, MessageType, Packet, RequestType as Method, ResponseType as Status,
};
use rsenml::{MediaType, Pack, Record, Value};

/// Every name in a pack starts with this base name
/// ([RFC 8428 section 4.5.1](https://www.rfc-editor.org/rfc/rfc8428#section-4.5.1)).
const BASE_NAME: &str = "urn:example:todo:";

/// The format used when nothing else says which: `application/senml-exi`.
const DEFAULT_FORMAT: MediaType = MediaType::SenmlExi;

struct Item {
    title: String,
    done: bool,
}

struct Todos {
    items: BTreeMap<u32, Item>,
    next_id: u32,
}

/// What a request handler gives back: a status and, maybe, a pack to send.
struct Reply {
    status: Status,
    pack: Option<Pack<'static>>,
    location: Option<u32>,
    text: Option<String>,
}

impl Reply {
    fn status(status: Status) -> Self {
        Reply {
            status,
            pack: None,
            location: None,
            text: None,
        }
    }

    fn error(status: Status, text: impl Into<String>) -> Self {
        Reply {
            status,
            pack: None,
            location: None,
            text: Some(text.into()),
        }
    }
}

impl Todos {
    /// The whole list as one pack. The first record carries the base name and the
    /// number of items, so even an empty list has one record (the SenML XML and EXI
    /// forms need at least one, see the schema in RFC 8428 section 7). Then each
    /// item gives two records: `<id>/title` (`vs`) and `<id>/done` (`vb`).
    fn pack(&self) -> Pack<'static> {
        let mut pack = Pack::new();
        pack.push(
            Record::new()
                .with_base_name(BASE_NAME)
                .with_name("count")
                .with_value(self.items.len() as f64),
        );
        for (id, item) in &self.items {
            pack.push(
                Record::new()
                    .with_name(format!("{id}/title"))
                    .with_string_value(item.title.clone()),
            );
            pack.push(
                Record::new()
                    .with_name(format!("{id}/done"))
                    .with_bool_value(item.done),
            );
        }
        pack
    }

    fn handle(&mut self, method: &Method, path: &[String], body: Option<Pack<'_>>) -> Reply {
        let id = match path {
            [p] if p == "todos" => None,
            [p, id] if p == "todos" => match id.parse::<u32>() {
                Ok(id) => Some(id),
                Err(_) => return Reply::error(Status::NotFound, "no such item"),
            },
            _ => return Reply::error(Status::NotFound, "unknown resource"),
        };
        match (method, id) {
            (Method::Get, None) => Reply {
                pack: Some(self.pack()),
                ..Reply::status(Status::Content)
            },
            (Method::Post, None) => {
                let Some(title) = body
                    .as_ref()
                    .and_then(|b| field(b, "title"))
                    .and_then(as_string)
                else {
                    return Reply::error(
                        Status::BadRequest,
                        "need a urn:example:todo:title record with vs",
                    );
                };
                let id = self.next_id;
                self.next_id += 1;
                self.items.insert(id, Item { title, done: false });
                Reply {
                    location: Some(id),
                    ..Reply::status(Status::Created)
                }
            }
            (Method::Put, Some(id)) => {
                let Some(done) = body
                    .as_ref()
                    .and_then(|b| field(b, "done"))
                    .and_then(as_bool)
                else {
                    return Reply::error(
                        Status::BadRequest,
                        "need a urn:example:todo:done record with vb",
                    );
                };
                match self.items.get_mut(&id) {
                    Some(item) => {
                        item.done = done;
                        Reply::status(Status::Changed)
                    }
                    None => Reply::error(Status::NotFound, "no such item"),
                }
            }
            (Method::Delete, Some(id)) => match self.items.remove(&id) {
                Some(_) => Reply::status(Status::Deleted),
                None => Reply::error(Status::NotFound, "no such item"),
            },
            _ => Reply::status(Status::MethodNotAllowed),
        }
    }
}

/// The value of the record whose full name (base name + name, RFC 8428 section
/// 4.5.1) is `urn:example:todo:<field>`.
fn field(pack: &Pack<'_>, field: &str) -> Option<Value<'static>> {
    let records = pack.resolve(Some(now())).ok()?;
    records
        .into_iter()
        .find(|r| r.name.strip_prefix(BASE_NAME) == Some(field))
        .and_then(|r| r.value)
        .map(Value::into_owned)
}

fn as_string(v: Value<'_>) -> Option<String> {
    match v {
        Value::String(s) => Some(s.into_owned()),
        _ => None,
    }
}

fn as_bool(v: Value<'_>) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(b),
        _ => None,
    }
}

/// A CoAP option that holds an unsigned number: big-endian, with no leading zero
/// bytes ([RFC 7252 section 3.2](https://www.rfc-editor.org/rfc/rfc7252#section-3.2)).
fn uint_option(packet: &Packet, option: CoapOption) -> Option<u16> {
    let bytes = packet.get_first_option(option)?;
    Some(bytes.iter().fold(0u16, |n, b| (n << 8) | u16::from(*b)))
}

fn uint_bytes(n: u16) -> Vec<u8> {
    match n {
        0 => vec![],
        1..=255 => vec![n as u8],
        _ => n.to_be_bytes().to_vec(),
    }
}

/// Turns one request datagram into the answer datagram.
fn respond(todos: &mut Todos, request: &Packet, verbose: bool) -> Option<Packet> {
    let MessageClass::Request(method) = request.header.code else {
        return None; // a response or an empty message: nothing to answer
    };
    let mut response = coap_lite::CoapResponse::new(request)?.message;
    let path: Vec<String> = request
        .get_option(CoapOption::UriPath)
        .map(|segments| {
            segments
                .iter()
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .collect()
        })
        .unwrap_or_default();

    // The body's format comes from Content-Format, the answer's from Accept.
    let body_format = match uint_option(request, CoapOption::ContentFormat) {
        Some(id) => match MediaType::from_coap_content_format(id) {
            Some(m) => Some(m),
            None => {
                response.header.code = MessageClass::Response(Status::UnsupportedContentFormat);
                return Some(response);
            }
        },
        None => None,
    };
    let answer_format = match uint_option(request, CoapOption::Accept) {
        Some(id) => match MediaType::from_coap_content_format(id) {
            Some(m) => m,
            None => {
                response.header.code = MessageClass::Response(Status::NotAcceptable);
                return Some(response);
            }
        },
        None => body_format.unwrap_or(DEFAULT_FORMAT),
    };

    if verbose && !request.payload.is_empty() {
        let hex: String = request.payload.iter().map(|b| format!("{b:02x}")).collect();
        eprintln!("body ({} bytes): {hex}", request.payload.len());
    }
    let body = if request.payload.is_empty() {
        None
    } else {
        let format = body_format.unwrap_or(DEFAULT_FORMAT);
        match rsenml::decode(format, &request.payload) {
            Ok(pack) => Some(pack),
            Err(e) => {
                eprintln!("bad {} body: {e}", format.content_type());
                response.header.code = MessageClass::Response(Status::BadRequest);
                response.payload = format!("bad SenML: {e}").into_bytes();
                return Some(response);
            }
        }
    };

    let reply = todos.handle(&method, &path, body);
    eprintln!("{:?} /{} -> {:?}", method, path.join("/"), reply.status);
    response.header.code = MessageClass::Response(reply.status);
    if let Some(id) = reply.location {
        response.add_option(CoapOption::LocationPath, b"todos".to_vec());
        response.add_option(CoapOption::LocationPath, id.to_string().into_bytes());
    }
    if let Some(pack) = reply.pack {
        match rsenml::encode(answer_format, &pack) {
            Ok(bytes) => {
                response.add_option(
                    CoapOption::ContentFormat,
                    uint_bytes(answer_format.coap_content_format()),
                );
                response.payload = bytes;
            }
            Err(e) => {
                response.header.code = MessageClass::Response(Status::InternalServerError);
                response.payload = format!("cannot encode: {e}").into_bytes();
            }
        }
    } else if let Some(text) = reply.text {
        response.payload = text.into_bytes();
    }
    Some(response)
}

/// Answers already sent, kept so that a repeated Confirmable message (the client
/// did not see our answer and sent it again) gets the same answer and is not
/// applied twice ([RFC 7252 section 4.5](https://www.rfc-editor.org/rfc/rfc7252#section-4.5)).
struct Recent {
    sent: VecDeque<((SocketAddr, u16), Vec<u8>)>,
}

impl Recent {
    const KEEP: usize = 256;

    fn find(&self, key: (SocketAddr, u16)) -> Option<&[u8]> {
        self.sent
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, bytes)| bytes.as_slice())
    }

    fn remember(&mut self, key: (SocketAddr, u16), bytes: Vec<u8>) {
        if self.sent.len() == Self::KEEP {
            self.sent.pop_front();
        }
        self.sent.push_back((key, bytes));
    }
}

fn main() -> ExitCode {
    let mut port: u16 = 5683; // the default CoAP port (RFC 7252 section 6.1)
    let mut bind = String::from("127.0.0.1");
    let mut verbose = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--verbose" {
            verbose = true;
            continue;
        }
        match (arg.as_str(), args.next()) {
            ("--port", Some(p)) if p.parse::<u16>().is_ok() => port = p.parse().unwrap_or(port),
            ("--bind", Some(b)) => bind = b,
            _ => {
                eprintln!("usage: coap-todo-server [--port N] [--bind ADDRESS] [--verbose]");
                return ExitCode::from(2);
            }
        }
    }
    let socket = match UdpSocket::bind((bind.as_str(), port)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot listen on {bind}:{port}: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("coap-todo-server listening on coap://{bind}:{port}/todos");

    let mut todos = Todos {
        items: BTreeMap::new(),
        next_id: 1,
    };
    let mut recent = Recent {
        sent: VecDeque::new(),
    };
    let mut buf = [0u8; 1500];
    loop {
        let (n, peer) = match socket.recv_from(&mut buf) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("receive failed: {e}");
                continue;
            }
        };
        let Ok(request) = Packet::from_bytes(&buf[..n]) else {
            continue; // not a CoAP message: RFC 7252 section 4.2 says ignore it
        };
        let key = (peer, request.header.message_id);
        let confirmable = request.header.get_type() == MessageType::Confirmable;
        if confirmable && let Some(bytes) = recent.find(key) {
            let _ = socket.send_to(bytes, peer);
            continue;
        }
        let Some(response) = respond(&mut todos, &request, verbose) else {
            continue;
        };
        match response.to_bytes() {
            Ok(bytes) => {
                if let Err(e) = socket.send_to(&bytes, peer) {
                    eprintln!("send failed: {e}");
                }
                if confirmable {
                    recent.remember(key, bytes);
                }
            }
            Err(e) => eprintln!("cannot build the response: {e:?}"),
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
