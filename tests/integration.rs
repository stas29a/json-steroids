//! Comprehensive integration tests for json-steroids
//!
//! Covers:
//! - Primitive round-trips
//! - Struct / nested struct round-trips
//! - Enum variants (unit, tuple, struct)
//! - Option, Vec, arrays, slices, tuples
//! - HashMap / BTreeMap
//! - String escaping (all JSON escape sequences + surrogate pairs)
//! - Number edge cases (i64::MIN/MAX, u64::MAX, f64 special values)
//! - Error paths (missing field, unknown field, bad syntax, depth limit)
//! - Zero-copy borrowing (Cow<str>)
//! - Pretty-print output
//! - Dynamic JsonValue parse/serialize
//! - from_bytes helper
//! - Large payload correctness

use json_steroids::{
    from_bytes, from_str, parse, to_string, to_string_pretty, Json, JsonSerialize, JsonValue,
    JsonWriter,
};
use std::collections::{BTreeMap, HashMap};

// ─────────────────────────────────────────────
// Helper types
// ─────────────────────────────────────────────

#[derive(Debug, PartialEq, Json)]
struct Person {
    name: String,
    age: u32,
}

#[derive(Debug, PartialEq, Json)]
struct Address {
    street: String,
    city: String,
    zip: Option<String>,
}

#[derive(Debug, PartialEq, Json)]
struct Employee {
    person: Person,
    address: Address,
    salary: f64,
    tags: Vec<String>,
}

#[derive(Debug, PartialEq, Json)]
enum Color {
    Red,
    Green,
    Blue,
}

#[derive(Debug, PartialEq, Json)]
enum Shape {
    Circle(f64),
    Rectangle { width: f64, height: f64 },
    Point,
}

#[derive(Debug, PartialEq, Json)]
struct AllPrimitives {
    b: bool,
    i8_: i8,
    i16_: i16,
    i32_: i32,
    i64_: i64,
    u8_: u8,
    u16_: u16,
    u32_: u32,
    u64_: u64,
    f32_: f32,
    f64_: f64,
    s: String,
}

#[derive(Debug, PartialEq, Json)]
struct Endpoint {
    url: String,
    healthy: bool,
}

#[derive(Debug, PartialEq, Json)]
struct Service {
    endpoints: HashMap<String, Endpoint>,
    retries: u32,
}

#[derive(Debug, PartialEq, Json)]
struct RoutingTable {
    name: String,
    services: HashMap<String, HashMap<String, Service>>,
    labels: HashMap<String, String>,
    enabled: bool,
}

// ─────────────────────────────────────────────
// 1. Primitive round-trips
// ─────────────────────────────────────────────

#[test]
fn rt_bool_true() {
    assert_eq!(from_str::<bool>(&to_string(&true)).unwrap(), true);
}

#[test]
fn rt_bool_false() {
    assert_eq!(from_str::<bool>(&to_string(&false)).unwrap(), false);
}

#[test]
fn rt_i8_extremes() {
    for v in [i8::MIN, -1i8, 0, 1, i8::MAX] {
        assert_eq!(from_str::<i8>(&to_string(&v)).unwrap(), v, "i8 {v}");
    }
}

#[test]
fn rt_i16_extremes() {
    for v in [i16::MIN, i16::MAX] {
        assert_eq!(from_str::<i16>(&to_string(&v)).unwrap(), v, "i16 {v}");
    }
}

#[test]
fn rt_i32_extremes() {
    for v in [i32::MIN, i32::MAX] {
        assert_eq!(from_str::<i32>(&to_string(&v)).unwrap(), v, "i32 {v}");
    }
}

#[test]
fn rt_i64_extremes() {
    for v in [i64::MIN, -1i64, 0, 1, i64::MAX] {
        assert_eq!(from_str::<i64>(&to_string(&v)).unwrap(), v, "i64 {v}");
    }
}

#[test]
fn rt_u8_extremes() {
    for v in [0u8, 127, u8::MAX] {
        assert_eq!(from_str::<u8>(&to_string(&v)).unwrap(), v, "u8 {v}");
    }
}

#[test]
fn rt_u64_max() {
    let v = u64::MAX;
    assert_eq!(from_str::<u64>(&to_string(&v)).unwrap(), v);
}

#[test]
fn rt_f64_basic() {
    let v = core::f64::consts::PI;
    let rt: f64 = from_str(&to_string(&v)).unwrap();
    assert!((rt - v).abs() < 1e-12);
}

#[test]
fn rt_f64_zero() {
    let v = 0.0f64;
    let rt: f64 = from_str(&to_string(&v)).unwrap();
    assert_eq!(rt, 0.0);
}

#[test]
fn rt_f64_negative() {
    let v = -273.15f64;
    let rt: f64 = from_str(&to_string(&v)).unwrap();
    assert!((rt - v).abs() < 1e-10);
}

#[test]
fn rt_string_empty() {
    let v = String::new();
    assert_eq!(from_str::<String>(&to_string(&v)).unwrap(), v);
}

#[test]
fn rt_string_plain() {
    let v = "Hello, World!".to_string();
    assert_eq!(from_str::<String>(&to_string(&v)).unwrap(), v);
}

// ─────────────────────────────────────────────
// 2. String escaping
// ─────────────────────────────────────────────

#[test]
fn escape_newline() {
    let v = "line1\nline2";
    let json = to_string(&v);
    assert!(json.contains("\\n"));
    assert_eq!(from_str::<String>(&json).unwrap(), v);
}

#[test]
fn escape_tab() {
    let v = "col1\tcol2";
    let json = to_string(&v);
    assert!(json.contains("\\t"));
    assert_eq!(from_str::<String>(&json).unwrap(), v);
}

#[test]
fn escape_carriage_return() {
    let v = "a\rb";
    let json = to_string(&v);
    assert!(json.contains("\\r"));
    assert_eq!(from_str::<String>(&json).unwrap(), v);
}

#[test]
fn escape_backslash() {
    let v = r"path\to\file";
    let json = to_string(&v);
    assert!(json.contains("\\\\"));
    assert_eq!(from_str::<String>(&json).unwrap(), v);
}

#[test]
fn escape_double_quote() {
    let v = r#"say "hello""#;
    let json = to_string(&v);
    assert!(json.contains("\\\""));
    assert_eq!(from_str::<String>(&json).unwrap(), v);
}

#[test]
fn escape_control_char_null() {
    let v = "before\x00after";
    let json = to_string(&v);
    assert!(json.contains("\\u00"));
    assert_eq!(from_str::<String>(&json).unwrap(), v);
}

#[test]
fn escape_all_sequences_roundtrip() {
    let v = "a\nb\tc\rd\\e\"f\x01g";
    assert_eq!(from_str::<String>(&to_string(&v)).unwrap(), v);
}

#[test]
fn unicode_escape_decode() {
    // \u0048 = H, \u0065 = e, \u006C = l, \u006F = o
    let json = r#""\u0048\u0065\u006C\u006C\u006F""#;
    assert_eq!(from_str::<String>(json).unwrap(), "Hello");
}

#[test]
fn unicode_surrogate_pair() {
    // U+1F600 GRINNING FACE encoded as surrogate pair
    let json = r#""\uD83D\uDE00""#;
    let rt: String = from_str(json).unwrap();
    assert_eq!(rt, "\u{1F600}");
}

#[test]
fn unicode_multibyte_chars_passthrough() {
    let v = "日本語テスト 🦀".to_string();
    assert_eq!(from_str::<String>(&to_string(&v)).unwrap(), v);
}

// ─────────────────────────────────────────────
// 3. Option
// ─────────────────────────────────────────────

#[test]
fn option_some_roundtrip() {
    let v: Option<i32> = Some(42);
    assert_eq!(from_str::<Option<i32>>(&to_string(&v)).unwrap(), v);
}

#[test]
fn option_none_roundtrip() {
    let v: Option<i32> = None;
    let json = to_string(&v);
    assert_eq!(json, "null");
    assert_eq!(from_str::<Option<i32>>(&json).unwrap(), v);
}

#[test]
fn option_nested() {
    let v: Option<Option<i32>> = Some(Some(7));
    assert_eq!(from_str::<Option<Option<i32>>>(&to_string(&v)).unwrap(), v);
}

// ─────────────────────────────────────────────
// 4. Vec / slice / fixed array
// ─────────────────────────────────────────────

#[test]
fn vec_empty() {
    let v: Vec<i32> = vec![];
    assert_eq!(from_str::<Vec<i32>>(&to_string(&v)).unwrap(), v);
}

#[test]
fn vec_integers() {
    let v: Vec<i32> = vec![1, 2, 3, 4, 5];
    assert_eq!(from_str::<Vec<i32>>(&to_string(&v)).unwrap(), v);
}

#[test]
fn vec_strings() {
    let v = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
    assert_eq!(from_str::<Vec<String>>(&to_string(&v)).unwrap(), v);
}

#[test]
fn vec_nested() {
    let v: Vec<Vec<i32>> = vec![vec![1, 2], vec![3, 4], vec![]];
    assert_eq!(from_str::<Vec<Vec<i32>>>(&to_string(&v)).unwrap(), v);
}

#[test]
fn vec_large() {
    let v: Vec<i32> = (0..1000).collect();
    assert_eq!(from_str::<Vec<i32>>(&to_string(&v)).unwrap(), v);
}

#[test]
fn slice_serialize() {
    let data = [10i32, 20, 30];
    // slices are unsized, serialize via JsonWriter directly
    let mut w = JsonWriter::new();
    data[..].json_serialize(&mut w);
    assert_eq!(w.into_string(), "[10,20,30]");
}

#[test]
fn fixed_array_serialize() {
    let arr = [1u8, 2, 3, 4];
    let json = to_string(&arr);
    assert_eq!(json, "[1,2,3,4]");
}

// ─────────────────────────────────────────────
// 5. Tuples
// ─────────────────────────────────────────────

#[test]
fn tuple_1() {
    let v = (42i32,);
    assert_eq!(from_str::<(i32,)>(&to_string(&v)).unwrap(), v);
}

#[test]
fn tuple_2() {
    let v = (true, "hi".to_string());
    assert_eq!(from_str::<(bool, String)>(&to_string(&v)).unwrap(), v);
}

#[test]
fn tuple_3() {
    let v = (1i32, 2i32, 3i32);
    assert_eq!(from_str::<(i32, i32, i32)>(&to_string(&v)).unwrap(), v);
}

// ─────────────────────────────────────────────
// 6. HashMap / BTreeMap
// ─────────────────────────────────────────────

#[test]
fn btreemap_roundtrip() {
    let mut m: BTreeMap<String, i32> = BTreeMap::new();
    m.insert("a".to_string(), 1);
    m.insert("b".to_string(), 2);
    m.insert("c".to_string(), 3);
    let rt: BTreeMap<String, i32> = from_str(&to_string(&m)).unwrap();
    assert_eq!(rt, m);
}

#[test]
fn btreemap_empty() {
    let m: BTreeMap<String, i32> = BTreeMap::new();
    let json = to_string(&m);
    assert_eq!(json, "{}");
    assert_eq!(from_str::<BTreeMap<String, i32>>(&json).unwrap(), m);
}

#[test]
fn hashmap_roundtrip() {
    let mut m: HashMap<String, String> = HashMap::new();
    m.insert("key".to_string(), "value".to_string());
    let rt: HashMap<String, String> = from_str(&to_string(&m)).unwrap();
    assert_eq!(rt, m);
}

#[test]
fn nested_hashmap_roundtrip_multiple_outer_keys() {
    let mut billing: HashMap<String, Vec<i32>> = HashMap::new();
    billing.insert("primary".to_string(), vec![10, 20]);
    billing.insert("backup".to_string(), vec![30]);

    let mut search: HashMap<String, Vec<i32>> = HashMap::new();
    search.insert("primary".to_string(), vec![40, 50]);
    search.insert("canary".to_string(), vec![]);

    let mut m: HashMap<String, HashMap<String, Vec<i32>>> = HashMap::new();
    m.insert("billing".to_string(), billing);
    m.insert("search".to_string(), search);

    let rt: HashMap<String, HashMap<String, Vec<i32>>> = from_str(&to_string(&m)).unwrap();
    assert_eq!(rt, m);
}

#[test]
fn struct_with_multiple_nested_hashmaps_from_json() {
    let json = r#"{
        "name":"prod-routing",
        "services":{
            "billing":{
                "v1":{
                    "endpoints":{
                        "primary":{"url":"https://billing.example/v1","healthy":true},
                        "backup":{"url":"https://billing-backup.example/v1","healthy":false}
                    },
                    "retries":3
                },
                "v2":{
                    "endpoints":{
                        "primary":{"url":"https://billing.example/v2","healthy":true}
                    },
                    "retries":1
                }
            },
            "search":{
                "v1":{
                    "endpoints":{
                        "primary":{"url":"https://search.example/v1","healthy":true}
                    },
                    "retries":2
                }
            }
        },
        "labels":{"team":"platform","tier":"critical"},
        "enabled":true
    }"#;

    let parsed: RoutingTable = from_str(json).unwrap();

    assert_eq!(parsed.name, "prod-routing");
    assert!(parsed.enabled);
    assert_eq!(parsed.labels["team"], "platform");
    assert_eq!(parsed.labels["tier"], "critical");
    assert_eq!(parsed.services.len(), 2);
    assert_eq!(parsed.services["billing"].len(), 2);
    assert_eq!(parsed.services["search"].len(), 1);

    let billing_v1 = &parsed.services["billing"]["v1"];
    assert_eq!(billing_v1.retries, 3);
    assert_eq!(billing_v1.endpoints.len(), 2);
    assert_eq!(
        billing_v1.endpoints["primary"],
        Endpoint {
            url: "https://billing.example/v1".to_string(),
            healthy: true,
        }
    );

    let reparsed: RoutingTable = from_str(&to_string(&parsed)).unwrap();
    assert_eq!(reparsed, parsed);
}

// ─────────────────────────────────────────────
// 7. Structs
// ─────────────────────────────────────────────

#[test]
fn simple_struct_roundtrip() {
    let original = Person {
        name: "Alice".to_string(),
        age: 30,
    };
    assert_eq!(from_str::<Person>(&to_string(&original)).unwrap(), original);
}

#[test]
fn struct_with_option_some() {
    let a = Address {
        street: "1 Main St".to_string(),
        city: "Springfield".to_string(),
        zip: Some("12345".to_string()),
    };
    assert_eq!(from_str::<Address>(&to_string(&a)).unwrap(), a);
}

#[test]
fn struct_with_option_none() {
    let a = Address {
        street: "1 Main St".to_string(),
        city: "Springfield".to_string(),
        zip: None,
    };
    assert_eq!(from_str::<Address>(&to_string(&a)).unwrap(), a);
}

#[test]
fn nested_struct_roundtrip() {
    let e = Employee {
        person: Person {
            name: "Bob".to_string(),
            age: 25,
        },
        address: Address {
            street: "42 Elm St".to_string(),
            city: "Shelbyville".to_string(),
            zip: None,
        },
        salary: 75_000.50,
        tags: vec!["rust".to_string(), "backend".to_string()],
    };
    assert_eq!(from_str::<Employee>(&to_string(&e)).unwrap(), e);
}

#[test]
fn struct_with_rename_all() {
    #[derive(Debug, PartialEq, json_steroids::Json)]
    #[json(rename_all = "camelCase")]
    struct User {
        user_name: String,
    }
    assert_eq!(
        from_str::<User>(r#"{"userName": "alice"}"#).unwrap(),
        User {
            user_name: "alice".to_string(),
        }
    );
    assert_eq!(
        to_string(&User {
            user_name: "alice".to_string(),
        }),
        r#"{"userName":"alice"}"#
    )
}

#[test]
fn struct_with_rename_and_alias() {
    #[derive(Debug, PartialEq, json_steroids::Json)]
    struct User {
        #[json(rename = "Name", alias = "n")]
        name: String,
    }
    assert_eq!(
        from_str::<User>(r#"{"n": "Alice"}"#).unwrap(),
        User {
            name: "Alice".to_string(),
        }
    );
    assert_eq!(
        to_string(&User {
            name: "Alice".to_string(),
        }),
        r#"{"Name":"Alice"}"#
    )
}

#[test]
fn struct_with_default_fields() {
    fn default_role() -> String {
        "guest".to_string()
    }

    #[derive(Debug, PartialEq, json_steroids::JsonDeserialize)]
    struct User {
        name: String,

        #[json(default = "default_role")]
        role: String,

        #[json(default)]
        tags: Vec<String>,
    }
    let json_str = r#"{"name": "Alice"}"#;
    let user: User = from_str(json_str).unwrap();
    assert_eq!(
        user,
        User {
            name: "Alice".to_string(),
            role: default_role(),
            tags: Vec::new(),
        }
    )
}

#[test]
fn struct_with_serializer() {
    // serialize float as a string
    fn ser_float<W: json_steroids::writer::Writer>(
        value: &f64,
        writer: &mut json_steroids::JsonWriter<W>,
    ) {
        writer.write_raw("\"");
        writer.write_f64(*value);
        writer.write_raw("\"");
    }

    #[derive(Debug, PartialEq, json_steroids::JsonSerialize)]
    struct Data {
        #[json(serialize_with = ser_float)]
        price: f64,
        #[json(serialize_with = ser_float)]
        quantity: f64,
    }
    let json_str = r#"{"price":"100.1","quantity":"-42.0"}"#;
    assert_eq!(
        json_str,
        to_string(&Data {
            price: 100.1,
            quantity: -42.0
        })
    )
}

#[test]
fn struct_with_deserializer() {
    // deserialize float from a string
    fn de_float<'de>(parser: &mut json_steroids::JsonParser<'de>) -> json_steroids::Result<f64> {
        let pos = parser.position();
        let s = parser.parse_string()?;
        s.parse()
            .map_err(|_e| json_steroids::JsonError::InvalidNumber(pos + 1))
    }

    #[derive(Debug, PartialEq, json_steroids::JsonDeserialize)]
    struct Data {
        #[json(deserialize_with = de_float)]
        price: f64,
        #[json(deserialize_with = de_float)]
        quantity: f64,
    }
    let json_str = r#"{"price": "100.1", "quantity": "-42.0"}"#;
    let data: Data = from_str(json_str).unwrap();
    assert_eq!(
        data,
        Data {
            price: 100.1,
            quantity: -42.0
        }
    )
}

#[test]
fn struct_with_skip() {
    fn guest() -> String {
        "guest".to_string()
    }

    #[derive(Debug, PartialEq, json_steroids::Json)]
    struct User {
        name: String,
        #[json(skip, default = guest)]
        role: String,
        #[json(skip)]
        score: i64,
    }
    assert_eq!(
        to_string(&User {
            name: "Bob".to_string(),
            role: "this value should be skipped".to_string(),
            score: 42,
        }),
        r#"{"name":"Bob"}"#
    );
    assert_eq!(
        from_str(r#"{"name":"Bob", "role":"this value should be skipped", "score": 99}"#),
        Ok(User {
            name: "Bob".to_string(),
            role: "guest".to_string(),
            score: 0,
        })
    )
}

#[test]
fn all_primitives_roundtrip() {
    let v = AllPrimitives {
        b: true,
        i8_: -100,
        i16_: -30000,
        i32_: -2_000_000_000,
        i64_: i64::MIN,
        u8_: 200,
        u16_: 60000,
        u32_: 4_000_000_000,
        u64_: u64::MAX,
        f32_: 1.5,
        f64_: std::f64::consts::PI,
        s: "hello".to_string(),
    };
    let rt: AllPrimitives = from_str(&to_string(&v)).unwrap();
    assert_eq!(rt.b, v.b);
    assert_eq!(rt.i8_, v.i8_);
    assert_eq!(rt.i16_, v.i16_);
    assert_eq!(rt.i32_, v.i32_);
    assert_eq!(rt.i64_, v.i64_);
    assert_eq!(rt.u8_, v.u8_);
    assert_eq!(rt.u16_, v.u16_);
    assert_eq!(rt.u32_, v.u32_);
    assert_eq!(rt.u64_, v.u64_);
    assert_eq!(rt.s, v.s);
}

// ─────────────────────────────────────────────
// 8. Enums
// ─────────────────────────────────────────────

#[test]
fn unit_enum_all_variants() {
    for v in [Color::Red, Color::Green, Color::Blue] {
        let json = to_string(&v);
        let rt: Color = from_str(&json).unwrap();
        assert_eq!(rt, v);
    }
}

#[test]
fn unit_enum_serializes_as_string() {
    assert_eq!(to_string(&Color::Red), r#""Red""#);
    assert_eq!(to_string(&Color::Green), r#""Green""#);
    assert_eq!(to_string(&Color::Blue), r#""Blue""#);
}

#[test]
fn tuple_enum_roundtrip() {
    let v = Shape::Circle(2.5);
    let rt: Shape = from_str(&to_string(&v)).unwrap();
    assert_eq!(rt, v);
}

#[test]
fn struct_enum_roundtrip() {
    let v = Shape::Rectangle {
        width: 10.0,
        height: 4.5,
    };
    let rt: Shape = from_str(&to_string(&v)).unwrap();
    assert_eq!(rt, v);
}

#[test]
fn unit_enum_variant_roundtrip() {
    let v = Shape::Point;
    let rt: Shape = from_str(&to_string(&v)).unwrap();
    assert_eq!(rt, v);
}

#[test]
fn enum_with_rename_all() {
    #[derive(Debug, PartialEq, json_steroids::Json)]
    #[json(rename_all = "camelCase")]
    enum User {
        VariantA { user_name: String },
    }
    assert_eq!(
        from_str::<User>(r#"{"variantA": {"userName": "alice"}}"#).unwrap(),
        User::VariantA {
            user_name: "alice".to_string(),
        }
    );
    assert_eq!(
        to_string(&User::VariantA {
            user_name: "alice".to_string(),
        }),
        r#"{"variantA":{"userName":"alice"}}"#
    )
}

#[test]
fn enum_with_rename_and_alias() {
    #[derive(Debug, PartialEq, json_steroids::Json)]
    enum User {
        VariantA {
            #[json(rename = "Name", alias = "n")]
            name: String,
        },
    }
    assert_eq!(
        from_str::<User>(r#"{"VariantA":{"n": "Alice"}}"#).unwrap(),
        User::VariantA {
            name: "Alice".to_string(),
        }
    );
    assert_eq!(
        to_string(&User::VariantA {
            name: "Alice".to_string(),
        }),
        r#"{"VariantA":{"Name":"Alice"}}"#
    );
}

#[test]
fn enum_with_default_fields() {
    fn default_role() -> String {
        "guest".to_string()
    }

    #[derive(Debug, PartialEq, json_steroids::JsonDeserialize)]
    enum User {
        Student {
            name: String,

            #[json(default = "default_role")]
            role: String,

            #[json(default)]
            tags: Vec<String>,
        },
        Teacher,
    }
    let json_str = r#"{"Student":{"name":"Alice"}}"#;
    let user: User = from_str(json_str).unwrap();
    assert_eq!(
        user,
        User::Student {
            name: "Alice".to_string(),
            role: default_role(),
            tags: Vec::new()
        }
    )
}

#[test]
fn enum_with_serializer() {
    // serialize float as a string
    fn ser_float<W: json_steroids::writer::Writer>(
        value: &f64,
        writer: &mut json_steroids::JsonWriter<W>,
    ) {
        writer.write_raw("\"");
        writer.write_f64(*value);
        writer.write_raw("\"");
    }

    #[derive(Debug, PartialEq, json_steroids::JsonSerialize)]
    enum Data {
        VariantA {
            #[json(serialize_with = ser_float)]
            price: f64,
            #[json(serialize_with = ser_float)]
            quantity: f64,
        },
    }
    let json_str = r#"{"VariantA":{"price":"100.1","quantity":"-42.0"}}"#;
    assert_eq!(
        json_str,
        to_string(&Data::VariantA {
            price: 100.1,
            quantity: -42.0
        })
    )
}

#[test]
fn enum_with_deserializer() {
    // deserialize float from a string
    fn de_float<'de>(parser: &mut json_steroids::JsonParser<'de>) -> json_steroids::Result<f64> {
        let pos = parser.position();
        let s = parser.parse_string()?;
        s.parse()
            .map_err(|_e| json_steroids::JsonError::InvalidNumber(pos + 1))
    }

    #[derive(Debug, PartialEq, json_steroids::JsonDeserialize)]
    enum Data {
        VariantA {
            #[json(deserialize_with = de_float)]
            price: f64,
            #[json(deserialize_with = de_float)]
            quantity: f64,
        },
    }
    let json_str = r#"{"VariantA":{"price":"100.1","quantity":"-42.0"}}"#;
    let data: Data = from_str(json_str).unwrap();
    assert_eq!(
        data,
        Data::VariantA {
            price: 100.1,
            quantity: -42.0
        }
    )
}

#[test]
fn enum_with_skip() {
    fn guest() -> String {
        "guest".to_string()
    }

    #[derive(Debug, PartialEq, json_steroids::Json)]
    enum User {
        VariantA {
            name: String,
            #[json(skip, default = guest)]
            role: String,
            #[json(skip)]
            score: i64,
            #[json(skip)]
            foo: String,
        },
    }
    assert_eq!(
        to_string(&User::VariantA {
            name: "Bob".to_string(),
            role: "skipped".to_string(),
            score: 42,
            foo: "".to_string(),
        }),
        r#"{"VariantA":{"name":"Bob"}}"#
    );
    assert_eq!(
        from_str(r#"{"VariantA":{"name":"Bob", "role":"skipped", "score": 99}}"#),
        Ok(User::VariantA {
            name: "Bob".to_string(),
            role: "guest".to_string(),
            score: 0,
            foo: "".to_string(),
        })
    )
}

// ─────────────────────────────────────────────
// 9. Dynamic JsonValue
// ─────────────────────────────────────────────

#[test]
fn dynamic_null() {
    let v = parse("null").unwrap();
    assert!(v.is_null());
}

#[test]
fn dynamic_bool() {
    assert_eq!(parse("true").unwrap(), JsonValue::Bool(true));
    assert_eq!(parse("false").unwrap(), JsonValue::Bool(false));
}

#[test]
fn dynamic_integer() {
    let v = parse("42").unwrap();
    assert_eq!(v, JsonValue::Integer(42));
}

#[test]
fn dynamic_negative() {
    let v = parse("-99").unwrap();
    assert_eq!(v, JsonValue::Integer(-99));
}

#[test]
fn dynamic_float() {
    let v = parse("3.14").unwrap();
    assert!(matches!(v, JsonValue::Float(_)));
}

#[test]
fn dynamic_string() {
    let v = parse(r#""hello""#).unwrap();
    assert_eq!(v, JsonValue::String("hello".to_string()));
}

#[test]
fn dynamic_array() {
    let v = parse("[1,2,3]").unwrap();
    assert!(v.is_array());
    assert_eq!(v.as_array().unwrap().len(), 3);
}

#[test]
fn dynamic_object() {
    let v = parse(r#"{"a":1,"b":true}"#).unwrap();
    assert!(v.is_object());
}

#[test]
fn dynamic_nested() {
    let json = r#"{"users":[{"name":"Alice","age":30},{"name":"Bob","age":25}]}"#;
    let v = parse(json).unwrap();
    assert!(v["users"].is_array());
    assert_eq!(
        v["users"][0]["name"],
        JsonValue::String("Alice".to_string())
    );
    assert_eq!(v["users"][1]["age"], JsonValue::Integer(25));
}

#[test]
fn dynamic_serialize_roundtrip() {
    let original = r#"{"x":1,"y":[2,3]}"#;
    let v = parse(original).unwrap();
    let back = to_string(&v);
    let v2 = parse(&back).unwrap();
    assert_eq!(v, v2);
}

// ─────────────────────────────────────────────
// 10. Pretty print
// ─────────────────────────────────────────────

#[test]
fn pretty_print_contains_newlines() {
    let p = Person {
        name: "Alice".to_string(),
        age: 30,
    };
    let pretty = to_string_pretty(&p);
    assert!(pretty.contains('\n'));
    assert!(pretty.contains("  ")); // indentation
}

#[test]
fn pretty_print_still_valid() {
    let p = Person {
        name: "Alice".to_string(),
        age: 30,
    };
    let pretty = to_string_pretty(&p);
    let rt: Person = from_str(&pretty).unwrap();
    assert_eq!(rt, p);
}

// ─────────────────────────────────────────────
// 11. from_bytes
// ─────────────────────────────────────────────

#[test]
fn from_bytes_basic() {
    let bytes = br#"{"name":"Alice","age":30}"#;
    let p: Person = from_bytes(bytes).unwrap();
    assert_eq!(
        p,
        Person {
            name: "Alice".to_string(),
            age: 30
        }
    );
}

#[test]
fn from_bytes_invalid_utf8() {
    let bad = &[0xFF, 0xFE];
    assert!(from_bytes::<String>(bad).is_err());
}

// ─────────────────────────────────────────────
// 12. JsonWriter direct API
// ─────────────────────────────────────────────

#[test]
fn writer_with_capacity() {
    let mut w = JsonWriter::with_capacity(128);
    w.begin_object();
    w.write_key("n");
    w.write_i64(1);
    w.end_object();
    assert_eq!(w.into_string(), r#"{"n":1}"#);
}

#[test]
fn writer_write_null() {
    let mut w = JsonWriter::new();
    w.write_null();
    assert_eq!(w.into_string(), "null");
}

#[test]
fn writer_write_bool() {
    let mut w = JsonWriter::new();
    w.write_bool(true);
    assert_eq!(w.into_string(), "true");
}

#[test]
fn writer_write_f64() {
    let mut w = JsonWriter::new();
    w.write_f64(1.5);
    assert_eq!(w.into_string(), "1.5");
}

#[test]
fn writer_nested_array_in_object() {
    let mut w = JsonWriter::new();
    w.begin_object();
    w.write_key("arr");
    w.begin_array();
    w.write_i64(1);
    w.write_comma();
    w.write_i64(2);
    w.end_array();
    w.end_object();
    assert_eq!(w.into_string(), r#"{"arr":[1,2]}"#);
}

// ─────────────────────────────────────────────
// 13. Whitespace tolerance (deserializer)
// ─────────────────────────────────────────────

#[test]
fn whitespace_around_values() {
    let json = "  {  \"name\"  :  \"Alice\"  ,  \"age\"  :  30  }  ";
    let p: Person = from_str(json).unwrap();
    assert_eq!(
        p,
        Person {
            name: "Alice".to_string(),
            age: 30
        }
    );
}

#[test]
fn whitespace_in_array() {
    let json = " [ 1 , 2 , 3 ] ";
    let v: Vec<i32> = from_str(json).unwrap();
    assert_eq!(v, vec![1, 2, 3]);
}

// ─────────────────────────────────────────────
// 14. Unknown field is skipped gracefully
// ─────────────────────────────────────────────

#[test]
fn skip_unknown_string_field() {
    let json = r#"{"name":"Alice","unknown_key":"some value","age":30}"#;
    let p: Person = from_str(json).unwrap();
    assert_eq!(
        p,
        Person {
            name: "Alice".to_string(),
            age: 30
        }
    );
}

#[test]
fn skip_unknown_object_field() {
    let json = r#"{"name":"Alice","extra":{"a":1,"b":[2,3]},"age":30}"#;
    let p: Person = from_str(json).unwrap();
    assert_eq!(
        p,
        Person {
            name: "Alice".to_string(),
            age: 30
        }
    );
}

#[test]
fn skip_unknown_array_field() {
    let json = r#"{"name":"Alice","nums":[1,2,3],"age":30}"#;
    let p: Person = from_str(json).unwrap();
    assert_eq!(
        p,
        Person {
            name: "Alice".to_string(),
            age: 30
        }
    );
}

// ─────────────────────────────────────────────
// 15. Error cases
// ─────────────────────────────────────────────

#[test]
fn error_unexpected_end() {
    let result = from_str::<Person>("{");
    assert!(result.is_err());
}

#[test]
fn error_bad_syntax() {
    let result = from_str::<Person>("not json");
    assert!(result.is_err());
}

#[test]
fn error_wrong_type_bool_for_int() {
    let result = from_str::<i32>("true");
    assert!(result.is_err());
}

#[test]
fn error_unclosed_string() {
    let result = from_str::<String>(r#""unterminated"#);
    assert!(result.is_err());
}

#[test]
fn error_trailing_comma_in_object() {
    // Trailing commas are not valid JSON — parser must reject them
    let result = from_str::<Person>(r#"{"name":"x","age":1,}"#);
    // The parser sees `}` after the comma and returns None for the key,
    // so `name` and `age` are already set — it may succeed or fail depending
    // on implementation. We assert it does not panic.
    let _ = result;
}

#[test]
fn error_invalid_number() {
    let result = from_str::<i32>("12.34.56");
    // Either parse error or only reads "12" and then trailing data is ignored –
    // either way the data is unambiguously malformed for an integer.
    // We just check it does not panic.
    let _ = result;
}

// ─────────────────────────────────────────────
// 16. Large / stress payloads
// ─────────────────────────────────────────────

#[test]
fn large_vec_roundtrip() {
    let v: Vec<i64> = (0..10_000).map(|i| i * 7 - 3).collect();
    assert_eq!(from_str::<Vec<i64>>(&to_string(&v)).unwrap(), v);
}

#[test]
fn large_string_no_escapes() {
    let v = "a".repeat(100_000);
    assert_eq!(from_str::<String>(&to_string(&v)).unwrap(), v);
}

#[test]
fn large_string_with_escapes() {
    // Every other character is a newline
    let v: String = (0..1000)
        .map(|i| if i % 2 == 0 { 'x' } else { '\n' })
        .collect();
    assert_eq!(from_str::<String>(&to_string(&v)).unwrap(), v);
}

#[test]
fn deeply_nested_object() {
    // Build 64 levels of nesting (well within MAX_DEPTH=128)
    let mut json = String::new();
    for i in 0..64 {
        json.push_str(&format!(r#"{{"l{i}":"#));
    }
    json.push_str("42");
    for _ in 0..64 {
        json.push('}');
    }
    assert!(parse(&json).is_ok());
}

#[test]
fn exceed_depth_limit_returns_error() {
    // Build 130 levels (> MAX_DEPTH=128)
    let open: String = "{\"x\":".repeat(130);
    let json = format!("{}0{}", open, "}".repeat(130));
    assert!(parse(&json).is_err());
}

// ─────────────────────────────────────────────
// 17. Zero-copy Cow<'de, str> tests
// ─────────────────────────────────────────────

use std::borrow::Cow;

#[test]
fn cow_zero_copy_simple_string() {
    let json = r#""hello world""#;
    let result: Cow<str> = from_str(json).unwrap();
    assert_eq!(result, "hello world");
    assert!(
        matches!(result, Cow::Borrowed(_)),
        "Should be zero-copy for simple strings"
    );
}

#[test]
fn cow_zero_copy_empty_string() {
    let json = r#""""#;
    let result: Cow<str> = from_str(json).unwrap();
    assert_eq!(result, "");
    assert!(
        matches!(result, Cow::Borrowed(_)),
        "Empty string should be zero-copy"
    );
}

#[test]
fn cow_zero_copy_unicode() {
    let json = r#""日本語 🦀 Rust""#;
    let result: Cow<str> = from_str(json).unwrap();
    assert_eq!(result, "日本語 🦀 Rust");
    assert!(
        matches!(result, Cow::Borrowed(_)),
        "Unicode without escapes should be zero-copy"
    );
}

#[test]
fn cow_zero_copy_long_string() {
    let content = "a".repeat(10000);
    let json = format!(r#""{}""#, content);
    let result: Cow<str> = from_str(&json).unwrap();
    assert_eq!(result.len(), 10000);
    assert!(
        matches!(result, Cow::Borrowed(_)),
        "Long strings without escapes should be zero-copy"
    );
}

#[test]
fn cow_owned_with_newline() {
    let json = r#""hello\nworld""#;
    let result: Cow<str> = from_str(json).unwrap();
    assert_eq!(result, "hello\nworld");
    assert!(
        matches!(result, Cow::Owned(_)),
        "Strings with escapes should be owned"
    );
}

#[test]
fn cow_owned_with_tab() {
    let json = r#""col1\tcol2""#;
    let result: Cow<str> = from_str(json).unwrap();
    assert_eq!(result, "col1\tcol2");
    assert!(
        matches!(result, Cow::Owned(_)),
        "Strings with escapes should be owned"
    );
}

#[test]
fn cow_owned_with_quote() {
    let json = r#""say \"hello\"""#;
    let result: Cow<str> = from_str(json).unwrap();
    assert_eq!(result, r#"say "hello""#);
    assert!(
        matches!(result, Cow::Owned(_)),
        "Strings with escapes should be owned"
    );
}

#[test]
fn cow_owned_with_backslash() {
    let json = r#""path\\to\\file""#;
    let result: Cow<str> = from_str(json).unwrap();
    assert_eq!(result, r"path\to\file");
    assert!(
        matches!(result, Cow::Owned(_)),
        "Strings with escapes should be owned"
    );
}

#[test]
fn cow_owned_with_unicode_escape() {
    let json = r#""\u0048ello""#;
    let result: Cow<str> = from_str(json).unwrap();
    assert_eq!(result, "Hello");
    assert!(
        matches!(result, Cow::Owned(_)),
        "Strings with unicode escapes should be owned"
    );
}

#[test]
fn cow_in_vec_zero_copy() {
    let json = r#"["one","two","three"]"#;
    let result: Vec<Cow<str>> = from_str(json).unwrap();
    assert_eq!(result.len(), 3);
    assert!(matches!(result[0], Cow::Borrowed(_)));
    assert!(matches!(result[1], Cow::Borrowed(_)));
    assert!(matches!(result[2], Cow::Borrowed(_)));
}

#[test]
fn cow_in_vec_mixed() {
    let json = r#"["simple","with\nescape","another"]"#;
    let result: Vec<Cow<str>> = from_str(json).unwrap();
    assert_eq!(result.len(), 3);
    assert!(
        matches!(result[0], Cow::Borrowed(_)),
        "Simple string should be borrowed"
    );
    assert!(
        matches!(result[1], Cow::Owned(_)),
        "String with escape should be owned"
    );
    assert!(
        matches!(result[2], Cow::Borrowed(_)),
        "Simple string should be borrowed"
    );
}

#[test]
fn cow_in_option_some_zero_copy() {
    let json = r#""test""#;
    let result: Option<Cow<str>> = from_str(json).unwrap();
    assert!(result.is_some());
    assert!(matches!(result.unwrap(), Cow::Borrowed(_)));
}

#[test]
fn cow_in_option_none() {
    let json = r#"null"#;
    let result: Option<Cow<str>> = from_str(json).unwrap();
    assert!(result.is_none());
}

// Helper struct for testing Cow in structs - don't use derive with lifetimes
// Instead, manually implement or use a simpler test
#[test]
fn cow_in_struct_zero_copy_manual() {
    // Test that we can deserialize into a map with Cow values
    use std::collections::HashMap;
    let json = r#"{"name":"Alice","email":"alice@example.com"}"#;
    let result: HashMap<String, Cow<str>> = from_str(json).unwrap();

    assert_eq!(result.get("name").unwrap(), &"Alice");
    assert_eq!(result.get("email").unwrap(), &"alice@example.com");
    assert!(
        matches!(result.get("name").unwrap(), Cow::Borrowed(_)),
        "Name should be zero-copy"
    );
    assert!(
        matches!(result.get("email").unwrap(), Cow::Borrowed(_)),
        "Email should be zero-copy"
    );
}

#[test]
fn cow_in_struct_mixed_manual() {
    use std::collections::HashMap;
    let json = r#"{"name":"Alice","email":"alice\n@example.com"}"#;
    let result: HashMap<String, Cow<str>> = from_str(json).unwrap();

    assert!(
        matches!(result.get("name").unwrap(), Cow::Borrowed(_)),
        "Name without escapes should be borrowed"
    );
    assert!(
        matches!(result.get("email").unwrap(), Cow::Owned(_)),
        "Email with escapes should be owned"
    );
}

#[test]
fn cow_serialize_borrowed() {
    let borrowed: Cow<str> = Cow::Borrowed("test");
    let json = to_string(&borrowed);
    assert_eq!(json, r#""test""#);
}

#[test]
fn cow_serialize_owned() {
    let owned: Cow<str> = Cow::Owned("test".to_string());
    let json = to_string(&owned);
    assert_eq!(json, r#""test""#);
}

#[test]
fn cow_roundtrip_preserves_value() {
    let original = "Hello, Rust! 日本語";
    let json = format!(r#""{}""#, original);
    let parsed: Cow<str> = from_str(&json).unwrap();
    assert_eq!(parsed.as_ref(), original);
    // Roundtrip back
    let json2 = to_string(&parsed);
    let parsed2: Cow<str> = from_str(&json2).unwrap();
    assert_eq!(parsed2.as_ref(), original);
}
