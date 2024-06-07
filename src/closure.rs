use std::{fmt::Display, future::Future, pin::Pin};

pub struct Closure {
    pub(crate) id: String,
}
impl Display for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "window.Coaxial.callClosure('{}')", self.id)
    }
}

pub(crate) trait AsyncFn: Send + Sync {
    fn call(&self) -> Pin<Box<dyn Future<Output = ()> + 'static>>;
}

impl<T: Send + Sync, F> AsyncFn for T
where
    T: Fn() -> F,
    F: Future<Output = ()> + 'static,
{
    fn call(&self) -> Pin<Box<dyn Future<Output = ()> + 'static>> {
        Box::pin(self())
    }
}
