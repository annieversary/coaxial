use std::{collections::HashMap, sync::Arc};

use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket},
        FromRequestParts, Query, Request, WebSocketUpgrade,
    },
    http::request::Parts,
    routing::{get, MethodRouter},
    Extension,
};
use rand::random;
use tokio::select;
use tokio_util::task::LocalPoolHandle;

use crate::{
    closure::ClosureTrait,
    config::Config,
    context::Context,
    event_handlers::EventHandler,
    handler::CoaxialHandler,
    html::DOCTYPE_HTML,
    state::{AnyState, StateId},
};

pub fn live<T, H, S>(handler: H) -> MethodRouter<S>
where
    H: CoaxialHandler<T, S>,
    S: Clone + Send + Sync + 'static,
{
    get(
        |axum::extract::State(state): axum::extract::State<S>,
         config: Option<Extension<Config>>,
         Query(query): Query<HashMap<String, String>>,
         request: Request| {
            let config = config.map(|c| c.0).unwrap_or_default();

            let is_websocket = request
                .headers()
                .get("Upgrade")
                .and_then(|v| v.to_str().ok())
                == Some("websocket");

            async move {
                if !is_websocket {
                    let rng_seed: u64 = random();

                    let response = handler.call(request, state, Context::new(rng_seed)).await;

                    let (parts, mut body) = response.into_parts();

                    let mut element = body.element;
                    element.optimize();
                    element.give_ids(&mut body.context.rng);

                    // TODO we will need to pass in like a bunch of stuff we'll get out of element
                    // like all of the element changing scripts
                    let adapter_script = body.context.adapter_script_element();
                    let mut html = config.layout.call(element, adapter_script);
                    html.optimize();

                    let mut output = String::from(DOCTYPE_HTML);
                    html.render(&mut output);

                    return axum::response::Response::from_parts(parts, Body::from(output));
                }

                let (mut parts, body) = request.into_parts();
                let request_parts = parts.clone();
                let ws = WebSocketUpgrade::from_request_parts(&mut parts, &state)
                    .await
                    .unwrap();
                let request = Request::from_parts(parts, body);

                let rng_seed: u64 = query
                    .get("coaxial-seed")
                    .expect("coaxial-seed param was not present")
                    .parse()
                    .expect("seed is not a number");

                let response = handler
                    .call(request, state.clone(), Context::new(rng_seed))
                    .await;

                ws.on_upgrade(|mut socket: WebSocket| async move {
                    let (_parts, body) = response.into_parts();

                    let mut context = body.context;
                    let pool = LocalPoolHandle::new(5);
                    let mut changes = Vec::new();

                    loop {
                        select! {
                            msg = socket.recv() => {
                                let Some(msg) = msg else {
                                    return;
                                };

                                let res = handle_socket_message(
                                    msg.map_err(|_| ()),
                                    &pool,
                                    request_parts.clone(),
                                    state.clone(),
                                    &context.states,
                                    &context.closures,
                                    &context.event_handlers,
                                )
                                    .await;

                                match res {
                                    Ok(_) => {}
                                    Err(SocketError::SkipMessage) => continue,
                                    Err(SocketError::Fatal) => return,
                                };
                            }
                            _ = context.changes_rx.recv_many(&mut changes, 10000) => {
                                let mut updates = Vec::new();
                                std::mem::swap(&mut changes, &mut updates);

                                let updates = updates.into_iter().map(|(id, v)| (id.to_string(), v)).collect::<Vec<_>>();

                                let out = OutMessage::Update { fields: &updates };
                                let msg = axum::extract::ws::Message::Text(serde_json::to_string(&out).unwrap());
                                socket.send(msg).await.unwrap();
                            }
                        }
                    }
                })
            }
        },
    )
}

type Closures<S> = std::collections::HashMap<String, Arc<dyn ClosureTrait<S>>>;
type EventHandlers = std::collections::HashMap<String, Arc<dyn EventHandler>>;
type States = std::collections::HashMap<StateId, Arc<dyn AnyState>>;

enum SocketError {
    Fatal,
    SkipMessage,
}

async fn handle_socket_message<S: Clone + Send + Sync + 'static>(
    msg: Result<Message, ()>,
    pool: &LocalPoolHandle,
    parts: Parts,
    state: S,
    states: &States,
    closures: &Closures<S>,
    events: &EventHandlers,
) -> Result<(), SocketError> {
    let msg: InMessage = match msg {
        Ok(Message::Text(msg)) => serde_json::from_str(&msg).unwrap(),
        Ok(_) => {
            return Err(SocketError::SkipMessage);
        }
        Err(_) => {
            // client disconnected
            return Err(SocketError::Fatal);
        }
    };

    match msg {
        InMessage::Closure { closure } => {
            let Some(closure) = closures.get(&closure) else {
                return Err(SocketError::Fatal);
            };

            let closure = closure.clone();
            let closure = move || async move { closure.call(parts, state).await };
            pool.spawn_pinned(closure).await.unwrap();
        }
        InMessage::Event { name, params } => {
            let Some(event) = events.get(&name) else {
                return Err(SocketError::Fatal);
            };

            let event = event.clone();
            pool.spawn_pinned(move || event.call(params)).await.unwrap();
        }
        InMessage::Set { id, value } => {
            // get the state and set it
            let Some(state) = states.get(&id) else {
                // TODO maybe we should return an error here?
                panic!("state not found");
            };
            state.set_value(value);
        }
    }

    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(tag = "t")]
enum InMessage {
    Closure {
        closure: String,
    },
    Event {
        name: String,
        params: serde_json::Value,
    },
    Set {
        id: StateId,
        value: serde_json::Value,
    },
}
#[derive(serde::Serialize)]
#[serde(tag = "t")]
enum OutMessage<'a> {
    Update {
        /// (field, value)
        fields: &'a [(String, String)],
    },
}
