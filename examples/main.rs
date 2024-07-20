use axum::Router;
use coaxial::{
    attrs, coaxial_adapter_script,
    context::Context,
    html::{body, button, div, head, html, input, p, Attribute, Content},
    live::live,
    CoaxialResponse, Config,
};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", live(counter))
        // this following layer call is optional since this is the default, i'm adding it for documentation purposes
        .layer(
            Config::with_layout(|content| {
                html(
                    Content::Children(vec![
                        head(Content::Empty, Default::default()),
                        body(
                            Content::Children(vec![content, coaxial_adapter_script()]),
                            Default::default(),
                        ),
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
        Content::Children(vec![
            input(attrs!("value" => Attribute::State(counter.into()))),
            button(
                Content::Text("+".to_string()),
                attrs!("onclick" => Attribute::Closure(add.into())),
            ),
            button(
                Content::Text("-".to_string()),
                attrs!("onclick" => Attribute::Closure(sub.into())),
            ),
            p(Content::State(counter.into()), Default::default()),
        ]),
        Default::default(),
    );

    ctx.with(element)
}
