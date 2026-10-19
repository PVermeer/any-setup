use anyhow::{Context, Result};
use gtk::glib::Variant;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as DeError};
use std::{
    fmt,
    hash::{Hash, Hasher},
};

#[derive(Clone, Debug)]
pub struct DconfValue(Variant);

impl DconfValue {
    pub fn parse(value: &str) -> Result<Self> {
        Variant::parse(None, value)
            .map(Self)
            .context("Failed to parse GVariant value")
    }

    pub fn as_str(&self) -> String {
        self.0.print(false).to_string()
    }
}

impl fmt::Display for DconfValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.print(false))
    }
}

impl PartialEq for DconfValue {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for DconfValue {}

impl Hash for DconfValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.print(false).hash(state);
    }
}

impl Serialize for DconfValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.as_str())
    }
}

impl<'de> Deserialize<'de> for DconfValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;

        Self::parse(&value).map_err(D::Error::custom)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;

    fn hash(value: &DconfValue) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn parse_boolean() {
        let value = DconfValue::parse("false").unwrap();

        assert_eq!(value.as_str(), "false");
        assert_eq!(value.to_string(), "false");
    }

    #[test]
    fn parse_string() {
        let value = DconfValue::parse("'prefer-dark'").unwrap();

        assert_eq!(value.as_str(), "'prefer-dark'");
        assert_eq!(value.to_string(), "'prefer-dark'");
    }

    #[test]
    fn parse_integer() {
        let value = DconfValue::parse("42").unwrap();

        assert_eq!(value.as_str(), "42");
    }

    #[test]
    fn parse_array() {
        let value = DconfValue::parse("['one', 'two']").unwrap();

        assert_eq!(value.as_str(), "['one', 'two']");
    }

    #[test]
    fn parse_invalid_value_returns_error() {
        let result = DconfValue::parse("not a valid GVariant");

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Failed to parse GVariant value"
        );
    }

    #[test]
    fn equal_values_are_equal() {
        let first = DconfValue::parse("false").unwrap();
        let second = DconfValue::parse("false").unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn different_values_are_not_equal() {
        let first = DconfValue::parse("false").unwrap();
        let second = DconfValue::parse("true").unwrap();

        assert_ne!(first, second);
    }

    #[test]
    fn equal_values_have_equal_hashes() {
        let first = DconfValue::parse("false").unwrap();
        let second = DconfValue::parse("false").unwrap();

        assert_eq!(hash(&first), hash(&second));
    }

    #[test]
    fn display_matches_as_str() {
        let value = DconfValue::parse("'prefer-dark'").unwrap();

        assert_eq!(value.to_string(), value.as_str());
    }

    #[test]
    fn serde_yaml_serializes_as_string() {
        let value = DconfValue::parse("false").unwrap();

        let yaml = serde_yaml::to_string(&value).unwrap();

        assert_eq!(yaml.trim(), "'false'");
    }

    #[test]
    fn serde_yaml_deserializes_boolean_expression() {
        let value: DconfValue = serde_yaml::from_str("\"false\"").unwrap();

        assert_eq!(value.as_str(), "false");
    }

    #[test]
    fn serde_yaml_round_trips_string_expression() {
        let original = DconfValue::parse("'prefer-dark'").unwrap();

        let yaml = serde_yaml::to_string(&original).unwrap();
        let parsed: DconfValue = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed, original);
    }

    #[test]
    fn serde_yaml_rejects_invalid_gvariant() {
        let result = serde_yaml::from_str::<DconfValue>("\"not a valid GVariant\"");

        assert!(result.is_err());
    }

    #[test]
    fn serde_yaml_rejects_non_string_input() {
        let yaml = serde_yaml::Value::Bool(false);
        let result = DconfValue::deserialize(yaml);

        assert!(result.is_err());
    }
}
