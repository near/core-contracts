use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Common utility for JSON serialization to use strings.
#[derive(Debug, Default)]
pub struct U128(u128);

impl From<u128> for U128 {
    fn from(v: u128) -> Self {
        Self(v)
    }
}

impl From<U128> for u128 {
    fn from(v: U128) -> u128 {
        v.0
    }
}

impl PartialEq<u128> for U128 {
    fn eq(&self, other: &u128) -> bool {
        &self.0 == other
    }
}

impl Serialize for U128 {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for U128 {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        Ok(Self(u128::from_str_radix(&s, 10).map_err(|err| {
            serde::de::Error::custom(err.to_string())
        })?))
    }
}
