use std::collections::HashMap;

use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket},
        FromRequestParts, Query, Request, WebSocketUpgrade,
    },
    routing::{get, MethodRouter},
    Extension,
};
use rand::random;
use tokio::{select, sync::mpsc::UnboundedSender};

use crate::{
    config::Config, context::Context, events::Events, handler::CoaxialHandler, html::DOCTYPE_HTML,
    random_id::RandomId, reactive_js::Reactivity, states::States,
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

                    let response = handler
                        .call(request, state, Context::new(rng_seed, false))
                        .await;

                    let (parts, mut body) = response.into_parts();

                    let mut element = body.element;
                    element.optimize();
                    element.give_ids(&mut body.context.rng);

                    let reactive_scripts = {
                        let mut reactivity = Reactivity::default();
                        element.reactivity(&mut reactivity);
                        reactivity.script()
                    };

                    let adapter_script = body.context.adapter_script_element(&reactive_scripts);
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

                // TODO ideally, we'll store the context in a HashMap after the initial request,
                // which allows us to not re-run the handler here
                let response = handler
                    .call(request, state.clone(), Context::new(rng_seed, true))
                    .await;

                ws.on_upgrade(|mut socket: WebSocket| async move {
                    let (_parts, body) = response.into_parts();

                    let mut context = body.context;

                    let mut changes = Vec::new();
                    let mut closure_calls = Vec::new();

                    loop {
                        select! {
                            msg = socket.recv() => {
                                let Some(msg) = msg else {
                                    return;
                                };

                                let res = handle_socket_message(
                                    msg.map_err(|_| ()),
                                    &context.states,
                                    &context.closures.call_tx,
                                    &mut context.events,
                                )
                                    .await;

                                match res {
                                    Ok(_) => {}
                                    Err(SocketError::SkipMessage) => continue,
                                    Err(SocketError::Fatal) => return,
                                };
                            }
                            _ = context.states.changes_rx.recv_many(&mut changes, 10000) => {
                                let mut updates = Vec::new();
                                std::mem::swap(&mut changes, &mut updates);

                                for (id, _) in &updates {
                                    context.computed_states.recompute_dependents(*id);
                                }

                                let updates = updates.into_iter().map(|(id, v)| (id.to_string(), v)).collect::<Vec<_>>();

                                let out = OutMessage::Update { fields: &updates };
                                let msg = axum::extract::ws::Message::Text(serde_json::to_string(&out).unwrap());
                                socket.send(msg).await.unwrap();
                            }
                            _ = context.closures.call_rx.recv_many(&mut closure_calls, 10000) => {
                                let mut closures: Vec<RandomId> = Vec::new();
                                std::mem::swap(&mut closures, &mut closure_calls);

                                for closure in  &closures {
                                    context.closures.run(*closure, &request_parts, &state);
                                }
                            }
                        }
                    }
                })
            }
        },
    )
}

enum SocketError {
    Fatal,
    SkipMessage,
}

async fn handle_socket_message(
    msg: Result<Message, ()>,
    states: &States,
    closure_call_tx: &UnboundedSender<RandomId>,
    events: &mut Events,
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
            closure_call_tx.send(closure).unwrap();
        }
        InMessage::Event { name, params } => {
            events.handle(name, params);
        }
        InMessage::SetState { id, value } => {
            states.set(id, value);
        }
    }

    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(tag = "t")]
enum InMessage {
    Closure {
        closure: RandomId,
    },
    Event {
        name: String,
        params: serde_json::Value,
    },
    SetState {
        id: RandomId,
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
