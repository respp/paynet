use bitcoin::{
    bip32::{ChildNumber, DerivationPath, Xpriv},
    key::Secp256k1,
};
use nuts::{
    nut02::MintKeySet,
    traits::{self, Unit},
};

#[derive(Debug, Clone)]
pub struct KeysManager {
    root_private_key: Xpriv,
}

impl KeysManager {
    pub fn new(seed: &[u8]) -> Self {
        let xpriv = Xpriv::new_master(bitcoin::Network::Bitcoin, seed).expect("RNG busted");

        Self {
            root_private_key: xpriv,
        }
    }

    pub fn generate_keyset<U: traits::Unit>(
        &self,
        unit: U,
        index: u32,
        max_order: u8,
    ) -> MintKeySet<U> {
        let secp_ctx = Secp256k1::new();
        let derivation_path = derivation_path_from_unit(unit, index);

        let xpriv = self
            .root_private_key
            .derive_priv(&secp_ctx, &derivation_path)
            .expect("RNG busted");

        MintKeySet::generate(&secp_ctx, xpriv, unit, max_order)
    }
}

fn derivation_path_from_unit<U: Unit>(unit: U, index: u32) -> DerivationPath {
    let unit_index: u32 = unit.into();

    DerivationPath::from(vec![
        ChildNumber::from_hardened_idx(0).expect("0 is a valid index"),
        ChildNumber::from_hardened_idx(unit_index).expect("should be a valid index"),
        ChildNumber::from_hardened_idx(index).expect("should be a valid index"),
    ])
}
