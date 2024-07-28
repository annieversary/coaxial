use std::{
    array::TryFromSliceError,
    fmt::{Debug, Display, Write},
};

use rand::{distributions::Alphanumeric, Rng};
use serde::{de::Deserializer, Deserialize};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RandomId([u8; 8]);

impl RandomId {
    pub fn from_rng<RNG: Rng>(rng: &mut RNG) -> Self {
        let array = [(); 8].map(|_| rng.sample(Alphanumeric));

        Self(array)
    }

    #[allow(dead_code)]
    pub(crate) fn from_str(string: &str) -> Self {
        let array: [u8; 8] = string.as_bytes()[..8]
            .try_into()
            .expect("provided string was less than 8 characters long");
        Self(array)
    }

    pub(crate) fn try_from_str(string: &str) -> Result<Self, TryFromSliceError> {
        let array: [u8; 8] = string.as_bytes()[..8].try_into()?;
        Ok(Self(array))
    }

    pub fn fmt(&self, output: &mut dyn Write) -> std::fmt::Result {
        for c in self.0 {
            output.write_char(char::from(c))?;
        }

        Ok(())
    }
}

impl Debug for RandomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RandomId(")?;

        Self::fmt(self, f)?;

        f.write_char(')')?;

        Ok(())
    }
}

impl Display for RandomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Self::fmt(self, f)
    }
}

impl<'de> Deserialize<'de> for RandomId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, Visitor};

        struct RandomIdVisitor;

        impl<'de> Visitor<'de> for RandomIdVisitor {
            type Value = RandomId;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a string no more than 8 bytes long")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                RandomId::try_from_str(v).map_err(|_| E::invalid_length(v.len(), &self))
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let s = std::str::from_utf8(v)
                    .map_err(|_| E::invalid_value(de::Unexpected::Bytes(v), &self))?;

                RandomId::try_from_str(s).map_err(|_| E::invalid_length(s.len(), &self))
            }
        }

        deserializer.deserialize_str(RandomIdVisitor)
    }
}
