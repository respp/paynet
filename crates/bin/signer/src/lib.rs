mod methods;
pub use methods::Method;

pub use proto::cashu_core::{BlindSignature, BlindedMessage, Proof};
pub use proto::cashu_signer::signer_client::SignerClient;
pub use proto::cashu_signer::signer_server::{Signer, SignerServer};
pub use proto::cashu_signer::*;

mod proto {
    pub mod cashu_core {
        tonic::include_proto!("cashu_core");
    }
    pub mod cashu_signer {
        tonic::include_proto!("cashu_signer");
    }
}
