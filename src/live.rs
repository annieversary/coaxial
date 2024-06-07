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
use tokio_util::task::LocalPoolHandle;

use crate::{handler::CoaxialHandler, Config};

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

                let mut response = handler.call(request, state).await;

                ws.on_upgrade(|mut socket: WebSocket| async move {
                    // if let Some(Ok(_)) = socket.recv().await {
                    //     let bytes = response.body().content.as_bytes();
                    //     let msg = axum::extract::ws::Message::Binary(bytes.to_vec());

                    //     if socket.send(msg).await.is_err() {
                    //         // client disconnected
                    //         return;
                    //     }
                    // }

                    let context = &mut response.body_mut().context;
                    let pool = LocalPoolHandle::new(5);

                    // TODO we are only sending messages back when a closure is run
                    // TODO we should probably separate listening to socket messages
                    // and sending messages back down when things change
                    while let Some(msg) = socket.recv().await {
                        let msg: InMessage = match msg {
                            Ok(Message::Text(msg)) => serde_json::from_str(&msg).unwrap(),
                            Ok(_) => {
                                continue;
                            }
                            Err(_) => {
                                // client disconnected
                                return;
                            }
                        };

                        match msg {
                            InMessage::Closure { closure } => {
                                // flush the changes channel
                                context
                                    .changes_rx
                                    .recv_many(&mut Vec::new(), context.changes_rx.len())
                                    .await;

                                let Some(closure) = context.closures.get(&closure) else {
                                    continue;
                                };

                                let closure = closure.clone();
                                pool.spawn_pinned(move || closure.call()).await.unwrap();

                                // if something changed, send a message with the update
                                let mut fields = Vec::new();
                                context.changes_rx.recv_many(&mut fields, 10000).await;

                                let out = OutMessage::Update { fields };
                                let msg = axum::extract::ws::Message::Text(
                                    serde_json::to_string(&out).unwrap(),
                                );
                                if socket.send(msg).await.is_err() {
                                    // client disconnected
                                    return;
                                }
                            }
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
enum InMessage {
    Closure { closure: String },
}
#[derive(serde::Serialize)]
#[serde(tag = "t")]
enum OutMessage {
    Update { fields: Vec<(u64, String)> },
}
