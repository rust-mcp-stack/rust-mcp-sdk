use core::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// Represents the audience claim, which can be a single string or a list of strings.
#[derive(Debug, Clone)]
pub enum Audience {
    Single(String),
    Multiple(Vec<String>),
}

impl Audience {
    /// Converts the audience to a Vec<String> for uniform access.
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            Audience::Single(s) => vec![s.clone()],
            Audience::Multiple(v) => v.clone(),
        }
    }
}

impl fmt::Display for Audience {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Audience::Single(s) => write!(f, "{s}"),
            Audience::Multiple(v) => {
                let formatted = v.join(", ");
                write!(f, "{formatted}")
            }
        }
    }
}

impl PartialEq for Audience {
    fn eq(&self, other: &Self) -> bool {
        self.to_vec() == other.to_vec()
    }
}

impl Eq for Audience {}

impl Serialize for Audience {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            // Serialize a single string directly as a JSON string
            Audience::Single(s) => serializer.serialize_str(s),
            // Serialize multiple strings as a JSON array
            Audience::Multiple(v) => serializer.collect_seq(v),
        }
    }
}

impl<'de> Deserialize<'de> for Audience {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Use a Value to handle both string and array cases
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::String(s) => Ok(Audience::Single(s)),
            Value::Array(arr) => {
                let strings = arr
                    .into_iter()
                    .map(|v| match v {
                        Value::String(s) => Ok(s),
                        _ => Err(serde::de::Error::custom(
                            "audience array must contain strings",
                        )),
                    })
                    .collect::<Result<Vec<String>, D::Error>>()?;
                Ok(Audience::Multiple(strings))
            }
            _ => Err(serde::de::Error::custom(
                "audience must be a string or an array of strings",
            )),
        }
    }
}

// Allow converting from &str
impl From<&str> for Audience {
    fn from(s: &str) -> Self {
        Audience::Single(s.to_string())
    }
}

// Allow converting from String
impl From<String> for Audience {
    fn from(s: String) -> Self {
        Audience::Single(s)
    }
}

// Allow converting from Vec<String>
impl From<Vec<String>> for Audience {
    fn from(v: Vec<String>) -> Self {
        Audience::Multiple(v)
    }
}

// Allow converting from Vec<&str> for convenience
impl From<Vec<&str>> for Audience {
    fn from(v: Vec<&str>) -> Self {
        Audience::Multiple(v.into_iter().map(|s| s.to_string()).collect())
    }
}
