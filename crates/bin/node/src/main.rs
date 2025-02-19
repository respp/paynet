use cashu_starknet::Unit;
use cashu_starknet_node::NodeServer;
use clap::Parser;
use commands::read_env_variables;
use errors::{Error, InitializationError, ServiceError, SignerError};
use futures::TryFutureExt;
use grpc_service::GrpcState;
use methods::Method;
use nuts::{
    nut04::MintMethodSettings, nut05::MeltMethodSettings, nut06::NutsSettings, Amount,
    QuoteTTLConfig,
};
use sqlx::PgPool;
use tokio::try_join;
use tracing::info;

mod app_state;
mod commands;
mod errors;
mod grpc_service;
mod indexer;
mod keyset_cache;
mod logic;
mod methods;
mod routes;
mod utils;

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
    tracing_subscriber::fmt().init();
    info!("Initializing node...");

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
    };

    let signer_client = cashu_signer::SignerClient::connect(config.signer_url)
        .await
        .map_err(SignerError::from)?;

    let grpc_service = GrpcState::new(
        pg_pool,
        signer_client,
        nuts_settings,
        QuoteTTLConfig {
            mint_ttl: 3600,
            melt_ttl: 3600,
        },
    );

    let addr = format!("[::1]:{}", config.grpc_server_port)
        .parse()
        .unwrap();
    let tonic_future = tonic::transport::Server::builder()
        .add_service(NodeServer::new(grpc_service))
        .serve(addr)
        .map_err(ServiceError::TonicTransport);

    let indexer_service = indexer::spawn_indexer_task(
        env_variables.apibara_token,
        strk_token_address,
        config.recipient_address,
    )
    .await?;
    let indexer_future = indexer::listen_to_indexer(indexer_service);

    // Run them forever
    info!("Initialized!");
    info!("Running gRPC server on port {}", config.grpc_server_port);
    let ((), ()) = try_join!(tonic_future, indexer_future)?;

    Ok(())
}
