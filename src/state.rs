use generational_box::{GenerationalBox, SyncStorage};
use std::fmt::Display;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone, Copy)]
pub struct State<T: 'static> {
    pub(crate) inner: GenerationalBox<StateInner<T>, SyncStorage>,
    pub(crate) id: u64,
}

pub(crate) struct StateInner<T: 'static> {
    pub(crate) value: T,
    pub(crate) changes_tx: UnboundedSender<(u64, String)>,
}

impl<T: Clone + Display + Send + Sync + 'static> State<T> {
    pub fn get(&self) -> T {
        self.inner.read().value.clone()
    }

    pub fn set(&self, value: T) {
        let mut w = self.inner.write();
        w.changes_tx.send((self.id, format!("{value}"))).unwrap();
        w.value = value;
    }
}
