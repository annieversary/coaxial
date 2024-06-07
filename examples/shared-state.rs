use std::sync::{atomic::AtomicI64, Arc};

use axum::{extract::State, Router};
use coaxial::{body, button, div, head, html, live, p, slot, Coaxial, CoaxialResponse, Context};
use tokio::sync::broadcast::{self, Sender};

struct AppState {
    counter: AtomicI64,
    tx: Sender<()>,
}
impl Default for AppState {
    fn default() -> Self {
        let (tx, _rx) = broadcast::channel(100);
        let counter = AtomicI64::new(0);

        Self { counter, tx }
    }
}

#[tokio::main]
async fn main() {
    // build our application with a single route
    let app = Router::new()
        .route("/", live(counter))
        .layer(Coaxial::with_layout(html(head(()) + body(slot()))))
        .with_state(Arc::new(AppState::default()));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn counter(mut ctx: Context, State(state): State<Arc<AppState>>) -> CoaxialResponse {
    let counter = ctx.use_state(state.counter.load(std::sync::atomic::Ordering::SeqCst));

    let s = state.clone();
    let add = ctx.use_closure(move || {
        let state = s.clone();
        async move {
            let out = state
                .counter
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            counter.set(out + 1);
            state.tx.send(()).unwrap();
        }
    });
    let s = state.clone();
    let sub = ctx.use_closure(move || {
        let state = s.clone();
        async move {
            let out = state
                .counter
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            counter.set(out - 1);
            state.tx.send(()).unwrap();
        }
    });

    // TODO this doesn't actually dynamically update things
    // TODO this updates things when it's changed on our own closure
    let state = state.clone();
    tokio::spawn(async move {
        let mut rx = state.tx.subscribe();
        while let Ok(()) = rx.recv().await {
            counter.set(state.counter.load(std::sync::atomic::Ordering::SeqCst));
        }
    });

    ctx.with(div(p(counter)
        + button(("+", ("onclick", add)))
        + button(("-", ("onclick", sub)))))
}
