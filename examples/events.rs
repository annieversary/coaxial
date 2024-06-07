use axum::Router;
use coaxial::{
    context::Context,
    html::{body, div, head, html, p, slot},
    live::live,
    CoaxialResponse, Config,
};

#[tokio::main]
async fn main() {
    // build our application with a single route
    let app = Router::new()
        .route("/", live(counter))
        // this following line is optional since this is the default, i'm adding it for documentation purposes
        .layer(Config::with_layout(html(head(()) + body(slot()))).layer());

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn counter(mut ctx: Context) -> CoaxialResponse {
    let x = ctx.use_state(0i32);

    // this is a global event, applies to document
    ctx.on("click", move |event| async move {
        #[derive(Debug, serde::Deserialize)]
        struct MouseMove {
            #[serde(rename = "clientX")]
            x: i32,
            #[serde(rename = "clientY")]
            _y: i32,
        }

        let event: MouseMove = serde_json::from_value(event).unwrap();
        x.set(event.x);
    });

    ctx.with(div(p(x)))
}
