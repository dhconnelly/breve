use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
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
use std::string;
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

struct HtmlResponse(pub StatusCode, pub Html<String>);

impl HtmlResponse {
    fn new<S: Into<String>>(code: StatusCode, html: S) -> Self {
        Self(code, Html(html.into()))
    }

    fn server_error() -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "server error")
    }

    fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "not found")
    }

    fn bad_request<S: Into<String>>(html: S) -> Self {
        Self::new(StatusCode::BAD_REQUEST, html)
    }

    fn ok<S: Into<String>>(html: S) -> Self {
        Self::new(StatusCode::OK, html)
    }
}

impl IntoResponse for HtmlResponse {
    fn into_response(self) -> axum::response::Response {
        let Self(code, html) = self;
        (code, html).into_response()
    }
}

impl From<url::ParseError> for HtmlResponse {
    fn from(_: url::ParseError) -> Self {
        HtmlResponse::bad_request("invalid url")
    }
}

impl From<sqlx::Error> for HtmlResponse {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => HtmlResponse::not_found(),
            _ => HtmlResponse::server_error(),
        }
    }
}

impl From<string::FromUtf8Error> for HtmlResponse {
    fn from(_: string::FromUtf8Error) -> Self {
        HtmlResponse::server_error()
    }
}

async fn index() -> Result<HtmlResponse, HtmlResponse> {
    let file = Assets::get("index.html").ok_or(HtmlResponse::server_error())?;
    let data = file.data.into_owned();
    let index = String::from_utf8(data)?;
    Ok(HtmlResponse::ok(index))
}

async fn redirect(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Redirect, HtmlResponse> {
    let url: (String,) = sqlx::query_as("SELECT url FROM urls WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Redirect::to(&url.0))
}

async fn shorten(
    State(state): State<AppState>,
    Form(form): Form<ShortenRequest>,
) -> Result<HtmlResponse, HtmlResponse> {
    let url = Url::parse(&form.url)?;
    let id = nanoid::nanoid!(21);
    let shortened = state.url_base.join(&id)?;
    let a = format!("<a href=\"{0}\">{0}</a>", shortened.to_string());
    sqlx::query("INSERT INTO urls(id, url) VALUES ($1, $2)")
        .bind(&id)
        .bind(url.as_str())
        .execute(&state.pool)
        .await?;
    Ok(HtmlResponse::ok(a))
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
