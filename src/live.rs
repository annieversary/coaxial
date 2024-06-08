use std::sync::Arc;

use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket},
        FromRequestParts, Request, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::{get, MethodRouter},
    Extension,
};
use tokio::select;
use tokio_util::task::LocalPoolHandle;

use crate::{closure::AsyncFn, event_handlers::EventHandler, handler::CoaxialHandler, Config};

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

                    let mut output = config.layout.call(body.element).content;

                    // add listeners for the registered event handlers
                    let events = body.context.event_handlers;
                    if !events.is_empty() {
                        output.push_str("<script>");
                        for (name, handler) in events {
                            output.push_str("document.addEventListener('");
                            output.push_str(&name);
                            output.push_str("', params=>{params={");

                            // NOTE: this serves two puposes:
                            // 1. events are big objects with lots of fields, so we only wanna send the ones we care about over the wire
                            // 2. serialization of events is wonky, and a lot of times fields are not set correctly
                            for field in handler.param_fields() {
                                output.push_str(field);
                                output.push_str(": params.");
                                output.push_str(field);
                                output.push(',');
                            }

                            output.push_str("};if (window.Coaxial) window.Coaxial.onEvent('");
                            output.push_str(&name);
                            output.push_str("', params);});");
                        }
                        output.push_str("</script>");
                    }

                    return axum::response::Response::from_parts(parts, Body::from(output));
                }

                let (mut parts, body) = request.into_parts();
                let ws = WebSocketUpgrade::from_request_parts(&mut parts, &state)
                    .await
                    .unwrap();
                let request = Request::from_parts(parts, body);

                let response = handler.call(request, state).await;

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
                .into_response()
            }
        },
    )
}

type Closures = std::collections::HashMap<String, Arc<dyn AsyncFn<()>>>;
type EventHandlers = std::collections::HashMap<String, Arc<dyn EventHandler>>;

enum SocketError {
    Fatal,
    SkipMessage,
}

async fn handle_socket_message(
    msg: Result<Message, ()>,
    pool: &LocalPoolHandle,
    closures: &Closures,
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
            pool.spawn_pinned(move || closure.call(())).await.unwrap();
        }
        InMessage::Event { name, params } => {
            let Some(event) = events.get(&name) else {
                return Err(SocketError::Fatal);
            };

            let event = event.clone();
            pool.spawn_pinned(move || event.call(params)).await.unwrap();
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
}
#[derive(serde::Serialize)]
#[serde(tag = "t")]
enum OutMessage<'a> {
    Update { fields: &'a [(u64, String)] },
}
