use anyhow::{Result, anyhow};
use node::{MintQuoteState, NodeClient};
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueHint};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(long, value_hint(ValueHint::FilePath))]
    db_path: Option<PathBuf>,
    #[arg(long, short, value_hint(ValueHint::Url))]
    node_url: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Mint new tokens
    Mint {
        #[arg(long, short)]
        amount: u64,
        #[arg(long, short)]
        unit: String,
    },

    /// Melt (burn) existing tokens
    Melt {
        /// Amount of tokens to melt
        #[arg(long)]
        amount: u64,

        /// Address to melt from
        #[arg(long)]
        from: String,
    },

    /// Swap tokens
    Swap {
        /// Amount of tokens to swap
        #[arg(long)]
        amount: u64,

        /// Token to swap from
        #[arg(long)]
        from_token: String,

        /// Token to swap to
        #[arg(long)]
        to_token: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut node_client = NodeClient::connect(cli.node_url.clone()).await?;
    let db_path = cli
        .db_path
        .or(dirs::data_dir().map(|mut dp| {
            dp.push("cli-wallet.sqlite3");
            dp
        }))
        .ok_or(anyhow!("couldn't find `data_dir` on this computer"))?;
    println!("database: {:?}", db_path);
    const STARKNET_METHOD: &str = "staknet";

    let mut db_conn = rusqlite::Connection::open(db_path)?;
    wallet::create_tables(&mut db_conn)?;

    match cli.command {
        Commands::Mint { amount, unit } => {
            println!("Asking {} to mint {} {}", cli.node_url, amount, unit);
            // Add mint logic here
            let mint_quote_response = wallet::create_mint_quote(
                &mut db_conn,
                &mut node_client,
                STARKNET_METHOD.to_string(),
                amount,
                unit,
            )
            .await?;

            println!(
                "MintQuote created with id: {}\nProceed to payment:\n{:?}",
                &mint_quote_response.quote, &mint_quote_response.request
            );

            loop {
                let state = wallet::get_mint_quote_state(
                    &mut db_conn,
                    &mut node_client,
                    STARKNET_METHOD.to_string(),
                    mint_quote_response.quote.clone(),
                )
                .await?;

                if state == MintQuoteState::MnqsPaid {
                    break;
                }
            }

            let outputs = todo!();

            wallet::mint(
                &mut node_client,
                STARKNET_METHOD.to_string(),
                mint_quote_response.quote,
                outputs,
            )
            .await?;
        }
        Commands::Melt { amount, from } => {
            println!("Melting {} tokens from {}", amount, from);
            // Add melt logic here
        }
        Commands::Swap {
            amount,
            from_token,
            to_token,
        } => {
            println!(
                "Swapping {} tokens from {} to {}",
                amount, from_token, to_token
            );
            // Add swap logic here
        }
    }

    Ok(())
}
