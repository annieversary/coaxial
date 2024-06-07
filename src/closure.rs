use std::{fmt::Display, future::Future, pin::Pin};

pub struct Closure {
    pub(crate) id: String,
}
impl Display for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "window.Coaxial.callClosure('{}')", self.id)
    }
}

pub(crate) trait AsyncFn<P>: Send + Sync {
    fn call(&self, params: P) -> Pin<Box<dyn Future<Output = ()> + 'static>>;
}

impl<T: Send + Sync, F> AsyncFn<()> for T
where
    T: Fn() -> F,
    F: Future<Output = ()> + 'static,
{
    fn call(&self, _params: ()) -> Pin<Box<dyn Future<Output = ()> + 'static>> {
        Box::pin(self())
    }
}

macro_rules! impl_async_fn {
    (
        $($ty:ident),*
    ) => {
        #[allow(non_snake_case)]
        impl<T: Send + Sync, F, $($ty,)*> AsyncFn<($($ty,)*)> for T
        where
            T: Fn($($ty,)*) -> F,
            F: Future<Output = ()> + 'static,
        {
            fn call(&self, ($($ty,)*): ($($ty,)*)) -> Pin<Box<dyn Future<Output = ()> + 'static>> {
                Box::pin(self($($ty,)*))
            }
        }
    };
}

#[rustfmt::skip]
macro_rules! all_the_tuples {
    ($name:ident) => {
        $name!(T1);
        $name!(T1, T2);
        $name!(T1, T2, T3);
        $name!(T1, T2, T3, T4);
        $name!(T1, T2, T3, T4, T5);
        $name!(T1, T2, T3, T4, T5, T6);
        $name!(T1, T2, T3, T4, T5, T6, T7);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
        $name!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);
    };
}

all_the_tuples!(impl_async_fn);
