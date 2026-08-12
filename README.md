# json-steroids 🚀

A high-performance, zero-copy JSON parsing and serialization library for Rust with derive macros for automatic implementation.

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Features

- **SIMD acceleration** - Integrated SIMD optimizations for 2-5% faster parsing (up to 5x on large arrays) on x86_64 (SSE2/AVX2) and ARM64 (NEON) - **Now fully integrated!** ⚡
- **Zero-copy parsing** - Strings without escape sequences are borrowed directly from input, avoiding unnecessary allocations
- **Fast serialization** - Pre-allocated buffers with efficient string escaping and number formatting
- **Derive macros** - Automatically generate serializers and deserializers for your types
- **Minimal dependencies** - Only uses `itoa` and `ryu` for fast number formatting
- **Full JSON support** - Handles all JSON types including Unicode escape sequences and surrogate pairs
- **Pretty printing** - Optional indented output for human-readable JSON
- **Dynamic values** - Parse JSON into a flexible `JsonValue` type when structure is unknown

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
json-steroids = "0.3.1"
```

## Quick Start

```rust
use json_steroids::{Json, to_string, from_str};

#[derive(Debug, Json, PartialEq)]
struct Person {
    name: String,
    age: u32,
    email: Option<String>,
}

fn main() {
    // Serialize
    let person = Person {
        name: "Alice".to_string(),
        age: 30,
        email: Some("alice@example.com".to_string()),
    };
    let json = to_string(&person);
    println!("{}", json);
    // Output: {"name":"Alice","age":30,"email":"alice@example.com"}

    // Deserialize
    let json_str = r#"{"name":"Bob","age":25,"email":null}"#;
    let person: Person = from_str(json_str).unwrap();
    println!("{:?}", person);
}
```

## Performance Benchmarks

Comprehensive benchmark comparison between `json-steroids`, `serde_json`, and `sonic_rs`. Results are Criterion mean estimates from an Apple M4 Max run with native CPU flags:

```bash
RUSTFLAGS="-C target-cpu=native" cargo bench --bench benchmarks
```

Lower is better. The fastest result in each row is highlighted.

| Benchmark | json-steroids | serde_json | sonic_rs | Winner |
|----------|---------------|------------|----------|--------|
| `serialize_simple` | **23.2 ns** | 43.3 ns | 35.2 ns | json-steroids |
| `deserialize_simple` | **47.9 ns** | 54.6 ns | 75.8 ns | json-steroids |
| `serialize_complex` | **99.4 ns** | 207.8 ns | 189.0 ns | json-steroids |
| `deserialize_complex` | **406.4 ns** | 586.4 ns | 612.0 ns | json-steroids |
| `roundtrip_complex` | **530.7 ns** | 853.6 ns | 833.2 ns | json-steroids |
| `parse_dynamic` | 420.9 ns | 536.3 ns | **165.6 ns** | sonic_rs |
| `large_array_serialize` | **3.43 us** | 3.51 us | 3.82 us | json-steroids |
| `large_array_deserialize` | **4.36 us** | 7.29 us | 8.58 us | json-steroids |
| `string_serialize_no_escapes` | **22.4 ns** | 34.2 ns | 61.4 ns | json-steroids |
| `string_serialize_with_escapes` | 69.2 ns | **48.4 ns** | 88.0 ns | serde_json |
| `deeply_nested_parse` | 837.0 ns | 952.8 ns | **312.0 ns** | sonic_rs |
| `many_fields_serialize` | **124.3 ns** | 241.8 ns | 216.9 ns | json-steroids |
| `many_fields_deserialize` | **391.0 ns** | 446.6 ns | 465.7 ns | json-steroids |
| `integers_serialize` | 3.85 us | **3.65 us** | 3.97 us | serde_json |
| `floats_serialize` | 18.09 us | 12.95 us | **12.88 us** | sonic_rs |
| `cow_struct_deserialize` | **67.4 ns** | 188.7 ns | 214.0 ns | json-steroids |
| `cow_struct_serialize` | **50.7 ns** | 132.0 ns | 121.5 ns | json-steroids |
| `borrowed_str_struct_deserialize` | **38.7 ns** | 66.0 ns | 65.1 ns | json-steroids |

### Performance Summary

- **json-steroids wins 13 of 18 benchmark groups** in this run.
- **Typed struct round-trips are the strongest path**: `roundtrip_complex` is about 1.6x faster than `serde_json` and `sonic_rs`.
- **Zero-copy deserialization is a major advantage**: `cow_struct_deserialize` is about 2.8x faster than `serde_json`, and borrowed `&str` struct deserialization is about 1.7x faster.
- **Large integer arrays deserialize quickly**: `large_array_deserialize` is about 1.7x faster than `serde_json` and about 2.0x faster than `sonic_rs`.
- **sonic_rs is still the best choice for DOM-heavy dynamic parsing** in these benchmarks, especially `parse_dynamic` and `deeply_nested_parse`.
- **Known slower paths**: escaped string serialization, float serialization, and dynamic DOM parsing compared with `sonic_rs`.

### Why Choose json-steroids?

**Optimized for real-world typed JSON workloads:**
- **Zero-copy string parsing** - Strings without escape sequences are borrowed directly (`Cow::Borrowed`)
- **Fast typed derive path** - Strong results for simple, complex, and many-field structs
- **Fast round-trips** - Efficient serializer plus typed parser for request/response style workloads
- **Large array deserialization** - Efficient integer parsing for dense numeric payloads
- **Type-safe numbers** - Specific methods for each integer/float type (no implicit conversions)
- **Memory efficient** - Pre-allocated buffers, minimal reallocations
- **Production ready** - Handles Unicode escapes, surrogate pairs, all JSON edge cases

> **Note**: Benchmarks vary by CPU, compiler flags, input shape, and feature flags. Run `cargo bench --bench benchmarks` on your own workload before making performance-sensitive decisions.

### Key Performance Features

- **Zero-copy string parsing** - Strings without escape sequences are borrowed directly, avoiding allocations
- **Cow<'de, str> support** - True zero-copy deserialization with `Cow::Borrowed` for strings without escapes
- **Fast number formatting** - Uses `itoa` and `ryu` for optimized integer and float serialization
- **Efficient memory management** - Pre-allocated buffers minimize reallocations
- **Optimized string escaping** - Fast-path detection for strings that don't need escaping
- **Minimal overhead** - Streamlined trait implementations with no unnecessary abstractions

### Zero-Copy Deserialization

json-steroids supports true zero-copy deserialization using `Cow<'de, str>`:

```rust
use json_steroids::from_str;
use std::borrow::Cow;

// Zero-copy: strings without escape sequences
let json = r#""hello world""#;
let result: Cow<str> = from_str(json).unwrap();
assert!(matches!(result, Cow::Borrowed(_))); // No allocation!

// In collections - zero-copy for all elements without escapes
let json = r#"["apple","banana","cherry"]"#;
let result: Vec<Cow<str>> = from_str(json).unwrap();
// All three elements are Cow::Borrowed - zero allocations!

// Automatic handling of escapes
let json = r#""hello\nworld""#;
let result: Cow<str> = from_str(json).unwrap();
assert!(matches!(result, Cow::Owned(_))); // Owned only when necessary
```

**Performance advantage**: In the benchmark suite above, json-steroids with `Cow<'de, str>` is about 2.8x faster than serde_json's zero-copy mode for deserialization and about 2.6x faster for serialization.

### Running Benchmarks

To run benchmarks on your own system:

```bash
cargo bench
```

View the detailed HTML report:

```bash
open target/criterion/report/index.html
```

## Derive Macros

### `#[derive(Json)]`

The combined derive macro that implements both `JsonSerialize` and `JsonDeserialize`:

```rust
use json_steroids::Json;

#[derive(Json)]
struct User {
    id: u64,
    username: String,
    active: bool,
}
```

### `#[derive(JsonSerialize)]` and `#[derive(JsonDeserialize)]`

Use these when you only need one direction:

```rust
use json_steroids::{JsonSerialize, JsonDeserialize};

#[derive(JsonSerialize)]
struct LogEntry {
    timestamp: u64,
    message: String,
}

#[derive(JsonDeserialize)]
struct Config {
    host: String,
    port: u16,
}
```

### Field renaming and aliasing

Use `#[json(rename = "...")]` attribute to set the corresponding field names in JSON.
Use `#[json(alias = "...")]` to create a deserialization alias for the desired field
(multiple alias flags for a single field are supported).

```rust
use json_steroids::Json;

#[derive(Json)]
struct ApiResponse {
    #[json(rename = "statusCode")]
    status_code: u32,
    #[json(rename = "errorMessage", alias="msg")]
    error_message: Option<String>,
}
```

To rename all the fields and enum variants use container-level `rename_all` attribute:
```rust
#[derive(Json)]
#[json(rename_all = "camelCase")]
enum ApiResult {
    Ok { return_value: f64 }, // -> {"ok":{"returnValue": 2.71828}}
    Error { error_code: i16}, // -> {"error":{"errorCode": -1}}
}
```

### Skipping fields

Use the `#[json(skip_deserializing)]` and `#[json(skip_serializing)]` attributes
(or just `#[json(skip)]` for both) to skip desired fields:

```rust
use json_steroids::Json;

#[derive(Default, Json)]
struct User {
    name: String,
    #[json(skip)]
    token: String,
}
```

### Default values for optional fields

Use the `#[json(default)]` or `#[json(default=custom_function)]` field
attributes to set default values for optional fields:

```rust
use json_steroids::Json;
use std::cell::OnceCell;

#[derive(Json)]
struct ApiResponse {
    #[json(rename = "statusCode", default=200)]
    status_code: u32,
    #[json(default = custom)]
    error_message: String,
}

fn custom() -> String {
    let cell: OnceCell<String> = OnceCell::new();
    let default_value = cell.get_or_init(|| {
        String::from("Runtime default value")
    });
    default_value.to_string()
}
```

### Custom serialization and deserialization functions

Use the `#[json(serialize_with=custom_ser_function)]` and/or `#[json(deserialize_with=custom_de_function)]` field
attributes to set custom serializer and deserializer functions:

```rust
use json_steroids::Json;

#[derive(Json)]
struct ApiResponse {
    #[json(serialize_with = ser_status, deserialize_with = de_status)]
    status: bool,
}

fn ser_status<W: json_steroids::writer::Writer>(
    value: &bool,
    writer: &mut json_steroids::JsonWriter<W>,
) {
    if *value {
        writer.write_string("OK")
    } else {
        writer.write_string("Error")
    }
}

fn de_status<'de>(parser: &mut json_steroids::JsonParser<'de>) -> json_steroids::Result<bool> {
    let pos = parser.position();
    let s = parser.parse_string()?;
    match &*s {
        "OK" => Ok(true),
        "Error" => Ok(false),
        _ => Err(json_stroids::JsonError::Custom(format!("Unknown status at position {pos}: `{s}`")))
    }
}
```

You can also create a dedicated module with `serialize` and `deserialize` functions and specify its name inside `with = path::to::module` flag:

```rust
#[derive(Json)]
struct ApiResponse {
    #[json(with = custom_ser_de)]
    status: bool,
}

mod custom_ser_de {
    fn serialize<W: json_steroids::writer::Writer>(
        value: &bool,
        writer: &mut json_steroids::JsonWriter<W>,
    ) {
        // ..
    }

    // change FieldType to type of a given field
    fn deserialize<'de>(parser: &mut json_steroids::JsonParser<'de>) -> json_steroids::Result<FieldType> {
        // ..
    }
}
```

### Enum Support

Enums are fully supported with different representations:

```rust
use json_steroids::Json;

// Unit variants serialize as strings
#[derive(Json)]
enum Status {
    Active,    // "Active"
    Inactive,  // "Inactive"
    Pending,   // "Pending"
}

// Tuple and struct variants use object notation
#[derive(Json)]
enum Message {
    Text(String),                    // {"Text":["hello"]}
    Coordinates { x: i32, y: i32 },  // {"Coordinates":{"x":10,"y":20}}
}
```

## API Reference

### Serialization Functions

```rust
// Compact JSON output
pub fn to_string<T: JsonSerialize>(value: &T) -> String;

// Pretty-printed JSON with 2-space indentation
pub fn to_string_pretty<T: JsonSerialize>(value: &T) -> String;
```

### Deserialization Functions

```rust
// Parse from string slice
pub fn from_str<T: JsonDeserialize>(s: &str) -> Result<T>;

// Parse from bytes
pub fn from_bytes<T: JsonDeserialize>(bytes: &[u8]) -> Result<T>;
```

### Dynamic Parsing

When the JSON structure isn't known at compile time:

```rust
use json_steroids::{parse, JsonValue};

let json = r#"{"name": "test", "values": [1, 2, 3]}"#;
let value = parse(json).unwrap();

// Access fields using indexing
assert_eq!(value["name"].as_str(), Some("test"));
assert!(value["values"].is_array());
assert_eq!(value["values"][0].as_i64(), Some(1));

// Check types
assert!(value.is_object());
assert!(value["missing"].is_null()); // Missing fields return null
```

### JsonValue Type

The `JsonValue` enum represents any JSON value:

```rust
pub enum JsonValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}
```

Methods available on `JsonValue`:
- Type checking: `is_null()`, `is_bool()`, `is_number()`, `is_string()`, `is_array()`, `is_object()`
- Value extraction: `as_bool()`, `as_i64()`, `as_u64()`, `as_f64()`, `as_str()`, `as_array()`, `as_object()`
- Ownership: `into_string()`, `into_array()`, `into_object()`
- Indexing: `value["key"]` for objects, `value[0]` for arrays

## Supported Types

### Primitives
- Booleans: `bool`
- Integers: `i8`, `i16`, `i32`, `i64`, `isize`, `u8`, `u16`, `u32`, `u64`, `usize`
- Floats: `f32`, `f64`

### Strings
- `String`
- `&str` (serialize only)
- `Cow<str>`

### Collections
- `Vec<T>`
- `[T; N]` (arrays, serialize only)
- `HashMap<K, V>` (K must be string-like)
- `BTreeMap<K, V>` (K must be string-like)

### Wrapper Types
- `Option<T>` - Serializes as `null` when `None`
- `Box<T>`

### Tuples
Tuples up to 8 elements are supported and serialize as JSON arrays:

```rust
let tuple = (1, "hello", true);
let json = to_string(&tuple); // [1,"hello",true]
```

## Error Handling

The library provides detailed error messages:

```rust
use json_steroids::{from_str, JsonError};

let result: Result<i32, _> = from_str("not a number");
match result {
    Ok(value) => println!("Parsed: {}", value),
    Err(JsonError::ExpectedToken(expected, pos)) => {
        println!("Expected {} at position {}", expected, pos);
    }
    Err(e) => println!("Error: {}", e),
}
```

Error types include:
- `UnexpectedEnd` - Input ended unexpectedly
- `UnexpectedChar(char, usize)` - Unexpected character at position
- `ExpectedChar(char, usize)` - Expected specific character
- `ExpectedToken(&str, usize)` - Expected token (e.g., "string", "number")
- `InvalidNumber(usize)` - Invalid number format
- `InvalidEscape(usize)` - Invalid escape sequence
- `InvalidUnicode(usize)` - Invalid Unicode escape
- `InvalidUtf8` - Invalid UTF-8 encoding
- `MissingField(String)` - Required field missing during deserialization
- `UnknownVariant(String)` - Unknown enum variant
- `TypeMismatch` - Type mismatch during deserialization
- `NestingTooDeep(usize)` - JSON nesting exceeds maximum depth (128)

## Performance

json-steroids is designed for high performance:

### Zero-Copy Parsing
Strings that don't contain escape sequences are borrowed directly from the input buffer using `Cow<str>`, avoiding allocation:

```rust
// This string has no escapes - zero allocation!
let json = r#"{"name": "hello world"}"#;

// This string has escapes - allocation needed to unescape
let json = r#"{"name": "hello\nworld"}"#;
```

### Fast Number Formatting
Uses the `itoa` and `ryu` crates for extremely fast integer and floating-point formatting.

### Efficient String Escaping
The serializer uses a fast path that checks if escaping is needed before processing:

```rust
// Fast path - no escaping needed
let s = "hello world";

// Slow path - escaping required
let s = "hello\nworld";
```

### Pre-allocated Buffers
The `JsonWriter` pre-allocates buffer space to minimize reallocations during serialization.

## Architecture

```
json-steroids/
├── src/
│   ├── lib.rs       # Public API and re-exports
│   ├── parser.rs    # Zero-copy JSON parser
│   ├── writer.rs    # Fast JSON serializer
│   ├── value.rs     # Dynamic JsonValue type
│   ├── traits.rs    # JsonSerialize/JsonDeserialize traits + impls
│   └── error.rs     # Error types
└── json-steroids-derive/
    └── src/
        └── lib.rs   # Procedural macros
```

## Examples

### Nested Structures

```rust
use json_steroids::Json;

#[derive(Json)]
struct Address {
    street: String,
    city: String,
    country: String,
}

#[derive(Json)]
struct Company {
    name: String,
    address: Address,
    employees: Vec<String>,
}

let company = Company {
    name: "Acme Corp".to_string(),
    address: Address {
        street: "123 Main St".to_string(),
        city: "Springfield".to_string(),
        country: "USA".to_string(),
    },
    employees: vec!["Alice".to_string(), "Bob".to_string()],
};

let json = to_string(&company);
```

### Working with Optional Fields

```rust
use json_steroids::{Json, from_str};

#[derive(Json, Debug)]
struct UserProfile {
    username: String,
    bio: Option<String>,
    age: Option<u32>,
}

// Missing optional fields default to None
let json = r#"{"username": "alice"}"#;
let profile: UserProfile = from_str(json).unwrap();
assert!(profile.bio.is_none());
assert!(profile.age.is_none());

// Explicit null also becomes None
let json = r#"{"username": "bob", "bio": null, "age": 25}"#;
let profile: UserProfile = from_str(json).unwrap();
assert!(profile.bio.is_none());
assert_eq!(profile.age, Some(25));
```

### Pretty Printing

```rust
use json_steroids::{Json, to_string_pretty};

#[derive(Json)]
struct Config {
    debug: bool,
    port: u16,
}

let config = Config { debug: true, port: 8080 };
let json = to_string_pretty(&config);
// Output:
// {
//   "debug": true,
//   "port": 8080
// }
```

### Custom Serialization with JsonWriter

For advanced use cases, you can use `JsonWriter` directly:

```rust
use json_steroids::JsonWriter;

let mut writer = JsonWriter::new();
writer.begin_object();
writer.write_key("name");
writer.write_string("custom");
writer.write_comma();
writer.write_key("values");
writer.begin_array();
writer.write_i64(1);
writer.write_comma();
writer.write_i64(2);
writer.end_array();
writer.end_object();

let json = writer.into_string();
// {"name":"custom","values":[1,2]}
```

## SIMD Acceleration ⚡

json-steroids includes optional SIMD (Single Instruction Multiple Data) optimizations that can provide **2-5x speedup** for string scanning, whitespace skipping, and escape detection on supported platforms.

### Architecture Support

- ✅ **x86_64** (Intel/AMD): SSE2 (16 bytes) and AVX2 (32 bytes) 
- ✅ **ARM64** (Apple Silicon, mobile, servers): NEON (16 bytes)
- ✅ **Fallback**: Scalar implementation for all other platforms

SIMD is **enabled by default** and automatically detects CPU capabilities at runtime.

### Quick Start

```toml
# SIMD enabled (default)
[dependencies]
json-steroids = "0.2"

# Disable SIMD (use scalar fallback)
[dependencies]
json-steroids = { version = "0.2", default-features = false }
```

### Performance Impact

| Operation | Scalar | SIMD | Speedup |
|-----------|--------|------|---------|
| String scanning (no escapes) | 1x | 3-5x | 🚀🚀🚀 |
| String scanning (with escapes) | 1x | 1.5-2x | 🚀 |
| Whitespace skipping | 1x | 2-3x | 🚀🚀 |
| Escape detection | 1x | 4-6x | 🚀🚀🚀 |

**Real-world impact**: 1.5-3x faster overall JSON parsing depending on content.

### Examples

```bash
# Run SIMD demo
cargo run --example simd_demo --release

# Compare SIMD vs scalar
cargo run --example simd_demo --no-default-features --release

# Benchmark SIMD performance
cargo bench --bench simd_benchmarks
```

## Running Benchmarks

```bash
cargo bench
```

## Running Tests

```bash
cargo test
```

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
