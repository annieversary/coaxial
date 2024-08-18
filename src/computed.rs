use std::{collections::HashMap, fmt::Display, future::Future, pin::Pin, sync::Arc};

use serde::de::DeserializeOwned;
use tokio::task::JoinSet;

use crate::{random_id::RandomId, state::State};

pub(crate) type OnChangeHandler = Arc<dyn Fn() + 'static + Send + Sync>;
pub(crate) type OnChangeHandlerAsync =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send + Sync>> + Send + Sync>;

#[derive(Default)]
pub(crate) struct ComputedStates {
    on_change_handler: HashMap<RandomId, Vec<OnChangeHandler>>,
    on_change_handler_async: HashMap<RandomId, Vec<OnChangeHandlerAsync>>,

    join_set: JoinSet<()>,
}

impl ComputedStates {
    pub(crate) fn add_computed<O, I, F>(
        &mut self,
        state: State<O>,
        states: I,
        compute: F,
    ) -> ComputedState<O>
    where
        O: DeserializeOwned + Display + Send + Sync + 'static,
        I: StateGetter + Send + Sync + 'static,
        F: Fn(<I as StateGetter>::Output) -> O + Send + Sync + 'static,
    {
        let compute = Arc::new(compute);
        for id in states.id_list() {
            let compute = compute.clone();
            let states = states.clone();
            let on_change_listener = move || {
                state.set(compute(states.get()));
            };

            if let Some(value) = self.on_change_handler.get_mut(&id) {
                value.push(Arc::new(on_change_listener));
            } else {
                self.on_change_handler
                    .insert(id, vec![Arc::new(on_change_listener)]);
            }
        }

        ComputedState(state)
    }

    pub(crate) fn add_computed_async<O, I, F, FUT>(
        &mut self,
        state: State<O>,
        states: I,
        compute: F,
        immediately_recompute: bool,
    ) -> ComputedState<O>
    where
        O: DeserializeOwned + Display + Send + Sync + 'static,
        I: StateGetter,
        F: Fn(<I as StateGetter>::Output) -> FUT + Send + Sync + 'static,
        FUT: Future<Output = O> + Send + Sync + 'static,
    {
        let compute = Arc::new(compute);
        let _states = states.clone();
        let on_change_listener: OnChangeHandlerAsync = Arc::new(move || {
            let compute = compute.clone();
            let states = _states.clone();
            Box::pin(async move {
                state.set(compute(states.get()).await);
            })
        });

        for id in states.id_list() {
            if let Some(value) = self.on_change_handler_async.get_mut(&id) {
                value.push(on_change_listener.clone());
            } else {
                self.on_change_handler_async
                    .insert(id, vec![on_change_listener.clone()]);
            }
        }

        if immediately_recompute {
            self.join_set.spawn(on_change_listener());
        }

        ComputedState(state)
    }

    /// Recompute sync ComputedStates that depend on the state with id `id`
    pub(crate) fn recompute_dependents(&mut self, id: RandomId) {
        if let Some(funcs) = self.on_change_handler.get(&id) {
            for func in funcs {
                (*func)();
            }
        }

        if let Some(async_funcs) = self.on_change_handler_async.get(&id) {
            for func in async_funcs {
                self.join_set.spawn((*func)());
            }
        }
    }
}

pub enum InitialValue<O> {
    /// Set the initial value.
    Value(O),
    /// Set the initial value, and recompute in the background.
    ValueAndCompute(O),
}

// States

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

#[cfg(test)]
mod tests {
    use crate::{computed::InitialValue, context::Context};

    #[test]
    fn test_u32_computed_state() {
        let mut ctx = Context::<()>::new(0, true);

        let state = ctx.use_state(0u32);

        let computed = ctx.use_computed(state, |value| value + 1);

        assert_eq!(1, computed.get());
    }

    #[test]
    fn test_string_computed_state() {
        let mut ctx = Context::<()>::new(0, true);

        let state = ctx.use_state(0u32);

        let computed = ctx.use_computed(state, |value| value.to_string());

        assert_eq!("0", computed.get());
    }

    #[test]
    fn test_string_computed_state_async_uses_initial_value() {
        let mut ctx = Context::<()>::new(0, true);

        let state = ctx.use_state(0u32);

        let computed = ctx.use_computed_async_with(
            state,
            |value| async move { value.to_string() },
            InitialValue::Value("initial".to_string()),
        );

        assert_eq!("initial", computed.get());
    }

    /// Using an async computed state with a ValueAndCompute causes the value to be immediately recomputed in the background
    #[tokio::test]
    async fn test_async_computed_state_immediate_recompute() {
        let mut ctx = Context::<()>::new(0, true);

        let state = ctx.use_state(0u32);
        let computed = ctx.use_computed_async_with(
            state,
            |value| async move { value.to_string() },
            InitialValue::ValueAndCompute("initial".to_string()),
        );

        assert_eq!("initial", computed.get());

        ctx.computed_states
            .join_set
            .join_next()
            .await
            .unwrap()
            .unwrap();

        assert_eq!("0", computed.get());
    }

    #[tokio::test]
    async fn test_async_computed_state() {
        let mut ctx = Context::<()>::new(0, true);

        let state = ctx.use_state(0u32);
        let computed = ctx
            .use_computed_async(state, |value| async move { value.to_string() })
            .await;

        assert_eq!("0", computed.get());
    }

    #[test]
    fn test_sync_gets_recomputed() {
        let mut ctx = Context::<()>::new(0, true);

        let state = ctx.use_state(0u32);
        let computed = ctx.use_computed(state, |value| value.to_string());

        assert_eq!("0", computed.get());

        state.set(1);

        ctx.computed_states.recompute_dependents(state.id);

        assert_eq!("1", computed.get());
    }

    #[tokio::test]
    async fn test_async_gets_recomputed() {
        let mut ctx = Context::<()>::new(0, true);

        let state = ctx.use_state(0u32);
        let computed = ctx.use_computed_async_with(
            state,
            |value| async move { value.to_string() },
            InitialValue::Value("initial".to_string()),
        );

        state.set(1);

        ctx.computed_states.recompute_dependents(state.id);

        ctx.computed_states
            .join_set
            .join_next()
            .await
            .unwrap()
            .unwrap();

        assert_eq!("1", computed.get());
    }
}
