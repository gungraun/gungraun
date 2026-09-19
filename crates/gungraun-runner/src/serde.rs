//! Custom `serde` serializer and deserializer implementations

/// Serializes and deserializes [`::either_or_both::EitherOrBoth`] as an object whose keys describe
/// whether a value is new, old, or both.
pub mod either_or_both {
    use std::fmt;
    use std::marker::PhantomData;

    use either_or_both::EitherOrBoth;
    use serde::de::{Error, MapAccess, Visitor};
    use serde::ser::SerializeMap;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    /// Either the new, old or both values are present
    #[cfg(feature = "schema")]
    #[derive(schemars::JsonSchema)]
    #[serde(deny_unknown_fields, untagged)]
    #[expect(
        dead_code,
        reason = "schema-only proxy variants are never instantiated"
    )]
    pub(crate) enum NewOldOrBoth<L, R> {
        /// Both the new and old values are present.
        Both { new: L, old: R },
        /// Only the new value is present.
        New { new: L },
        /// Only the old value is present.
        Old { old: R },
    }

    struct EitherOrBothVisitor<L, R> {
        marker: PhantomData<fn() -> EitherOrBoth<L, R>>,
    }

    impl<L, R> EitherOrBothVisitor<L, R> {
        const fn new() -> Self {
            Self {
                marker: PhantomData,
            }
        }
    }

    impl<'de, L, R> Visitor<'de> for EitherOrBothVisitor<L, R>
    where
        L: Deserialize<'de>,
        R: Deserialize<'de>,
    {
        type Value = EitherOrBoth<L, R>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an object containing a non-null `new`, `old`, or both values")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut new = None;
            let mut old = None;

            while let Some(key) = map.next_key::<String>()? {
                match key.as_str() {
                    "new" if new.is_none() => {
                        new = Some(
                            map.next_value::<Option<L>>()?
                                .ok_or_else(|| Error::custom("`new` must not be null"))?,
                        );
                    }
                    "new" => {
                        return Err(Error::duplicate_field("new"));
                    }
                    "old" if old.is_none() => {
                        old = Some(
                            map.next_value::<Option<R>>()?
                                .ok_or_else(|| Error::custom("`old` must not be null"))?,
                        );
                    }
                    "old" => {
                        return Err(Error::duplicate_field("old"));
                    }
                    field => return Err(Error::unknown_field(field, &["new", "old"])),
                }
            }

            EitherOrBoth::<L, R>::try_from((new, old))
                .map_err(|_| Error::custom("expected `new`, `old`, or both values"))
        }
    }

    /// Deserializes an object containing a `new`, `old`, or both values.
    pub fn deserialize<'de, D, L, R>(deserializer: D) -> Result<EitherOrBoth<L, R>, D::Error>
    where
        D: Deserializer<'de>,
        L: Deserialize<'de>,
        R: Deserialize<'de>,
    {
        deserializer.deserialize_map(EitherOrBothVisitor::new())
    }

    /// Serializes an [`EitherOrBoth`] as an object containing a `new`, `old`, or both values.
    pub fn serialize<S, L, R>(input: &EitherOrBoth<L, R>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        L: Serialize,
        R: Serialize,
    {
        let mut map = serializer.serialize_map(Some(if input.is_both() { 2 } else { 1 }))?;
        match input {
            EitherOrBoth::Left(new) => map.serialize_entry("new", new)?,
            EitherOrBoth::Both(new, old) => {
                map.serialize_entry("new", new)?;
                map.serialize_entry("old", old)?;
            }
            EitherOrBoth::Right(old) => map.serialize_entry("old", old)?,
        }
        map.end()
    }

    #[cfg(test)]
    mod tests {
        use rstest::rstest;

        use super::*;

        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct Fixture {
            #[serde(with = "super")]
            value: EitherOrBoth<u64>,
        }

        #[rstest]
        #[case::empty_object(r#"{"value":{}}"#)]
        #[case::unknown_field(r#"{"value":{"unknown":1}}"#)]
        #[case::duplicate_new(r#"{"value":{"new":1,"new":2}}"#)]
        #[case::duplicate_old(r#"{"value":{"old":1,"old":2}}"#)]
        #[case::null_new(r#"{"value":{"new":null}}"#)]
        #[case::null_old(r#"{"value":{"old":null}}"#)]
        #[case::null_new_and_old(r#"{"value":{"new":null,"old":null}}"#)]
        fn test_rejects_invalid_objects(#[case] input: &str) {
            assert!(serde_json::from_str::<Fixture>(input).is_err(), "{input}");
        }

        #[rstest]
        #[case::new(EitherOrBoth::Left(2), r#"{"value":{"new":2}}"#)]
        #[case::old(EitherOrBoth::Right(1), r#"{"value":{"old":1}}"#)]
        #[case::both(EitherOrBoth::Both(2, 1), r#"{"value":{"new":2,"old":1}}"#)]
        fn test_round_trips_new_old_and_both_values(
            #[case] value: EitherOrBoth<u64>,
            #[case] expected: &str,
        ) {
            let fixture = Fixture { value };
            let serialized = serde_json::to_string(&fixture).unwrap();
            assert_eq!(serialized, expected);
            assert_eq!(
                serde_json::from_str::<Fixture>(&serialized).unwrap(),
                fixture
            );
        }
    }
}

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

    /// FIX: serialize always with at least one decimal ("25.0")
    /// Serializes `f64` into a `String`.
    pub fn serialize<S>(input: &f64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&input.to_string())
    }

    #[cfg(test)]
    mod tests {
        use rstest::rstest;
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize)]
        struct ValueFixture {
            #[serde(with = "super")]
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
}
