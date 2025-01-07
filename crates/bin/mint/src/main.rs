use axum::{routing::post, Router};
use sqlx::{migrate::Migrator, PgPool};
use std::{net::Ipv6Addr, path::Path, str::FromStr};

mod app_state;
mod errors;
mod keyset_cache;
mod routes;
use app_state::AppState;

const CASHU_REST_PORT: u16 = 3338;

async fn init() -> PgPool {
    dotenv::dotenv().expect("faild to load .env file");
    let database_url =
        std::env::var("DATABASE_URL").expect("env should contain a `DATABASE_URL` var");
    let migration_folder = std::env::var("DATABASE_MIGRATIONS")
        .expect("env should contain a `DATABASE_MIGRATIONS` var");

    let pool = PgPool::connect(&database_url)
        .await
        .expect("should be able to connect to db");

    Migrator::new(Path::new(&migration_folder))
        .await
        .expect("should be a migration folder")
        .run(&pool)
        .await
        .expect("should be able to run migrations");

    pool
}

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();

    let pg_pool = init().await;

    // build our application with a route
    let app = Router::new()
        .route("/swap", post(routes::swap))
        .with_state(AppState::new(pg_pool, &[1, 2, 3, 4, 5]));
    // .with_state(KeysetCache::default());

    let rest_socket_address =
        std::net::SocketAddr::new(Ipv6Addr::LOCALHOST.into(), CASHU_REST_PORT);
    let listener = tokio::net::TcpListener::bind(rest_socket_address)
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Strk,
}

impl FromStr for Unit {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let unit = match s {
            "strk" => Self::Strk,
            _ => return Err("invalid value for enum `Unit`"),
        };

        Ok(unit)
    }
}

impl std::fmt::Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(
            match self {
                Unit::Strk => "strk",
            },
            f,
        )
    }
}

impl From<Unit> for u32 {
    fn from(value: Unit) -> Self {
        match value {
            Unit::Strk => 0,
        }
    }
}

impl nuts::traits::Unit for Unit {}
