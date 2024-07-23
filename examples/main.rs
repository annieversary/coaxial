use axum::Router;
use coaxial::{
    attrs,
    config::Config,
    context::Context,
    html::{body, button, div, head, html, p, strong, style, Content},
    live::live,
    CoaxialResponse,
};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", live(counter))
        // this following layer call is optional since this is the default, i'm adding it for documentation purposes
        .layer(
            Config::with_layout(|content, coaxial_adapter_script| {
                html(
                    Content::List(vec![
                        head(
                            Content::List(vec![style(
                                Content::Raw(
                                    html_escape::encode_style(include_str!("styles.css"))
                                        .to_string(),
                                ),
                                Default::default(),
                            )
                            .into()]),
                            Default::default(),
                        )
                        .into(),
                        body(
                            Content::List(vec![content.into(), coaxial_adapter_script.into()]),
                            Default::default(),
                        )
                        .into(),
                    ]),
                    Default::default(),
                )
            })
            .layer(),
        );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn counter(mut ctx: Context) -> CoaxialResponse {
    let counter = ctx.use_state(0i32);
    let clicks = ctx.use_state(0u32);

    let add = ctx.use_closure(move || async move {
        counter.set(counter.get() + 1);
        clicks.set(clicks.get() + 1);
    });
    let sub = ctx.use_closure(move || async move {
        counter.set(counter.get() - 1);
        clicks.set(clicks.get() + 1);
    });

    let click = ctx.use_closure(move || async move {
        clicks.set(clicks.get() + 1);
    });

    let element = div(
        Content::List(vec![
            div(
                Content::List(vec![
                    button("increment counter", attrs!("onclick" => add)).into(),
                    button("decrement counter", attrs!("onclick" => sub)).into(),
                    button("click for fun :3", attrs!("onclick" => click)).into(),
                ]),
                attrs!("class" => "buttons"),
            )
            .into(),
            p(
                Content::List(vec![
                    "counter is ".into(),
                    counter.into(),
                    ". ".into(),
                    strong("Wow!", Default::default()).into(),
                    " ".into(),
                    clicks.into(),
                    " total clicks. ".into(),
                    strong(
                        "This next number is the counter again: ",
                        Default::default(),
                    )
                    .into(),
                    counter.into(),
                ]),
                // counter,
                Default::default(),
            )
            .into(),
        ]),
        attrs!("class" => "container"),
    );

    ctx.with(element)
}
