use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, Redirect},
    routing::{get, post},
    Router,
};
use nanoid;
use rust_embed::RustEmbed;
use serde::Deserialize;
use shuttle_runtime;
use shuttle_secrets;
use shuttle_shared_db;
use sqlx;
use url::Url;

#[derive(Clone)]
struct AppState {
    pool: sqlx::PgPool,
    url_base: Url,
}

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

#[derive(Deserialize)]
struct ShortenRequest {
    pub url: String,
}

async fn index() -> Result<Html<String>, StatusCode> {
    let file =
        Assets::get("index.html").ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let data = file.data.into_owned();
    let index = String::from_utf8(data)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(index))
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
            sqlx::Error::RowNotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })
        .map(|(url,)| Redirect::to(&url))
}

async fn shorten(
    State(state): State<AppState>,
    Form(form): Form<ShortenRequest>,
) -> Result<(StatusCode, Html<String>), StatusCode> {
    let url = Url::parse(&form.url).map_err(|_| StatusCode::BAD_REQUEST)?;
    let id = nanoid::nanoid!(21);
    let shortened = state
        .url_base
        .join(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let a = Html(format!("<a href=\"{0}\">{0}</a>", shortened.to_string()));
    sqlx::query("INSERT INTO urls(id, url) VALUES ($1, $2)")
        .bind(&id)
        .bind(url.as_str())
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        .map(|_| (StatusCode::OK, a))
}

#[shuttle_runtime::main]
async fn main(
    #[shuttle_shared_db::Postgres] pool: sqlx::PgPool,
    #[shuttle_secrets::Secrets] secrets: shuttle_secrets::SecretStore,
) -> shuttle_axum::ShuttleAxum {
    sqlx::migrate!()
        .run(&pool)
        .await
        .map_err(shuttle_runtime::CustomError::new)?;

    let url_base = secrets
        .get("URL_BASE")
        .unwrap_or(String::from("http://localhost:8000"));
    let url_base =
        Url::parse(&url_base).map_err(shuttle_runtime::CustomError::new)?;
    let state = AppState { pool, url_base };

    let router = Router::new()
        .route("/", get(index))
        .route("/:id", get(redirect))
        .route("/", post(shorten))
        .with_state(state);
    Ok(router.into())
}
