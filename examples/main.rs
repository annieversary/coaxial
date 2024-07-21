use axum::Router;
use coaxial::{
    attrs,
    config::Config,
    context::Context,
    html::{body, button, div, head, html, input, p, Content},
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
                        head(Content::Empty, Default::default()).into(),
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
    let counter = ctx.use_state(0u32);

    let add = ctx.use_closure(move || async move {
        counter.set(counter.get() + 1);
    });
    let sub = ctx.use_closure(move || async move {
        counter.set(counter.get() - 1);
    });

    let element = div(
        Content::List(vec![
            input(attrs!("value" => counter)).into(),
            button("+", attrs!("onclick" => add)).into(),
            button("-", attrs!("onclick" => sub)).into(),
            p(
                Content::List(vec![counter.into(), " clicks".into()]),
                // counter,
                Default::default(),
            )
            .into(),
        ]),
        Default::default(),
    );

    ctx.with(element)
}
