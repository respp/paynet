mod amount;
mod mint_url;
mod types;

pub mod dhke;
pub mod nut00;
pub mod nut01;
pub mod nut02;
pub mod nut03;
pub mod nut04;
pub mod nut05;
pub mod nut06;

pub use amount::*;
pub use types::*;

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

        #[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum TestUnit {
            Sat,
            Msat,
            Usd,
            Eur,
        }

        impl Display for TestUnit {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                Display::fmt(
                    match self {
                        TestUnit::Sat => "sat",
                        TestUnit::Msat => "msat",
                        TestUnit::Usd => "usd",
                        TestUnit::Eur => "eur",
                    },
                    f,
                )
            }
        }

        impl From<TestUnit> for u32 {
            fn from(value: TestUnit) -> Self {
                match value {
                    TestUnit::Sat => 0,
                    TestUnit::Msat => 1,
                    TestUnit::Usd => 2,
                    TestUnit::Eur => 3,
                }
            }
        }

        impl FromStr for TestUnit {
            type Err = Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let unit = match s {
                    "sat" => TestUnit::Sat,
                    "msat" => TestUnit::Msat,
                    "usd" => TestUnit::Usd,
                    "eur" => TestUnit::Eur,
                    _ => return Err(Error::CannotConvertUnits),
                };
                Ok(unit)
            }
        }

        impl Unit for TestUnit {}

        #[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum TestMethod {
            Bolt11,
        }
    }
}

/// Secp256k1 global context
pub static SECP256K1: Lazy<Secp256k1<All>> = Lazy::new(|| {
    let mut ctx = Secp256k1::new();
    let mut rng = rand::thread_rng();
    ctx.randomize(&mut rng);
    ctx
});

#[derive(
    Debug, Clone, Copy, Hash, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "UPPERCASE")]
pub enum QuoteState {
    /// Quote has not been paid
    #[default]
    Unpaid,
    /// Quote has been paid and wallet can mint
    Paid,
    /// ecash issued for quote
    Issued,
    ///
    Failed,
}
