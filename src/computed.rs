use crate::{random_id::RandomId, state::State};

pub struct ComputedState<T: 'static>(pub(crate) State<T>);

// we implement Copy and Clone instead of deriving them, cause we dont need the
// `T: Clone` bound
impl<T: 'static> Clone for ComputedState<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: 'static> Copy for ComputedState<T> {}

impl<T: Clone + Send + Sync + 'static> ComputedState<T> {
    pub fn get(&self) -> T {
        self.0.get()
    }
}

pub trait StateGetter: Clone + Send + Sync + 'static {
    type Output;

    fn get(&self) -> Self::Output;

    fn id_list(&self) -> impl Iterator<Item = RandomId>;
}

impl<T: Clone + Send + Sync + 'static> StateGetter for State<T> {
    type Output = T;

    fn get(&self) -> Self::Output {
        State::get(self)
    }

    fn id_list(&self) -> impl Iterator<Item = RandomId> {
        [self.id].into_iter()
    }
}

impl<T, U> StateGetter for (State<T>, State<U>)
where
    T: Clone + Send + Sync + 'static,
    U: Clone + Send + Sync + 'static,
{
    type Output = (T, U);

    fn get(&self) -> Self::Output {
        (State::get(&self.0), State::get(&self.1))
    }

    fn id_list(&self) -> impl Iterator<Item = RandomId> {
        [self.0.id, self.1.id].into_iter()
    }
}
