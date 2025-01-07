mod amount;
pub mod dhke;
mod mint_url;
pub mod nut00;
pub mod nut01;
pub mod nut02;
pub mod nut03;
pub use amount::*;

use bitcoin::secp256k1::{rand, All, Secp256k1};
use once_cell::sync::Lazy;

pub mod traits {
    use std::{
        fmt::{Debug, Display},
        str::FromStr,
    };

    pub trait Unit: FromStr + Sized + Debug + Copy + Clone + Display + Into<u32> {}

    #[cfg(test)]
    pub mod test_types {
        use std::{fmt::Display, str::FromStr};

        use crate::Error;

        use super::Unit;

        #[derive(Debug, Copy, Clone, serde::Serialize, serde::Deserialize)]
        pub enum CurrencyUnit {
            Sat,
            Msat,
            Usd,
            Eur,
        }

        impl Display for CurrencyUnit {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                Display::fmt(
                    match self {
                        CurrencyUnit::Sat => "sat",
                        CurrencyUnit::Msat => "msat",
                        CurrencyUnit::Usd => "usd",
                        CurrencyUnit::Eur => "eur",
                    },
                    f,
                )
            }
        }

        impl From<CurrencyUnit> for u32 {
            fn from(value: CurrencyUnit) -> Self {
                match value {
                    CurrencyUnit::Sat => 0,
                    CurrencyUnit::Msat => 1,
                    CurrencyUnit::Usd => 2,
                    CurrencyUnit::Eur => 3,
                }
            }
        }

        impl FromStr for CurrencyUnit {
            type Err = Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let unit = match s {
                    "sat" => CurrencyUnit::Sat,
                    "msat" => CurrencyUnit::Msat,
                    "usd" => CurrencyUnit::Usd,
                    "eur" => CurrencyUnit::Eur,
                    _ => return Err(Error::CannotConvertUnits),
                };
                Ok(unit)
            }
        }

        impl Unit for CurrencyUnit {}
    }
}

/// Secp256k1 global context
pub static SECP256K1: Lazy<Secp256k1<All>> = Lazy::new(|| {
    let mut ctx = Secp256k1::new();
    let mut rng = rand::thread_rng();
    ctx.randomize(&mut rng);
    ctx
});
