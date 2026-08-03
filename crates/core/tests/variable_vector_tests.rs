use std::collections::HashMap;
use std::fs;
use std::path::Path;

use cadence_core::variables::{interpolate, parse, variable_names, Segment};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Vector {
    content: String,
    segments: Vec<Segment>,
    names: Vec<String>,
    interpolations: Vec<Interpolation>,
}

#[derive(Debug, Deserialize)]
struct Interpolation {
    values: HashMap<String, String>,
    output: String,
}

#[test]
fn rust_parser_matches_shared_vectors_byte_for_byte() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("variable-vectors.json");
    let bytes = fs::read(&path).expect("read shared variable vectors");
    let vectors: Vec<Vector> =
        serde_json::from_slice(&bytes).expect("parse shared variable vectors");

    for vector in vectors {
        let segments = parse(&vector.content);
        assert_eq!(
            segments, vector.segments,
            "segments for {:?}",
            vector.content
        );
        assert_eq!(
            variable_names(&segments),
            vector.names,
            "names for {:?}",
            vector.content
        );
        assert_eq!(
            segments.iter().map(Segment::raw).collect::<String>(),
            vector.content,
            "raw concatenation for {:?}",
            vector.content
        );
        for interpolation in vector.interpolations {
            assert_eq!(
                interpolate(&segments, &interpolation.values),
                interpolation.output,
                "interpolation for {:?} with {:?}",
                vector.content,
                interpolation.values
            );
        }
    }
}
