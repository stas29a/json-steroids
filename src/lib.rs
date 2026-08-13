//! # json-steroids
//!
//! A high-performance, zero-copy JSON parsing library for Rust.
//!
//! ## Features
//! - Zero-copy string parsing where possible
//! - Efficient SIMD-friendly parsing (when available)
//! - Derive macros for automatic serialization/deserialization
//! - Minimal allocations
//!
//! ## Example
//!
//! ```rust,ignore
//! use json_steroids::{Json, JsonSerialize, JsonDeserialize, to_string, from_str};
//!
//! #[derive(Debug, Json, PartialEq)]
//! struct Person {
//!     name: String,
//!     age: u32,
//! }
//!
//! let person = Person { name: "Alice".to_string(), age: 30 };
//! let json = to_string(&person);
//! let parsed: Person = from_str(&json).unwrap();
//! assert_eq!(person, parsed);
//! ```

mod error;
mod parser;
mod traits;
mod value;
pub mod writer;

#[cfg(feature = "simd")]
pub mod simd;

#[cfg(feature = "arrayvec")]
pub mod arrayvec;

pub use error::{JsonError, Result};
pub use parser::JsonParser;
pub use traits::{JsonDeserialize, JsonSerialize};
pub use value::JsonValue;
pub use writer::JsonWriter;

// Re-export derive macros
pub use json_steroids_derive::{Json, JsonDeserialize, JsonSerialize};

/// Serialize a value to a JSON string
#[inline]
pub fn to_string<T: JsonSerialize>(value: &T) -> String {
    let mut writer = JsonWriter::new();
    value.json_serialize(&mut writer);
    writer.into_string()
}

/// Serialize a value to a JSON string with pretty printing
#[inline]
pub fn to_string_pretty<T: JsonSerialize>(value: &T) -> String {
    let mut writer = JsonWriter::with_indent(2);
    value.json_serialize(&mut writer);
    writer.into_string()
}

/// Deserialize a value from a JSON string
#[inline]
pub fn from_str<'de, T: JsonDeserialize<'de>>(s: &'de str) -> Result<T> {
    let mut parser = JsonParser::new(s);
    T::json_deserialize(&mut parser)
}

/// Deserialize a value from JSON bytes
#[inline]
pub fn from_bytes<'de, T: JsonDeserialize<'de>>(bytes: &'de [u8]) -> Result<T> {
    let s = std::str::from_utf8(bytes).map_err(|_| JsonError::InvalidUtf8)?;
    from_str(s)
}

/// Parse JSON into a dynamic value
#[inline]
pub fn parse(s: &str) -> Result<JsonValue> {
    let mut parser = JsonParser::new(s);
    parser.parse_value()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Json)]
    struct SimpleStruct {
        name: String,
        value: i64,
        active: bool,
    }

    #[derive(Debug, PartialEq, Json)]
    struct NestedStruct {
        id: u32,
        data: SimpleStruct,
    }

    #[derive(Debug, PartialEq, Json)]
    struct WithOption {
        required: String,
        optional: Option<i32>,
    }

    #[derive(Debug, PartialEq, Json)]
    struct WithVec {
        items: Vec<i32>,
    }

    #[derive(Debug, PartialEq, Json)]
    enum Status {
        Active,
        Inactive,
        Pending,
    }

    #[derive(Debug, PartialEq, Json)]
    enum Message {
        Text(String),
        Number(i64),
        Data { x: i32, y: i32 },
    }

    #[test]
    fn test_simple_struct_roundtrip() {
        let original = SimpleStruct {
            name: "test".to_string(),
            value: 42,
            active: true,
        };
        let json = to_string(&original);
        let parsed: SimpleStruct = from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_nested_struct_roundtrip() {
        let original = NestedStruct {
            id: 1,
            data: SimpleStruct {
                name: "nested".to_string(),
                value: 100,
                active: false,
            },
        };
        let json = to_string(&original);
        let parsed: NestedStruct = from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_option_some() {
        let original = WithOption {
            required: "hello".to_string(),
            optional: Some(42),
        };
        let json = to_string(&original);
        let parsed: WithOption = from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_option_none() {
        let original = WithOption {
            required: "hello".to_string(),
            optional: None,
        };
        let json = to_string(&original);
        let parsed: WithOption = from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_vec_roundtrip() {
        let original = WithVec {
            items: vec![1, 2, 3, 4, 5],
        };
        let json = to_string(&original);
        let parsed: WithVec = from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_unit_enum() {
        let original = Status::Active;
        let json = to_string(&original);
        assert_eq!(json, r#""Active""#);
        let parsed: Status = from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_tuple_enum() {
        let original = Message::Text("hello".to_string());
        let json = to_string(&original);
        let parsed: Message = from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_struct_enum() {
        let original = Message::Data { x: 10, y: 20 };
        let json = to_string(&original);
        let parsed: Message = from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_primitives() {
        assert_eq!(to_string(&42i32), "42");
        assert_eq!(to_string(&3.14f64), "3.14");
        assert_eq!(to_string(&true), "true");
        assert_eq!(to_string(&"hello"), r#""hello""#);
    }

    #[test]
    fn test_parse_primitives() {
        assert_eq!(from_str::<i32>("42").unwrap(), 42);
        assert_eq!(from_str::<f64>("3.14").unwrap(), 3.14);
        assert_eq!(from_str::<bool>("true").unwrap(), true);
        assert_eq!(from_str::<String>(r#""hello""#).unwrap(), "hello");
    }

    #[test]
    fn test_escape_sequences() {
        let s = "hello\nworld\t\"test\"\\path";
        let json = to_string(&s);
        let parsed: String = from_str(&json).unwrap();
        assert_eq!(s, parsed);
    }

    #[test]
    fn test_unicode() {
        let s = "こんにちは 🦀 emoji";
        let json = to_string(&s);
        let parsed: String = from_str(&json).unwrap();
        assert_eq!(s, parsed);
    }

    #[test]
    fn test_dynamic_value() {
        let json = r#"{"name": "test", "values": [1, 2, 3], "nested": {"a": true}}"#;
        let value = parse(json).unwrap();

        assert!(value.is_object());
        assert_eq!(value["name"].as_str(), Some("test"));
        assert!(value["values"].is_array());
        assert_eq!(value["nested"]["a"].as_bool(), Some(true));
    }

    #[test]
    fn test_pretty_print() {
        let original = SimpleStruct {
            name: "test".to_string(),
            value: 42,
            active: true,
        };
        let json = to_string_pretty(&original);
        assert!(json.contains('\n'));
        let parsed: SimpleStruct = from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }
}
