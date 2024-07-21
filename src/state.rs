use generational_box::{GenerationalBox, SyncStorage};
use serde::de::DeserializeOwned;
use serde::{de, Deserialize, Deserializer};
use std::{fmt::Display, str::FromStr};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct StateId(pub(crate) u64, pub(crate) u64);

impl FromStr for StateId {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut it = s.split('-');
        let first = it
            .next()
            .ok_or("No number found")?
            .parse()
            .map_err(|_| "First was not a valid number")?;
        let second = it
            .next()
            .ok_or("No number found")?
            .parse()
            .map_err(|_| "Second was not a valid number")?;

        Ok(StateId(first, second))
    }
}

impl<'de> Deserialize<'de> for StateId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl Display for StateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.0, self.1)
    }
}

#[derive(Copy)]
pub struct State<T: 'static> {
    pub(crate) inner: GenerationalBox<StateInner<T>, SyncStorage>,
    pub(crate) id: StateId,
}

// we implement Clone instead of deriving it, cause we dont need the
// `T: Clone` bound
impl<T: 'static> Clone for State<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner,
            id: self.id,
        }
    }
}

pub(crate) struct StateInner<T: 'static> {
    pub(crate) value: T,
    pub(crate) changes_tx: UnboundedSender<(StateId, String)>,
}

impl<T: Clone + Send + Sync + 'static> State<T> {
    pub fn get(&self) -> T {
        self.inner.read().value.clone()
    }
}

impl<T: Display + Send + Sync + 'static> State<T> {
    pub fn set(&self, value: T) {
        let mut w = self.inner.write();
        w.changes_tx.send((self.id, format!("{value}"))).unwrap();
        w.value = value;
    }
}

pub trait AnyState: Send + Sync + 'static {
    fn set_value(&self, value: serde_json::Value);
}

impl<T: DeserializeOwned + Display + Send + Sync + 'static> AnyState for State<T> {
    fn set_value(&self, value: serde_json::Value) {
        // numbers arrive as strings, so the from_value later doesn't work
        // we manually test inside the string.
        // if it succeeds we set the value, and if it fails we ignore and try the normal deserialize
        if let serde_json::Value::String(s) = &value {
            if let Ok(value) = serde_json::from_str::<T>(s) {
                self.set(value);
                return;
            }
        }

        let value: T = serde_json::from_value(value).unwrap();
        self.set(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_state_id() {
        let s: StateId = "123-456".parse().unwrap();
        assert_eq!(StateId(123, 456), s);
    }
}
