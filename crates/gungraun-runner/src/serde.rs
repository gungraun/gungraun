//! Custom `serde` serializer and deserializer implementations

/// To prevent serializing f64 values inf, -inf, NaN into a null value, serialize f64 as string.
/// That way the reverse operation retains the original value.
pub mod float_64 {
    use std::str::FromStr;

    use serde::de::Visitor;
    use serde::{Deserializer, Serializer};

    struct FieldVisitor;

    impl Visitor<'_> for FieldVisitor {
        type Value = f64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string with a f64 value")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            f64::from_str(v).map_err(|error| serde::de::Error::custom(error.to_string()))
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_str(&v)
        }
    }

    /// Deserializes a `String` into a `f64`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(FieldVisitor)
    }

    /// Serializes `f64` into a `String`.
    pub fn serialize<S>(input: &f64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&input.to_string())
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Serialize, Deserialize)]
    struct ValueFixture {
        #[serde(with = "float_64")]
        value: f64,
    }

    #[rstest]
    #[case::zero(0.0f64, "0")]
    #[case::one(1.0f64, "1")]
    #[case::neg_one(-1.0f64, "-1")]
    #[case::two_two(2.2f64, "2.2")]
    #[case::f64_max(f64::MAX,
        "17976931348623157000000000000000000000000000000000000000000000000000000000000000000000000\
        000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
        000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
        0000000000000000000000000000000000000000")]
    #[case::f64_min(f64::MIN,
        "-1797693134862315700000000000000000000000000000000000000000000000000000000000000000000000\
        000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
        000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\
        00000000000000000000000000000000000000000")]
    #[case::neg_inf(f64::NEG_INFINITY, "-inf")]
    #[case::pos_inf(f64::INFINITY, "inf")]
    fn test_serde_f64_round_trip(#[case] value: f64, #[case] expected: &str) {
        assert_round_trip_eq(value, expected);
    }

    #[track_caller]
    fn assert_round_trip_eq(value: f64, expected: &str) {
        assert_eq!(value, round_trip(value, expected));
    }

    #[track_caller]
    fn round_trip(value: f64, expected: &str) -> f64 {
        let serialized = serde_json::to_string(&ValueFixture { value }).unwrap();
        assert_eq!(serialized, format!(r#"{{"value":"{expected}"}}"#));
        serde_json::from_str::<ValueFixture>(&serialized)
            .unwrap()
            .value
    }

    #[test]
    fn test_serde_f64_nan() {
        assert!(round_trip(f64::NAN, "NaN").is_nan());
    }
}
