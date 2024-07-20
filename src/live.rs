use std::sync::Arc;

use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket},
        FromRequestParts, Request, WebSocketUpgrade,
    },
    http::request::Parts,
    routing::{get, MethodRouter},
    Extension,
};
use tokio::select;
use tokio_util::task::LocalPoolHandle;

use crate::{
    closure::ClosureTrait, event_handlers::EventHandler, handler::CoaxialHandler,
    html::DOCTYPE_HTML, state::AnyState, Config,
};

pub fn live<T, H, S>(handler: H) -> MethodRouter<S>
where
    H: CoaxialHandler<T, S>,
    S: Clone + Send + Sync + 'static,
{
    get(
        |axum::extract::State(state): axum::extract::State<S>,
         config: Option<Extension<Config>>,
         request: Request| {
            let config = config.map(|c| c.0).unwrap_or_default();

            let is_websocket = request
                .headers()
                .get("Upgrade")
                .and_then(|v| v.to_str().ok())
                == Some("websocket");

            async move {
                if !is_websocket {
                    let response = handler.call(request, state).await;

                    let (parts, body) = response.into_parts();

                    let element = config.layout.call(body.element);

                    // add listeners for the registered event handlers
                    let events = body.context.event_handlers;
                    if !events.is_empty() {
                        let mut script = String::new();

                        for (name, handler) in events {
                            script.push_str("document.addEventListener('");
                            script.push_str(&name);
                            script.push_str("', params=>{params={");

                            // NOTE: this serves two puposes:
                            // 1. events are big objects with lots of fields, so we only wanna send the ones we care about over the wire
                            // 2. serialization of events is wonky, and a lot of times fields are not set correctly
                            for field in handler.param_fields() {
                                script.push_str(field);
                                script.push_str(": params.");
                                script.push_str(field);
                                script.push(',');
                            }

                            script.push_str("};if (window.Coaxial) window.Coaxial.onEvent('");
                            script.push_str(&name);
                            script.push_str("', params);});");
                        }

                        // TODO add to the page
                    }

                    let mut output = String::from(DOCTYPE_HTML);
                    element.render(&mut output);

                    return axum::response::Response::from_parts(parts, Body::from(output));
                }

                let (mut parts, body) = request.into_parts();
                let request_parts = parts.clone();
                let ws = WebSocketUpgrade::from_request_parts(&mut parts, &state)
                    .await
                    .unwrap();
                let request = Request::from_parts(parts, body);

                let response = handler.call(request, state.clone()).await;

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
                                let out = OutMessage::Update { fields: &changes };
                                let msg = axum::extract::ws::Message::Text(serde_json::to_string(&out).unwrap());
                                socket.send(msg).await.unwrap();
                                changes.clear();
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
type States = std::collections::HashMap<u64, Arc<dyn AnyState>>;

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
                // TODO maybe we should error here?
                return Ok(());
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
        id: u64,
        value: serde_json::Value,
    },
}
#[derive(serde::Serialize)]
#[serde(tag = "t")]
enum OutMessage<'a> {
    Update { fields: &'a [(u64, String)] },
}
