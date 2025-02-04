use axum::{
    routing::{get, post},
    Router,
};
use cashu_starknet::{StarknetU256, STRK_TOKEN_ADDRESS};
use dotenv::dotenv;
use errors::StarknetError;
use invoice_payment_indexer::{index_stream, init_apibara_stream};
use methods::Method;
use nuts::{
    nut04::MintMethodSettings, nut05::MeltMethodSettings, nut06::NutsSettings, Amount,
    QuoteTTLConfig,
};
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use sqlx::{migrate::Migrator, PgPool};
use starknet_types_core::felt::Felt;
use std::{net::Ipv6Addr, path::Path, str::FromStr};

mod app_state;
mod errors;
mod keyset_cache;
mod methods;
mod routes;
mod utils;
use app_state::AppState;
mod logic;

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
    dotenv().ok();

    let pg_pool = init().await;

    {
        let dna_token = dotenv::var("APIBARA_TOKEN").expect("missing `APIBARA_TOKEN` env variable");
        let starknet_token_address = Felt::from_hex_unchecked(
            "0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d",
        );
        let our_recipient_account = Felt::from_hex_unchecked(
            "0x07487f6e8fc8c60049e82cf8b6593211aeefef7efd0021db585c7e78cc29ac9a",
        );

        let stream = init_apibara_stream(
            dna_token,
            vec![(our_recipient_account, starknet_token_address)],
        )
        .await
        .unwrap();

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let _handle = tokio::spawn(index_stream(conn, stream));
    }

    // build our application with a route
    let app = Router::new()
        .route("/v1/swap", post(routes::swap))
        .route("/v1/mint/quote/{method}", post(routes::mint_quote))
        .route(
            "/v1/mint/quote/{method}/{quote_id}",
            get(routes::mint_quote_state),
        )
        .route("/v1/mint/{method}", post(routes::mint))
        .route("/v1/melt/quote/{method}", post(routes::melt_quote))
        .with_state(AppState::new(
            pg_pool,
            &[1, 2, 3, 4, 5],
            NutsSettings {
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
            },
            QuoteTTLConfig {
                mint_ttl: 3600,
                melt_ttl: 3600,
            },
        ));

    let rest_socket_address =
        std::net::SocketAddr::new(Ipv6Addr::LOCALHOST.into(), CASHU_REST_PORT);
    let listener = tokio::net::TcpListener::bind(rest_socket_address)
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Strk,
    StrkAtto,
}

// Used for derivation path
impl From<Unit> for u32 {
    fn from(value: Unit) -> Self {
        match value {
            Unit::Strk => 0,
            Unit::StrkAtto => 1,
        }
    }
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
                Unit::StrkAtto => "str:atto",
            },
            f,
        )
    }
}

impl nuts::traits::Unit for Unit {}

const STRK_TOKEN_PRECISION: u64 = 1_000_000_000_000_000_000;

impl Unit {
    pub fn convert_amount_into_u256(&self, amount: Amount) -> StarknetU256 {
        match self {
            Unit::Strk => {
                StarknetU256::from(U256::from(u64::from(amount)) * U256::from(STRK_TOKEN_PRECISION))
            }
            Unit::StrkAtto => StarknetU256::from(amount),
        }
    }

    pub fn convert_u256_into_amount(
        &self,
        amount: StarknetU256,
    ) -> Result<(Amount, StarknetU256), StarknetError> {
        match self {
            Unit::Strk => {
                let (quotient, rem) =
                    primitive_types::U256::from(&amount).div_mod(U256::from(STRK_TOKEN_PRECISION));
                Ok((
                    Amount::from(
                        u64::try_from(quotient)
                            .map_err(|_| StarknetError::StarknetAmountTooHigh(*self, amount))?,
                    ),
                    StarknetU256::from(rem),
                ))
            }
            Unit::StrkAtto => Ok((
                Amount::from(
                    u64::try_from(primitive_types::U256::from(&amount))
                        .map_err(|_| StarknetError::StarknetAmountTooHigh(*self, amount))?,
                ),
                StarknetU256::ZERO,
            )),
        }
    }

    pub fn is_asset_supported(&self, asset: Felt) -> bool {
        if asset == STRK_TOKEN_ADDRESS {
            match self {
                Unit::Strk => true,
                Unit::StrkAtto => true,
            }
        } else {
            false
        }
    }
}
