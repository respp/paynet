pub use proto::cashu_core::{BlindSignature, BlindedMessage, Proof};
pub use proto::cashu_node::node_client::NodeClient;
pub use proto::cashu_node::node_server::{Node, NodeServer};
pub use proto::cashu_node::*;

mod proto {
    pub mod cashu_core {
        tonic::include_proto!("cashu_core");
    }
    pub mod cashu_node {
        tonic::include_proto!("cashu_node");
    }
}
