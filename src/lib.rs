use axum::{
    body::{to_bytes, Body},
    extract::{
        ws::{Message, WebSocket},
        FromRequestParts, Request, WebSocketUpgrade,
    },
    handler::Handler,
    http::request::Parts,
    response::{Html, IntoResponse, Response},
    routing::{get, MethodRouter},
    Extension,
};
use std::{
    collections::HashMap,
    convert::Infallible,
    fmt::Display,
    future::Future,
    marker::PhantomData,
    ops::{Add, Deref},
    pin::Pin,
    sync::Arc,
};
use tokio_util::task::LocalPoolHandle;

#[derive(Default)]
pub struct Element {
    content: String,
}

#[derive(Default)]
pub struct ElementParams {
    children: Element,
    attributes: Attributes,
}

#[derive(Default)]
pub struct Attributes {
    pub list: Vec<(String, String)>,
}

impl std::fmt::Display for Attributes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (attribute, value) in &self.list {
            write!(f, " {attribute}=\"{value}\"")?;
        }

        Ok(())
    }
}

#[macro_export]
macro_rules! attrs {
    ( $( ($attr:expr, $value:expr) ),* ) => {
        $crate::Attributes {
            list: vec![$( ($attr.to_string(), $value.to_string()), )*],
        }
    };
}

impl From<()> for Element {
    fn from(_val: ()) -> Self {
        Element {
            content: "".to_string(),
        }
    }
}
impl From<&'static str> for Element {
    fn from(val: &'static str) -> Self {
        Element {
            content: val.to_string(),
        }
    }
}
impl<T: Display> From<State<T>> for Element {
    fn from(value: State<T>) -> Self {
        Element {
            content: format!(
                "<span data-coaxial-id=\"{}\">{}</span>",
                value.id, value.value
            ),
        }
    }
}

impl From<()> for ElementParams {
    fn from(_val: ()) -> Self {
        ElementParams::default()
    }
}
impl From<&'static str> for ElementParams {
    fn from(children: &'static str) -> Self {
        ElementParams {
            children: children.into(),
            attributes: Attributes::default(),
        }
    }
}
impl From<Element> for ElementParams {
    fn from(children: Element) -> Self {
        ElementParams {
            children,
            attributes: Attributes::default(),
        }
    }
}
impl<T: Display> From<State<T>> for ElementParams {
    fn from(state: State<T>) -> Self {
        ElementParams {
            children: state.into(),
            attributes: Attributes::default(),
        }
    }
}

impl From<Vec<(String, String)>> for Attributes {
    fn from(list: Vec<(String, String)>) -> Self {
        Self { list }
    }
}

impl<C: Into<Element>, A: Into<Attributes>> From<(C, A)> for ElementParams {
    fn from((children, attributes): (C, A)) -> Self {
        ElementParams {
            children: children.into(),
            attributes: attributes.into(),
        }
    }
}

impl Add<Self> for Element {
    type Output = Self;

    fn add(mut self, rhs: Element) -> Self::Output {
        self.content.push_str(&rhs.content);
        self
    }
}

macro_rules! make_element {
    ($ident:ident) => {
        pub fn $ident(params: impl Into<ElementParams>) -> Element {
            let ElementParams {
                mut children,
                attributes,
            } = params.into();

            let attributes = attributes.to_string();

            children.content = format!(
                "<{}{attributes}>{}</{}>",
                stringify!($ident),
                children.content,
                stringify!($ident)
            );

            children
        }
    };
}

make_element!(div);
make_element!(p);
make_element!(button);
make_element!(html);
make_element!(body);
make_element!(head);

pub fn slot() -> Element {
    Element {
        content: "{^slot^}".to_string(),
    }
}

trait AsyncFn: Send + Sync {
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

pub struct Context {
    uuid: String,
    index: u64,
    closures: HashMap<String, Arc<dyn AsyncFn>>,
}

impl Context {
    pub fn new() -> Self {
        Self {
            // TODO generate something random
            uuid: "hi".to_string(),
            index: 0,
            closures: Default::default(),
        }
    }

    pub fn use_closure<F, Fut>(&mut self, closure: F) -> Closure
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.index += 1;
        let id = format!("{}-{}", self.uuid, self.index);
        self.closures.insert(id.clone(), Arc::new(closure));

        Closure { id }
    }

    pub fn use_state<T>(&mut self, value: T) -> State<T> {
        self.index += 1;
        State {
            value,
            id: format!("{}-{}", self.uuid, self.index),
        }
    }

    pub fn with(self, element: Element) -> CoaxialResponse {
        Response::new(Output {
            element,
            context: self,
        })
    }
}

pub struct State<T> {
    value: T,
    id: String,
}

// TODO we can do something live bevy's change detection with the DerefMut
// https://docs.rs/bevy_ecs/0.13.2/src/bevy_ecs/change_detection.rs.html#485
impl<T> Deref for State<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> State<T> {
    pub fn new(value: T, id: String) -> Self {
        Self { value, id }
    }

    pub fn get(&self) -> &T {
        &self.value
    }

    pub fn set(&self, value: T) {
        // TODO so uhh how do we send a message about the update here?
        // one option we have is to set the value on the use_state,
        // and then rerun the function from scratch
        // idk tho
        todo!()
    }
}

pub struct Closure {
    id: String,
}
impl Display for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "window.Coaxial.callClosure('{}')", self.id)
    }
}

pub type CoaxialResponse = Response<Output>;
pub struct Output {
    element: Element,
    context: Context,
}

#[derive(Clone)]
pub struct Coaxial {
    layout: String,
}

impl Coaxial {
    pub fn with_layout(layout: Element) -> Extension<Self> {
        let mut layout = layout.content;
        layout.push_str(include_str!("base.html"));
        Extension(Coaxial { layout })
    }
}

pub trait CoaxialHandler<S>: Clone + Send + Sized + 'static {
    type Future: Future<Output = CoaxialResponse> + Send + 'static;
    fn call(self, req: Request, state: S) -> Self::Future;
}

// TODO implement handler for the basic func that takes only the context
impl<F, Fut, S> CoaxialHandler<S> for F
where
    F: FnOnce(Context) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = CoaxialResponse> + Send,
    // TODO we can add an IntoCoaxialResponse here
{
    type Future = Pin<Box<dyn Future<Output = CoaxialResponse> + Send>>;

    fn call(self, _req: Request, _state: S) -> Self::Future {
        Box::pin(async move { self(Context::new()).await })
    }
}

pub fn live<H, S>(handler: H) -> MethodRouter<S>
where
    H: CoaxialHandler<S>,
    S: Clone + Send + Sync + 'static,
{
    get(
        |axum::extract::State(state): axum::extract::State<S>,
         Extension(config): Extension<Coaxial>,
         request: Request| {
            let is_websocket = request
                .headers()
                .get("Upgrade")
                .and_then(|v| v.to_str().ok())
                == Some("websocket");

            async move {
                if !is_websocket {
                    let response = handler.call(request, state).await;

                    let (parts, body) = response.into_parts();

                    let output = config
                        .layout
                        .clone()
                        .replace("{^slot^}", &body.element.content);

                    return axum::response::Response::from_parts(parts, Body::from(output));
                }

                let (mut parts, body) = request.into_parts();
                let ws = WebSocketUpgrade::from_request_parts(&mut parts, &state)
                    .await
                    .unwrap();
                let request = Request::from_parts(parts, body);

                let response = handler.call(request, state).await;

                ws.on_upgrade(|mut socket: WebSocket| async move {
                    // if let Some(Ok(_)) = socket.recv().await {
                    //     let bytes = response.body().content.as_bytes();
                    //     let msg = axum::extract::ws::Message::Binary(bytes.to_vec());

                    //     if socket.send(msg).await.is_err() {
                    //         // client disconnected
                    //         return;
                    //     }
                    // }

                    let context = &response.body().context;
                    let pool = LocalPoolHandle::new(5);

                    while let Some(msg) = socket.recv().await {
                        let msg: SocketMessage = match msg {
                            Ok(Message::Text(msg)) => serde_json::from_str(&msg).unwrap(),
                            Ok(_) => {
                                continue;
                            }
                            Err(_) => {
                                // client disconnected
                                return;
                            }
                        };

                        // TODO run closure
                        match msg {
                            SocketMessage::Closure { closure } => {
                                let Some(closure) = context.closures.get(&closure) else {
                                    continue;
                                };
                                pool.spawn_pinned(|| closure.call()).await.unwrap();
                            }
                        }

                        let msg = axum::extract::ws::Message::Text(
                            "{ \"t\": \"update\", \"fields\": [[\"hi-1\", 1]] }".to_string(),
                        );
                        if socket.send(msg).await.is_err() {
                            // client disconnected
                            return;
                        }
                    }
                })
                .into_response()
            }
        },
    )
}

#[derive(serde::Deserialize)]
#[serde(tag = "t")]
enum SocketMessage {
    Closure { closure: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let el = div(p("hello") + p("world"));

        assert_eq!(el.content, "<div><p>hello</p><p>world</p></div>");
    }

    #[test]
    fn test_attributes() {
        let el = div(("hello", vec![("hi".to_string(), "test".to_string())]));

        assert_eq!(el.content, "<div hi=\"test\">hello</div>");
    }

    #[test]
    fn test_attributes_macro() {
        let el = div(("hello", attrs![("hi", "test")]));

        assert_eq!(el.content, "<div hi=\"test\">hello</div>");
    }

    // #[test]
    // fn test_state() {
    //     let mut ctx = Context::new();

    //     let s = ctx.use_state(0u32);

    //     let el = div(s);

    //     assert_eq!(el.content, "<div hi=\"test\">hello</div>");
    // }
}
