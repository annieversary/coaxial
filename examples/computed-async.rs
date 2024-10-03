use axum::Router;
use coaxial::{
    attrs,
    computed::InitialValue,
    context::Context,
    html::{b, button, div, p, style, Content, ContentValue},
    live::live,
    CoaxialResponse,
};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", live(counter));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn counter(mut ctx: Context) -> CoaxialResponse {
    let counter = ctx.use_state(0i32);

    let delayed_counter = ctx.use_computed_async_with(
        counter,
        |counter| {
            let counter = *counter;
            async move {
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

                counter
            }
        },
        InitialValue::Value(0),
    );

    let update = ctx.use_closure(move || async move {
        counter.set(*counter.get() + 1);
    });

    ctx.with(div(
        Content::List(vec![
            style(
                ContentValue::Raw(
                    html_escape::encode_style(include_str!("styles.css")).to_string(),
                ),
                Default::default(),
            )
            .into(),
            p(
                Content::List(vec![
                    b(counter, Default::default()).into(),
                    ", and 3 seconds later: ".into(),
                    b(delayed_counter, Default::default()).into(),
                ]),
                Default::default(),
            )
            .into(),
            button("update", attrs!("onclick" => update)).into(),
        ]),
        Default::default(),
    ))
}
