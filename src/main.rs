use axum::{routing::get, Router};
use sqlx::PgPool;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

async fn hello_world() -> &'static str {
    "Hello, world!"
}

#[shuttle_runtime::main]
async fn main(
    #[shuttle_shared_db::Postgres] pool: PgPool,
) -> shuttle_axum::ShuttleAxum {
    let state = AppState { pool };
    let router = Router::new().route("/", get(hello_world)).with_state(state);
    Ok(router.into())
}
