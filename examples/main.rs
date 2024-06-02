use axum::Router;
use coaxial::{
    attrs, body, button, div, head, html, live, p, slot, Coaxial, CoaxialResponse, Context,
};

#[tokio::main]
async fn main() {
    // build our application with a single route
    let app = Router::new()
        .route("/", live(counter))
        .layer(Coaxial::with_layout(html(head(()) + body(slot()))));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn counter(mut ctx: Context) -> CoaxialResponse {
    let counter = ctx.use_state(0u32);

    let add = ctx.use_closure(move || async { todo!() });
    let sub = ctx.use_closure(move || async { todo!() });

    ctx.with(div(p(counter)
        + button(("+", attrs!(("onclick", add))))
        + button(("-", attrs!(("onclick", sub))))))
}
