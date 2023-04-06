use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct IbcMetadata {
    pub client_id_counter: u64,
    pub connection_id_counter: u64,
    pub channel_id_counter: u64,
}
