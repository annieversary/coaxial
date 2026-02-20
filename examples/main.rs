use axum::Router;
use coaxial::{
    attrs,
    config::Config,
    context::Context,
    html::{body, button, div, head, html, p, strong, style, Content, ContentValue, Element},
    live::live,
    states::State,
    CoaxialResponse,
};
use futures_signals::signal::Mutable;

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
                                ContentValue::Raw(
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

struct Function<Component> {
    f: fn(&Component),
}

// the user should probably we writing something like
struct MyCounter {
    counter: i32,
    clicks: u32,
}

// then a macro generates this

struct Counter {
    counter: State<i32>,
    clicks: State<u32>,

    click: Function<Self>,
    add: Function<Self>,
    sub: Function<Self>,
}

impl Counter {
    fn new(ctx: &mut Context) -> Self {
        // TODO do we just store the closures in the actual struct?
        // it feels ugly though
        // the code you end up writing looks weird

        // and how the fuck do u end up calling this function?
        // what do we send to the client that we can bounce up to the server
        // that can get called?
        // we dont have the function name or the field name

        // enums make a lot more sense for that side
        // but i really dont wanna do like a big update function

        Self {
            counter: ctx.use_state(0i32),
            clicks: ctx.use_state(0u32),

            click: Function { f: Self::click },
            add: Function { f: Self::add },
            sub: Function { f: Self::sub },
        }
    }

    fn click(&self) {
        self.clicks.replace_with(|value| *value + 1);
    }

    fn add(&self) {
        self.counter.replace_with(|value| *value + 1);
        self.click();
    }

    fn sub(&self) {
        self.counter.replace_with(|value| *value - 1);
        self.click();
    }

    // TODO so i need a way to wrap closures and add them to the context
    // maybe in new?

    fn build(&self) -> Element {
        div(
            Content::List(vec![
                div(
                    Content::List(vec![
                        button(
                            "increment counter",
                            attrs!(
                                "onclick" => self.add,
                                "title" => ("go from ",self.counter," to ",self.counter,"+1")
                            ),
                        )
                        .into(),
                        button("decrement counter", attrs!("onclick" => self.sub)).into(),
                        button("click for fun :3", attrs!("onclick" => self.click)).into(),
                    ]),
                    attrs!("class" => "buttons", "data-clicks" => self.clicks),
                )
                .into(),
                p(
                    Content::List(vec![
                        "counter is ".into(),
                        self.counter.into(),
                        ". ".into(),
                        strong("Wow!", Default::default()).into(),
                        " counter is ".into(),
                        self.counter.into(),
                        " and there are ".into(),
                        self.clicks.into(),
                        " total clicks. ".into(),
                        strong(
                            "This next number is the counter again: ",
                            Default::default(),
                        )
                        .into(),
                        self.counter.into(),
                    ]),
                    // counter,
                    Default::default(),
                )
                .into(),
            ]),
            attrs!("class" => "container"),
        )
    }
}

async fn counter(mut ctx: Context) -> CoaxialResponse {
    let counter = Counter::new(&mut ctx);

    ctx.with(counter.build())
}
