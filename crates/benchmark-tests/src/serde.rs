use serde::de::Visitor;
use serde::ser::SerializeTuple;
use serde::{Deserializer, Serializer};

pub mod rust_version {
    use super::{Deserializer, SerializeTuple, Serializer, Visitor};

    pub type VersionComparator = (version_compare::Cmp, String);

    pub fn serialize<S>(input: &Option<VersionComparator>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match input {
            Some(input) => {
                let mut ser = serializer.serialize_tuple(2)?;
                ser.serialize_element(match input.0 {
                    version_compare::Cmp::Eq => "=",
                    version_compare::Cmp::Ne => "!=",
                    version_compare::Cmp::Lt => "<",
                    version_compare::Cmp::Le => "<=",
                    version_compare::Cmp::Ge => ">=",
                    version_compare::Cmp::Gt => ">",
                })?;
                ser.serialize_element(&input.1)?;
                ser.end()
            }
            None => serializer.serialize_none(),
        }
    }

    struct FieldVisitor;

    impl Visitor<'_> for FieldVisitor {
        type Value = VersionComparator;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string starting with >,>=,<,<=,=,!= followed by a version")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let value = value.trim();
            if let Some(suffix) = value.strip_prefix(">=") {
                Ok((version_compare::Cmp::Ge, suffix.trim_start().to_owned()))
            } else if let Some(suffix) = value.strip_prefix(">") {
                Ok((version_compare::Cmp::Gt, suffix.trim_start().to_owned()))
            } else if let Some(suffix) = value.strip_prefix("<=") {
                Ok((version_compare::Cmp::Le, suffix.trim_start().to_owned()))
            } else if let Some(suffix) = value.strip_prefix("<") {
                Ok((version_compare::Cmp::Lt, suffix.trim_start().to_owned()))
            } else if let Some(suffix) = value.strip_prefix("=") {
                Ok((version_compare::Cmp::Eq, suffix.trim_start().to_owned()))
            } else if let Some(suffix) = value.strip_prefix("!=") {
                Ok((version_compare::Cmp::Ne, suffix.trim_start().to_owned()))
            } else {
                Err(serde::de::Error::custom("invalid input"))
            }
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_str(&v)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<VersionComparator>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(FieldVisitor).map(Some)
    }
}

pub mod runs_on {
    use serde::ser::SerializeTuple;

    use super::{Deserializer, Serializer, Visitor};

    pub type RunsOn = (bool, String);

    pub fn serialize<S>(input: &Option<RunsOn>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match input {
            Some(value) => {
                let mut ser = serializer.serialize_tuple(2)?;
                ser.serialize_element(if value.0 { "" } else { "!" })?;
                ser.serialize_element(&value.1)?;
                ser.end()
            }
            None => serializer.serialize_none(),
        }
    }

    struct FieldVisitor;

    impl Visitor<'_> for FieldVisitor {
        type Value = RunsOn;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string maybe starting with ! and followed by a target")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let value = value.trim();
            if let Some(suffix) = value.strip_prefix("!") {
                Ok((false, suffix.trim_start().to_owned()))
            } else {
                Ok((true, value.to_owned()))
            }
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_str(&v)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<RunsOn>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(FieldVisitor).map(Some)
    }
}
