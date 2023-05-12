use {
    ibc::{
        core::{
            ics04_channel::{
                channel::{Counterparty, Order},
                packet::{Acknowledgement, Packet},
                Version,
            },
            ics24_host::identifier::{ChannelId, ConnectionId, PortId},
        },
        Signer,
    },
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnChanOpenInitValidate {
    pub order: Order,
    pub connection_hops: Vec<ConnectionId>,
    pub port_id: PortId,
    pub channel_id: ChannelId,
    pub counterparty: Counterparty,
    pub version: Version,
} // -> Result<Version, ChannelError>

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnChanOpenInitExecute {
    pub order: Order,
    pub connection_hops: Vec<ConnectionId>,
    pub port_id: PortId,
    pub channel_id: ChannelId,
    pub counterparty: Counterparty,
    pub version: Version,
} // -> Result<(ModuleExtras, Version), ChannelError>

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnChanOpenTryValidate {
    pub order: Order,
    pub connection_hops: Vec<ConnectionId>,
    pub port_id: PortId,
    pub channel_id: ChannelId,
    pub counterparty: Counterparty,
    pub counterparty_version: Version,
} // -> Result<Version, ChannelError>

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnChanOpenTryExecute {
    pub order: Order,
    pub connection_hops: Vec<ConnectionId>,
    pub port_id: PortId,
    pub channel_id: ChannelId,
    pub counterparty: Counterparty,
    pub counterparty_version: Version,
} // -> Result<(ModuleExtras, Version), ChannelError>

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnChanOpenAckValidate {
    pub port_id: PortId,
    pub channel_id: ChannelId,
    pub counterparty_version: Version,
} // -> Result<(), ChannelError>

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnChanOpenAckExecute {
    pub port_id: PortId,
    pub channel_id: ChannelId,
    pub counterparty_version: Version,
} // -> Result<ModuleExtras, ChannelError>

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnChanOpenConfirmValidate {
    pub port_id: PortId,
    pub channel_id: ChannelId,
} // -> Result<(), ChannelError>

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnChanOpenConfirmExecute {
    pub port_id: PortId,
    pub channel_id: ChannelId,
} // -> Result<ModuleExtras, ChannelError>

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnChanCloseInitValidate {
    pub port_id: PortId,
    pub channel_id: ChannelId,
} // -> Result<(), ChannelError>

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnChanCloseInitExecute {
    pub port_id: PortId,
    pub channel_id: ChannelId,
} // -> Result<ModuleExtras, ChannelError>

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnChanCloseConfirmValidate {
    pub port_id: PortId,
    pub channel_id: ChannelId,
} // -> Result<(), ChannelError>

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnChanCloseConfirmExecute {
    pub port_id: PortId,
    pub channel_id: ChannelId,
} // -> Result<ModuleExtras, ChannelError>

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnRecvPacketExecute {
    pub packet: Packet,
    pub relayer: Signer,
} // -> (ModuleExtras, Acknowledgement)

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnAcknowledgementPacketValidate {
    pub packet: Packet,
    pub acknowledgement: Acknowledgement,
    pub relayer: Signer,
} // -> Result<(), PacketError>

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnAcknowledgementPacketExecute {
    pub packet: Packet,
    pub acknowledgement: Acknowledgement,
    pub relayer: Signer,
} // -> (ModuleExtras, Result<(), PacketError>)

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnTimeoutPacketValidate {
    pub packet: Packet,
    pub relayer: Signer,
} // -> Result<(), PacketError>

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnTimeoutPacketExecute {
    pub packet: Packet,
    pub relayer: Signer,
} // -> (ModuleExtras, Result<(), PacketError>)

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum IbcModuleInstruction {
    OnChanOpenInitValidate(OnChanOpenInitValidate),
    OnChanOpenInitExecute(OnChanOpenInitExecute),
    OnChanOpenTryValidate(OnChanOpenTryValidate),
    OnChanOpenTryExecute(OnChanOpenTryExecute),
    OnChanOpenAckValidate(OnChanOpenAckValidate),
    OnChanOpenAckExecute(OnChanOpenAckExecute),
    OnChanOpenConfirmValidate(OnChanOpenConfirmValidate),
    OnChanOpenConfirmExecute(OnChanOpenConfirmExecute),
    OnChanCloseInitValidate(OnChanCloseInitValidate),
    OnChanCloseInitExecute(OnChanCloseInitExecute),
    OnChanCloseConfirmValidate(OnChanCloseConfirmValidate),
    OnChanCloseConfirmExecute(OnChanCloseConfirmExecute),
    OnRecvPacketExecute(OnRecvPacketExecute),
    OnAcknowledgementPacketValidate(OnAcknowledgementPacketValidate),
    OnAcknowledgementPacketExecute(OnAcknowledgementPacketExecute),
    OnTimeoutPacketValidate(OnTimeoutPacketValidate),
    OnTimeoutPacketExecute(OnTimeoutPacketExecute),
}
