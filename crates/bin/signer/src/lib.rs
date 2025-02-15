mod methods;
pub use methods::Method;

pub use proto::cashu_core::{BlindSignature, BlindedMessage, Proof};
pub use proto::cashu_signer::signer_client::SignerClient;
pub use proto::cashu_signer::signer_server::{Signer, SignerServer};
pub use proto::cashu_signer::*;

// impl From<nut00::BlindSignature> for BlindSignature {
//     fn from(value: nut00::BlindSignature) -> Self {
//         BlindSignature {
//             amount: value.amount.into(),
//             keyset_id: value.keyset_id.to_bytes().to_vec(),
//             c: value.c.to_bytes().to_vec(),
//         }
//     }
// }

// #[derive(Debug, Error)]
// pub enum TryBlindSignatureFromProtobuf {
//     #[error("Invalid value for field keyset_id: {0}")]
//     KeysetId(nut02::Error),
//     #[error("Invalid value for field c: {0}")]
//     C(nut01::Error),
// }

// impl TryFrom<&BlindSignature> for nut00::BlindSignature {
//     type Error = TryBlindSignatureFromProtobuf;

//     fn try_from(value: &BlindSignature) -> Result<Self, Self::Error> {
//         let blind_signature = nut00::BlindSignature {
//             amount: value.amount.into(),
//             keyset_id: KeysetId::from_bytes(&value.keyset_id)
//                 .map_err(TryBlindSignatureFromProtobuf::KeysetId)?,
//             c: PublicKey::from_slice(&value.c).map_err(TryBlindSignatureFromProtobuf::C)?,
//         };

//         Ok(blind_signature)
//     }
// }

mod proto {
    pub mod cashu_core {
        tonic::include_proto!("cashu_core");
    }
    pub mod cashu_signer {
        tonic::include_proto!("cashu_signer");
    }
}
