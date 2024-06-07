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

use crate::{closure::AsyncFn, handler::CoaxialHandler, Config};

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
                                    &context.closures,
                                    &pool,
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

type Closures = std::collections::HashMap<String, Arc<dyn AsyncFn>>;

enum SocketError {
    Fatal,
    SkipMessage,
}

async fn handle_socket_message(
    msg: Result<Message, ()>,
    closures: &Closures,
    pool: &LocalPoolHandle,
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
            pool.spawn_pinned(move || closure.call()).await.unwrap();
        }
    }

    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(tag = "t")]
enum InMessage {
    Closure { closure: String },
}
#[derive(serde::Serialize)]
#[serde(tag = "t")]
enum OutMessage<'a> {
    Update { fields: &'a [(u64, String)] },
}
