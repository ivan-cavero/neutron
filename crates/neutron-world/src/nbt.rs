//! Thin helpers over `ussr-nbt` for Anvil regions and `level.dat`.
//!
//! Copyright (c) 2026 Neutron Contributors — MIT License

use std::io::Cursor;

use ussr_nbt::mutf8::MString;
use ussr_nbt::owned::{Compound, List, Nbt, Tag};

use crate::error::{WorldError, WorldResult};

// Re-export ussr-nbt types for downstream use.
pub use ussr_nbt;

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// Parse a raw NBT byte slice into an `Nbt` value.
pub fn read_nbt(data: &[u8]) -> WorldResult<Nbt> {
    Nbt::read(&mut Cursor::new(data))
        .map_err(|e| WorldError::Nbt(format!("failed to parse NBT: {e}")))
}

/// Read NBT from a gzip-compressed byte slice (used by level.dat).
pub fn read_gzip_nbt(data: &[u8]) -> WorldResult<Nbt> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| WorldError::Nbt(format!("gzip decompression failed: {e}")))?;

    read_nbt(&decompressed)
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

/// Serialize an `Nbt` to a byte vector.
pub fn write_nbt(nbt: &Nbt) -> Vec<u8> {
    let mut buf = Vec::new();
    nbt.write(&mut buf).expect("NBT write should not fail");
    buf
}

/// Serialize an `Nbt` to gzip-compressed bytes (used by level.dat).
pub fn write_gzip_nbt(nbt: &Nbt) -> WorldResult<Vec<u8>> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let raw = write_nbt(nbt);
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&raw)
        .map_err(|e| WorldError::Nbt(format!("gzip compression failed: {e}")))?;
    encoder
        .finish()
        .map_err(|e| WorldError::Nbt(format!("gzip finish failed: {e}")))
}

// ---------------------------------------------------------------------------
// Compound helpers
// ---------------------------------------------------------------------------

/// Create a new empty NBT compound.
pub fn new_compound() -> Compound {
    Compound { tags: Vec::new() }
}

/// Convert a Rust `&str` to an `MString` (MUTF-8).
pub fn mstr(s: &str) -> MString {
    MString::from(s)
}

/// Create a root-level `Nbt` with an empty name (common for chunk data).
pub fn root_nbt(compound: Compound) -> Nbt {
    Nbt {
        name: MString::new(),
        compound,
    }
}

/// Insert a tag into a compound.
pub fn compound_insert(compound: &mut Compound, key: &str, tag: Tag) {
    compound.tags.push((MString::from(key), tag));
}

/// Get a tag from a compound by key name.
pub fn compound_get<'a>(compound: &'a Compound, key: &str) -> Option<&'a Tag> {
    let key_mstr = MString::from(key);
    compound
        .tags
        .iter()
        .find(|(name, _)| name == &key_mstr)
        .map(|(_, tag)| tag)
}

// ---------------------------------------------------------------------------
// Tag construction helpers
// ---------------------------------------------------------------------------

/// Create a `Tag::Byte`.
pub fn tag_byte(v: u8) -> Tag {
    Tag::Byte(v)
}

/// Create a `Tag::Short`.
pub fn tag_short(v: i16) -> Tag {
    Tag::Short(v)
}

/// Create a `Tag::Int`.
pub fn tag_int(v: i32) -> Tag {
    Tag::Int(v)
}

/// Create a `Tag::Long`.
pub fn tag_long(v: i64) -> Tag {
    Tag::Long(v)
}

/// Create a `Tag::Float`.
pub fn tag_float(v: f32) -> Tag {
    Tag::Float(v)
}

/// Create a `Tag::Double`.
pub fn tag_double(v: f64) -> Tag {
    Tag::Double(v)
}

/// Create a `Tag::String`.
pub fn tag_string(v: &str) -> Tag {
    Tag::String(MString::from(v))
}

/// Create a `Tag::ByteArray`.
pub fn tag_byte_array(v: Vec<u8>) -> Tag {
    Tag::ByteArray(v)
}

/// Create a `Tag::IntArray`.
pub fn tag_int_array(v: Vec<i32>) -> Tag {
    Tag::IntArray(v.into())
}

/// Create a `Tag::LongArray`.
pub fn tag_long_array(v: Vec<i64>) -> Tag {
    Tag::LongArray(v.into())
}

/// Create a `Tag::Compound`.
pub fn tag_compound(compound: Compound) -> Tag {
    Tag::Compound(compound)
}

/// Create a `Tag::List`.
pub fn tag_list(list: List) -> Tag {
    Tag::List(list)
}

// ---------------------------------------------------------------------------
// Compound access helpers (safe wrappers)
// ---------------------------------------------------------------------------

/// Get an `i32` from a compound, returning `WorldError::MissingField` if absent.
pub fn get_int(compound: &Compound, key: &str) -> WorldResult<i32> {
    match compound_get(compound, key) {
        Some(Tag::Int(v)) => Ok(*v),
        _ => Err(WorldError::MissingField {
            field: key.to_string(),
        }),
    }
}

/// Get an `i64` from a compound.
pub fn get_long(compound: &Compound, key: &str) -> WorldResult<i64> {
    match compound_get(compound, key) {
        Some(Tag::Long(v)) => Ok(*v),
        _ => Err(WorldError::MissingField {
            field: key.to_string(),
        }),
    }
}

/// Get a `String` from a compound.
pub fn get_string(compound: &Compound, key: &str) -> WorldResult<String> {
    match compound_get(compound, key) {
        Some(Tag::String(v)) => Ok(v.to_string()),
        _ => Err(WorldError::MissingField {
            field: key.to_string(),
        }),
    }
}

/// Get a reference to a nested compound from a compound.
pub fn get_compound<'a>(compound: &'a Compound, key: &str) -> WorldResult<&'a Compound> {
    match compound_get(compound, key) {
        Some(Tag::Compound(c)) => Ok(c),
        _ => Err(WorldError::MissingField {
            field: key.to_string(),
        }),
    }
}

/// Get a `u8` (byte) from a compound.
pub fn get_byte(compound: &Compound, key: &str) -> WorldResult<u8> {
    match compound_get(compound, key) {
        Some(Tag::Byte(v)) => Ok(*v),
        _ => Err(WorldError::MissingField {
            field: key.to_string(),
        }),
    }
}

/// Get an `i32` with a default value if the key is missing.
pub fn get_int_or(compound: &Compound, key: &str, default: i32) -> i32 {
    match compound_get(compound, key) {
        Some(Tag::Int(v)) => *v,
        _ => default,
    }
}

/// Get an `i64` with a default value if the key is missing.
pub fn get_long_or(compound: &Compound, key: &str, default: i64) -> i64 {
    match compound_get(compound, key) {
        Some(Tag::Long(v)) => *v,
        _ => default,
    }
}

/// Get a `u8` (byte) with a default value if the key is missing.
pub fn get_byte_or(compound: &Compound, key: &str, default: u8) -> u8 {
    match compound_get(compound, key) {
        Some(Tag::Byte(v)) => *v,
        _ => default,
    }
}

/// Get a `String` with a default value if the key is missing.
pub fn get_string_or(compound: &Compound, key: &str, default: &str) -> String {
    match compound_get(compound, key) {
        Some(Tag::String(v)) => v.to_string(),
        _ => default.to_string(),
    }
}

/// Get a `f32` with a default value if the key is missing.
pub fn get_float_or(compound: &Compound, key: &str, default: f32) -> f32 {
    match compound_get(compound, key) {
        Some(Tag::Float(v)) => *v,
        _ => default,
    }
}

/// Get a `f64` with a default value if the key is missing.
pub fn get_double_or(compound: &Compound, key: &str, default: f64) -> f64 {
    match compound_get(compound, key) {
        Some(Tag::Double(v)) => *v,
        _ => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_constructors() {
        assert!(matches!(tag_byte(1), Tag::Byte(1)));
        assert!(matches!(tag_short(2), Tag::Short(2)));
        assert!(matches!(tag_int(3), Tag::Int(3)));
        assert!(matches!(tag_long(4), Tag::Long(4)));
        assert!(matches!(tag_float(1.5), Tag::Float(1.5)));
        assert!(matches!(tag_double(2.5), Tag::Double(2.5)));
        assert!(matches!(tag_string("hi"), Tag::String(_)));
        assert!(matches!(tag_byte_array(vec![1, 2]), Tag::ByteArray(_)));
        assert!(matches!(tag_int_array(vec![1, 2]), Tag::IntArray(_)));
        assert!(matches!(tag_long_array(vec![1, 2]), Tag::LongArray(_)));
    }

    #[test]
    fn test_compound_insert_and_get() {
        let mut compound = new_compound();
        compound_insert(&mut compound, "health", tag_int(20));
        compound_insert(&mut compound, "name", tag_string("Steve"));

        assert_eq!(get_int(&compound, "health").unwrap(), 20);
        assert_eq!(get_string(&compound, "name").unwrap(), "Steve");
    }

    #[test]
    fn test_missing_field_error() {
        let compound = new_compound();
        let result = get_int(&compound, "nope");
        assert!(result.is_err());
    }

    #[test]
    fn test_defaults() {
        let compound = new_compound();
        assert_eq!(get_int_or(&compound, "x", 42), 42);
        assert_eq!(get_long_or(&compound, "x", 99), 99);
        assert_eq!(get_byte_or(&compound, "x", 7), 7);
        assert_eq!(get_string_or(&compound, "x", "default"), "default");
        assert!((get_float_or(&compound, "x", 1.0) - 1.0).abs() < f32::EPSILON);
        assert!((get_double_or(&compound, "x", 2.0) - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_root_nbt_roundtrip() {
        let mut compound = new_compound();
        compound_insert(&mut compound, "key", tag_int(42));
        let nbt = root_nbt(compound);

        let bytes = write_nbt(&nbt);
        let restored = read_nbt(&bytes).unwrap();
        assert_eq!(get_int(&restored.compound, "key").unwrap(), 42);
    }

    #[test]
    fn test_gzip_roundtrip() {
        let mut compound = new_compound();
        compound_insert(&mut compound, "test", tag_string("gzip data"));
        let nbt = root_nbt(compound);

        let compressed = write_gzip_nbt(&nbt).unwrap();
        let restored = read_gzip_nbt(&compressed).unwrap();
        assert_eq!(get_string(&restored.compound, "test").unwrap(), "gzip data");
    }

    #[test]
    fn test_nested_compound() {
        let mut inner = new_compound();
        compound_insert(&mut inner, "value", tag_int(99));

        let mut outer = new_compound();
        compound_insert(&mut outer, "inner", tag_compound(inner));
        let nbt = root_nbt(outer);

        let bytes = write_nbt(&nbt);
        let restored = read_nbt(&bytes).unwrap();
        let inner_compound = get_compound(&restored.compound, "inner").unwrap();
        assert_eq!(get_int(inner_compound, "value").unwrap(), 99);
    }
}
