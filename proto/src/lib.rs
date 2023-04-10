pub mod eclipse {
    pub mod ibc {
        pub mod admin {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/eclipse.ibc.admin.v1.rs"));
            }
        }

        pub mod chain {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/eclipse.ibc.chain.v1.rs"));
            }
        }

        pub mod client {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/eclipse.ibc.client.v1.rs"));
            }
        }

        pub mod port {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/eclipse.ibc.port.v1.rs"));
            }
        }
    }
}
