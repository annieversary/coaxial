use axum::{response::IntoResponse, Router};
use coaxial::{
    attrs, body, button, div, head, html, live, p, slot, Coaxial, Context, State, Stated,
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

// TODO we might wanna make a macro that makes this but
// each field is wrapped in State
#[derive(Default)]
struct Data {
    counter: u32,
}
#[derive(Default)]
struct DataState {
    counter: State<u32>,
}
impl Stated for Data {
    type State = DataState;

    fn to_state(&self) -> Self::State {
        DataState {
            counter: State::new(value, id),
        }
    }
}

async fn counter(mut ctx: Context<Data>) -> impl IntoResponse {
    let add = ctx.use_closure(|d: &mut Data| async {
        d.counter += 1;
    });
    let sub = ctx.use_closure(|d: &mut Data| async {
        d.counter -= 1;
    });

    ctx.with(div(p(ctx.state.counter)
        + button(("+", attrs!(("onclick", add))))
        + button(("-", attrs!(("onclick", sub))))))
}
