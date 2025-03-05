use std::sync::Arc;

use axum::{Router, extract::Query, response::Html, routing::get};
use serde::Deserialize;

#[derive(Default)]
pub struct AppState {}

pub fn backend() -> Router<AppState> {
    let app_state = Arc::new(AppState::default());

    Router::new()
        .route("/greet", get(greet))
        .with_state(app_state)
}

#[derive(Deserialize)]
pub struct GreetParams {
    name: Option<String>,
}
pub async fn greet(Query(params): Query<GreetParams>) -> Html<String> {
    println!("WE ARE CALLED");
    Html(format!(
        "Hello, {}! You've been greeted from Rust!",
        params.name.unwrap_or_default()
    ))
}
