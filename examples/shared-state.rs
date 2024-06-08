use std::sync::{atomic::AtomicI64, Arc};

use axum::{extract::State, Router};
use coaxial::{
    context::Context,
    html::{button, div, p},
    live::live,
    CoaxialResponse,
};
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
impl AppState {
    fn change(&self, diff: i64) -> i64 {
        let out = self
            .counter
            .fetch_add(diff, std::sync::atomic::Ordering::SeqCst);
        self.tx.send(()).unwrap();

        out + diff
    }
}

#[tokio::main]
async fn main() {
    // build our application with a single route
    let app = Router::new()
        .route("/", live(counter))
        .with_state(Arc::new(AppState::default()));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn counter(
    mut ctx: Context<Arc<AppState>>,
    State(state): State<Arc<AppState>>,
) -> CoaxialResponse<Arc<AppState>> {
    let counter = ctx.use_state(state.counter.load(std::sync::atomic::Ordering::SeqCst));

    let add = ctx.use_closure(move |State(state): State<Arc<AppState>>| async move {
        let out = state.change(1);
        counter.set(out);
    });
    let sub = ctx.use_closure(move |State(state): State<Arc<AppState>>| async move {
        let out = state.change(-1);
        counter.set(out);
    });

    // TODO this should ignore events sent by this page
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
