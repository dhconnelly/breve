use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Redirect,
    routing::{get, post},
    Router,
};
use nanoid;
use shuttle_runtime;
use sqlx::{Error, PgPool};
use url::Url;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

async fn redirect(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Redirect, StatusCode> {
    sqlx::query_as::<_, (String,)>("SELECT url FROM urls WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|err| match err {
            Error::RowNotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })
        .map(|(url,)| Redirect::to(&url))
}

async fn shorten(
    State(state): State<AppState>,
    url: String,
) -> Result<(StatusCode, String), StatusCode> {
    let url = Url::parse(&url).map_err(|_| StatusCode::BAD_REQUEST)?;
    let id = nanoid::nanoid!(21);
    sqlx::query("INSERT INTO urls(id, url) VALUES ($1, $2)")
        .bind(&id)
        .bind(url.as_str())
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        .map(|_| (StatusCode::OK, id))
}

#[shuttle_runtime::main]
async fn main(
    #[shuttle_shared_db::Postgres] pool: PgPool,
) -> shuttle_axum::ShuttleAxum {
    sqlx::migrate!()
        .run(&pool)
        .await
        .map_err(shuttle_runtime::CustomError::new)?;

    let state = AppState { pool };
    let router = Router::new()
        .route("/:id", get(redirect))
        .route("/", post(shorten))
        .with_state(state);
    Ok(router.into())
}
