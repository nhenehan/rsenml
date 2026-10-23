//! Media types
//! ([RFC 8428 section 12.3](https://www.rfc-editor.org/rfc/rfc8428#section-12.3))
//! and CoAP Content-Formats
//! ([section 12.5](https://www.rfc-editor.org/rfc/rfc8428#section-12.5)).

use crate::error::Format;

/// The eight SenML and SenSML media types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaType {
    /// `application/senml+json`
    SenmlJson,
    /// `application/sensml+json`
    SensmlJson,
    /// `application/senml+cbor`
    SenmlCbor,
    /// `application/sensml+cbor`
    SensmlCbor,
    /// `application/senml+xml`
    SenmlXml,
    /// `application/sensml+xml`
    SensmlXml,
    /// `application/senml-exi`
    SenmlExi,
    /// `application/sensml-exi`
    SensmlExi,
}

impl MediaType {
    /// All eight. For each format, SenML comes before SenSML.
    pub const ALL: [MediaType; 8] = [
        MediaType::SenmlJson,
        MediaType::SensmlJson,
        MediaType::SenmlCbor,
        MediaType::SensmlCbor,
        MediaType::SenmlXml,
        MediaType::SensmlXml,
        MediaType::SenmlExi,
        MediaType::SensmlExi,
    ];

    /// The media type of a format. SenSML when `streaming` is true.
    pub const fn new(format: Format, streaming: bool) -> MediaType {
        match (format, streaming) {
            (Format::Json, false) => MediaType::SenmlJson,
            (Format::Json, true) => MediaType::SensmlJson,
            (Format::Cbor, false) => MediaType::SenmlCbor,
            (Format::Cbor, true) => MediaType::SensmlCbor,
            (Format::Xml, false) => MediaType::SenmlXml,
            (Format::Xml, true) => MediaType::SensmlXml,
            (Format::Exi, false) => MediaType::SenmlExi,
            (Format::Exi, true) => MediaType::SensmlExi,
        }
    }

    /// The media type name, for example `application/senml+json`.
    pub const fn content_type(self) -> &'static str {
        match self {
            MediaType::SenmlJson => "application/senml+json",
            MediaType::SensmlJson => "application/sensml+json",
            MediaType::SenmlCbor => "application/senml+cbor",
            MediaType::SensmlCbor => "application/sensml+cbor",
            MediaType::SenmlXml => "application/senml+xml",
            MediaType::SensmlXml => "application/sensml+xml",
            MediaType::SenmlExi => "application/senml-exi",
            MediaType::SensmlExi => "application/sensml-exi",
        }
    }

    /// The CoAP Content-Format ID (Table 8 in
    /// [section 12.5](https://www.rfc-editor.org/rfc/rfc8428#section-12.5)).
    pub const fn coap_content_format(self) -> u16 {
        match self {
            MediaType::SenmlJson => 110,
            MediaType::SensmlJson => 111,
            MediaType::SenmlCbor => 112,
            MediaType::SensmlCbor => 113,
            MediaType::SenmlExi => 114,
            MediaType::SensmlExi => 115,
            MediaType::SenmlXml => 310,
            MediaType::SensmlXml => 311,
        }
    }

    /// The format (JSON, CBOR, XML or EXI).
    pub const fn format(self) -> Format {
        match self {
            MediaType::SenmlJson | MediaType::SensmlJson => Format::Json,
            MediaType::SenmlCbor | MediaType::SensmlCbor => Format::Cbor,
            MediaType::SenmlXml | MediaType::SensmlXml => Format::Xml,
            MediaType::SenmlExi | MediaType::SensmlExi => Format::Exi,
        }
    }

    /// SenSML (streaming,
    /// [section 4.8](https://www.rfc-editor.org/rfc/rfc8428#section-4.8)) rather
    /// than SenML.
    pub const fn is_streaming(self) -> bool {
        matches!(
            self,
            MediaType::SensmlJson
                | MediaType::SensmlCbor
                | MediaType::SensmlXml
                | MediaType::SensmlExi
        )
    }

    /// Parses a content type, ignoring case and parameters (`; charset=...`).
    pub fn from_content_type(text: &str) -> Option<MediaType> {
        let essence = text.split(';').next().unwrap_or("").trim();
        MediaType::ALL
            .into_iter()
            .find(|m| m.content_type().eq_ignore_ascii_case(essence))
    }

    /// The media type with this CoAP Content-Format ID (Table 8 in
    /// [section 12.5](https://www.rfc-editor.org/rfc/rfc8428#section-12.5)).
    pub fn from_coap_content_format(id: u16) -> Option<MediaType> {
        MediaType::ALL
            .into_iter()
            .find(|m| m.coap_content_format() == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for m in MediaType::ALL {
            assert_eq!(MediaType::from_content_type(m.content_type()), Some(m));
            assert_eq!(
                MediaType::from_coap_content_format(m.coap_content_format()),
                Some(m)
            );
            assert_eq!(MediaType::new(m.format(), m.is_streaming()), m);
        }
        assert_eq!(
            MediaType::from_content_type("Application/SenML+JSON; charset=utf-8"),
            Some(MediaType::SenmlJson)
        );
        assert_eq!(MediaType::from_coap_content_format(50), None);
    }
}
