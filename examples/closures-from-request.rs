//! Navigate to http://localhost:3000?amount=NUM and press update.
//! The counter will increment by NUM

use axum::{extract::Query, Router};
use coaxial::{
    attrs,
    context::Context,
    html::{button, div, p, Content},
    live::live,
    CoaxialResponse,
};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", live(counter));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(serde::Deserialize)]
struct CounterQuery {
    amount: i32,
}

async fn counter(mut ctx: Context) -> CoaxialResponse {
    let counter = ctx.use_state(0i32);

    let update = ctx.use_closure(move |Query(query): Query<CounterQuery>| async move {
        counter.set(counter.get() + query.amount);
    });

    ctx.with(div(
        Content::List(vec![
            p(counter, Default::default()).into(),
            button("update", attrs!("onclick" => update)).into(),
        ]),
        Default::default(),
    ))
}
