use {
    eclipse_ibc_proto::eclipse::ibc::client::v1::ClientConnections as RawClientConnections,
    ibc::core::ics24_host::{error::ValidationError, identifier::ConnectionId},
    known_proto::KnownProtoWithFrom,
    std::collections::HashSet,
};

#[derive(Clone, Debug, Default)]
pub struct ClientConnections {
    pub connections: HashSet<ConnectionId>,
}

impl From<ClientConnections> for RawClientConnections {
    fn from(ClientConnections { connections }: ClientConnections) -> Self {
        Self {
            connections: connections
                .into_iter()
                .map(|connection_id| ConnectionId::to_string(&connection_id))
                .collect(),
        }
    }
}

impl TryFrom<RawClientConnections> for ClientConnections {
    type Error = ValidationError;

    fn try_from(
        RawClientConnections { connections }: RawClientConnections,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            connections: connections
                .into_iter()
                .map(|connection_id_str| connection_id_str.parse())
                .collect::<Result<_, _>>()?,
        })
    }
}

impl KnownProtoWithFrom for ClientConnections {
    type RawWithFrom = RawClientConnections;
}
