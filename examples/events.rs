use axum::Router;
use coaxial::{context::Context, html::p, live::live, CoaxialResponse};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", live(counter));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn counter(mut ctx: Context) -> CoaxialResponse {
    let x = ctx.use_state(0i32);

    #[derive(Debug, serde::Deserialize)]
    struct MouseMove {
        #[serde(rename = "clientX")]
        x: i32,
        #[serde(rename = "clientY")]
        _y: i32,
    }

    // this is a global event, applies to document
    ctx.on("click", move |event: MouseMove| async move {
        x.set(event.x);
    });

    ctx.with(p(x))
}
