// @generated
pub mod invoice_contract {
    // @@protoc_insertion_point(attribute:invoice_contract.v1)
    pub mod v1 {
        include!("invoice_contract.v1.rs");
        // @@protoc_insertion_point(invoice_contract.v1)
    }
}
pub mod sf {
    pub mod starknet {
        pub mod r#type {
            // @@protoc_insertion_point(attribute:sf.starknet.type.v1)
            pub mod v1 {
                include!("sf.starknet.type.v1.rs");
                // @@protoc_insertion_point(sf.starknet.type.v1)
            }
        }
    }
    // @@protoc_insertion_point(attribute:sf.substreams)
    pub mod substreams {
        include!("sf.substreams.rs");
        // @@protoc_insertion_point(sf.substreams)
        pub mod index {
            // @@protoc_insertion_point(attribute:sf.substreams.index.v1)
            pub mod v1 {
                include!("sf.substreams.index.v1.rs");
                // @@protoc_insertion_point(sf.substreams.index.v1)
            }
        }
        pub mod starknet {
            pub mod r#type {
                // @@protoc_insertion_point(attribute:sf.substreams.starknet.type.v1)
                pub mod v1 {
                    include!("sf.substreams.starknet.type.v1.rs");
                    // @@protoc_insertion_point(sf.substreams.starknet.type.v1)
                }
            }
        }
        // @@protoc_insertion_point(attribute:sf.substreams.v1)
        pub mod v1 {
            include!("sf.substreams.v1.rs");
            // @@protoc_insertion_point(sf.substreams.v1)
        }
    }
}
