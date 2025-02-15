use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Starknet,
}

impl core::fmt::Display for Method {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Method::Starknet => core::fmt::Display::fmt(&cashu_starknet::Method, f),
        }
    }
}

impl From<Method> for &'static str {
    fn from(value: Method) -> Self {
        match value {
            Method::Starknet => cashu_starknet::Method.into(),
        }
    }
}

impl FromStr for Method {
    type Err = <cashu_starknet::Method as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        cashu_starknet::Method::from_str(s).map(|_| Method::Starknet)
    }
}
