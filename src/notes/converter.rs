use std::io::Read;

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use prost::Message;

#[derive(Clone, PartialEq, Message)]
struct NoteStoreProto {
    #[prost(message, optional, tag = "2")]
    document: Option<Document>,
}

#[derive(Clone, PartialEq, Message)]
struct Document {
    #[prost(int32, tag = "2")]
    version: i32,
    #[prost(message, optional, tag = "3")]
    note: Option<Note>,
}

#[derive(Clone, PartialEq, Message)]
struct Note {
    #[prost(string, tag = "2")]
    note_text: String,
}

pub fn decode_note_data(compressed_data: &[u8]) -> Result<String> {
    let mut decoder = GzDecoder::new(compressed_data);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .context("Failed to decompress Apple Notes data")?;

    let proto =
        NoteStoreProto::decode(decompressed.as_slice()).context("Failed to decode protobuf")?;

    let text = proto
        .document
        .and_then(|doc| doc.note)
        .map(|note| note.note_text)
        .unwrap_or_default();

    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    #[test]
    fn round_trip_note_data() {
        let note = NoteStoreProto {
            document: Some(Document {
                version: 1,
                note: Some(Note {
                    note_text: "Q: What is Rust?\nA: A systems programming language".into(),
                }),
            }),
        };

        let encoded = note.encode_to_vec();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&encoded).unwrap();
        let compressed = encoder.finish().unwrap();

        let result = decode_note_data(&compressed).unwrap();
        assert_eq!(
            result,
            "Q: What is Rust?\nA: A systems programming language"
        );
    }

    #[test]
    fn empty_note_returns_empty_string() {
        let note = NoteStoreProto { document: None };

        let encoded = note.encode_to_vec();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&encoded).unwrap();
        let compressed = encoder.finish().unwrap();

        let result = decode_note_data(&compressed).unwrap();
        assert!(result.is_empty());
    }
}
