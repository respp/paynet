use axum::{
    routing::{get, post},
    Router,
};
use cashu_starknet::Unit;
use clap::Parser;
use commands::read_env_variables;
use errors::{Error, InitializationError, ServiceError, SignerError};
use methods::Method;
use nuts::{
    nut04::MintMethodSettings, nut05::MeltMethodSettings, nut06::NutsSettings, Amount,
    QuoteTTLConfig,
};
use sqlx::PgPool;
use std::net::Ipv6Addr;
use tokio::try_join;

mod app_state;
mod commands;
mod errors;
mod indexer;
mod keyset_cache;
mod routes;
mod utils;
use app_state::AppState;
mod logic;
mod methods;

const CASHU_REST_PORT: u16 = 3338;

async fn connect_to_db_and_run_migrations(pg_url: &str) -> Result<PgPool, InitializationError> {
    let pool = PgPool::connect(pg_url)
        .await
        .map_err(InitializationError::DbConnect)?;

    memory_db::run_migrations(&pool)
        .await
        .map_err(InitializationError::DbMigrate)?;

    Ok(pool)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // initialize tracing
    tracing_subscriber::fmt::init();

    let args = commands::Args::parse();
    let config = args.read_config()?;
    // Do this early to fail early
    let strk_token_address = config
        .strk_token_contract_address()
        .map_err(InitializationError::Config)?;
    let env_variables = read_env_variables()?;
    let pg_pool = connect_to_db_and_run_migrations(&env_variables.pg_url).await?;

    let nuts_settings = NutsSettings {
        nut04: nuts::nut04::Settings {
            methods: vec![MintMethodSettings {
                method: Method::Starknet,
                unit: Unit::Strk,
                min_amount: Some(Amount::ONE),
                max_amount: None,
                description: true,
            }],
            disabled: false,
        },
        nut05: nuts::nut05::Settings {
            methods: vec![MeltMethodSettings {
                method: Method::Starknet,
                unit: Unit::Strk,
                min_amount: Some(Amount::ONE),
                max_amount: None,
            }],
            disabled: false,
        },
        #[cfg(feature = "nut19")]
        nut19: nuts::nut19::Settings {
            ttl: Some(3600),
            cached_endpoints: vec![
                nuts::nut19::CachedEndpoint {
                    method: nuts::nut19::HttpMethod::Post,
                    path: nuts::nut19::Path::Mint(Method::Starknet),
                },
                nuts::nut19::CachedEndpoint {
                    method: nuts::nut19::HttpMethod::Post,
                    path: nuts::nut19::Path::Swap,
                },
                nuts::nut19::CachedEndpoint {
                    method: nuts::nut19::HttpMethod::Post,
                    path: nuts::nut19::Path::Melt(Method::Starknet),
                },
            ],
        },
    };
    let cached_routes = {
        let router = Router::new()
            .route("/v1/mint/{method}", post(routes::mint))
            .route("/v1/swap", post(routes::swap))
            .route("/v1/melt/{method}", post(routes::melt));

        #[cfg(feature = "nut19")]
        let router = router.layer(axum_response_cache::CacheLayer::with_lifespan(
            nuts_settings.nut19.ttl.unwrap_or(300),
        ));

        router
    };

    let signer_client = cashu_signer::SignerClient::connect(config.signer_url)
        .await
        .map_err(SignerError::from)?;

    let app = Router::new()
        .merge(cached_routes)
        .route("/v1/mint/quote/{method}", post(routes::mint_quote))
        .route(
            "/v1/mint/quote/{method}/{quote_id}",
            get(routes::mint_quote_state),
        )
        .route("/v1/melt/quote/{method}", post(routes::melt_quote))
        .route(
            "/v1/melt/quote/{method}/{quote_id}",
            get(routes::melt_quote_state),
        )
        .with_state(AppState::new(
            pg_pool,
            signer_client,
            nuts_settings,
            QuoteTTLConfig {
                mint_ttl: 3600,
                melt_ttl: 3600,
            },
        ));

    let rest_socket_address =
        std::net::SocketAddr::new(Ipv6Addr::LOCALHOST.into(), CASHU_REST_PORT);
    let listener = tokio::net::TcpListener::bind(rest_socket_address)
        .await
        .map_err(InitializationError::BindTcp)?;

    let axum_future = async {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                println!("Rpc server shut down gracefully");
            })
            .await
            .map_err(ServiceError::AxumServe)
    };

    let indexer_service = indexer::spawn_indexer_task(
        env_variables.apibara_token,
        strk_token_address,
        config.recipient_address,
    )
    .await?;
    let indexer_future = indexer::listen_to_indexer(indexer_service);

    // Run them forever
    let ((), ()) = try_join!(axum_future, indexer_future)?;

    Ok(())
}
