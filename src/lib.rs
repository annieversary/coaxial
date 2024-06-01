use axum::{
    body::{to_bytes, Body},
    extract::{ws::WebSocket, FromRequestParts, Request, WebSocketUpgrade},
    handler::Handler,
    http::request::Parts,
    response::{Html, IntoResponse},
    routing::{get, MethodRouter},
    Extension,
};
use std::{
    collections::HashMap,
    convert::Infallible,
    fmt::Display,
    future::Future,
    ops::{Add, Deref},
    pin::Pin,
};

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

pub trait Stated {
    type State;

    fn to_state(&self) -> Self::State;
}

type AsyncClosure<'a, D> = Box<dyn Fn(&'a mut D) -> Pin<Box<dyn Future<Output = ()> + 'a>>>;

pub struct Context<'a, D: Default + Stated> {
    uuid: String,
    index: u64,
    closures: HashMap<String, AsyncClosure<'a, D>>,
}

impl<'a, D: Default + Stated> Context<'a, D> {
    pub fn new() -> Self {
        let data: D = Default::default();
        Self {
            uuid: "hi".to_string(),
            index: 0,
            closures: Default::default(),

            state: data.to_state(),
            data,
        }
    }

    pub fn use_closure<F, Fut>(&'a mut self, closure: F) -> Closure
    where
        D: 'a,
        F: Fn(&'a mut D) -> Fut + 'static,
        Fut: Future<Output = ()> + Send + 'a,
    {
        self.index += 1;
        let id = format!("{}-{}", self.uuid, self.index);
        self.closures.insert(
            id.clone(),
            Box::new(move |d| Box::pin(closure(d)) as Pin<Box<dyn Future<Output = ()> + 'a>>)
                as AsyncClosure<D>,
        );

        Closure { id }
    }

    pub fn with(self, element: Element) -> Response {
        Response {
            content: div((element, attrs!(("id", "app")))).content,
        }
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
        write!(f, "alert('unimplemmented')")
    }
}

#[axum::async_trait]
impl<'a, S, D: Default + Stated> FromRequestParts<S> for Context<'a, D> {
    type Rejection = Infallible;

    async fn from_request_parts(_parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Context::new())
    }
}

pub struct Response {
    content: String,
}

impl IntoResponse for Response {
    fn into_response(self) -> axum::response::Response {
        Html(self.content).into_response()
    }
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

pub fn live<H, T, S>(handler: H) -> MethodRouter<S>
where
    H: Handler<T, S>,
    T: 'static,
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

                    let body =
                        String::from_utf8(to_bytes(body, 100_000_000).await.unwrap().to_vec())
                            .unwrap();
                    let output = config.layout.clone().replace("{^slot^}", &body);

                    return axum::response::Response::from_parts(parts, Body::from(output));
                }

                let (mut parts, body) = request.into_parts();
                let ws = WebSocketUpgrade::from_request_parts(&mut parts, &state)
                    .await
                    .unwrap();
                let request = Request::from_parts(parts, body);

                let response = handler.call(request, state).await;

                let body = to_bytes(response.into_body(), 100_000_000)
                    .await
                    .unwrap()
                    .to_vec();

                ws.on_upgrade(|mut socket: WebSocket| async move {
                    if let Some(Ok(_)) = socket.recv().await {
                        // TODO we could do streaming here
                        let msg = axum::extract::ws::Message::Binary(body);

                        if socket.send(msg).await.is_err() {
                            // client disconnected
                            return;
                        }
                    }

                    // todo runs the handler and like. prepares a Thing that can deal with the things
                    while let Some(msg) = socket.recv().await {
                        let msg = if let Ok(msg) = msg {
                            msg
                        } else {
                            // client disconnected
                            return;
                        };

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
