use {
    crate::module_instruction::*,
    anyhow::anyhow,
    core::ops::Bound::{Excluded, Unbounded},
    eclipse_ibc_extra_types::{AllModuleIds, ClientConnections, ConsensusHeights},
    eclipse_ibc_light_client::{eclipse_chain, EclipseConsensusState},
    eclipse_ibc_state::{
        decode_client_state, decode_consensus_state, encode_client_state, encode_consensus_state,
        internal_path::{
            AllModulesPath, ClientUpdateHeightPath, ClientUpdateTimePath, ConsensusHeightsPath,
        },
        IbcMetadata, IbcState, IbcStore,
    },
    ibc::{
        core::{
            context::{ContextError, ExecutionContext, Router, ValidationContext},
            ics02_client::{
                client_state::ClientState, consensus_state::ConsensusState, error::ClientError,
                height::Height,
            },
            ics03_connection::{connection::ConnectionEnd, error::ConnectionError},
            ics04_channel::{
                channel::{ChannelEnd, Counterparty, Order},
                commitment::{AcknowledgementCommitment, PacketCommitment},
                error::{ChannelError, PacketError},
                handler::ModuleExtras,
                msgs::acknowledgement::Acknowledgement,
                packet::{Packet, Receipt, Sequence},
                Version,
            },
            ics05_port::error::PortError,
            ics23_commitment::commitment::{CommitmentPrefix, CommitmentRoot},
            ics24_host::{
                identifier::{ChannelId, ClientId, ConnectionId, PortId},
                path::{
                    AckPath, ChannelEndPath, ClientConnectionPath, ClientConsensusStatePath,
                    ClientStatePath, CommitmentPath, ConnectionPath, PortPath, ReceiptPath,
                    SeqAckPath, SeqRecvPath, SeqSendPath,
                },
            },
            ics26_routing::context::{Module, ModuleId},
        },
        events::IbcEvent,
        signer::Signer,
        timestamp::Timestamp,
    },
    ibc_proto::google::protobuf,
    solana_sdk::{
        clock::Slot,
        instruction::Instruction,
        msg,
        program::{get_return_data, invoke},
        pubkey::Pubkey,
        sysvar::{clock::Clock, slot_hashes::SlotHashes},
    },
    std::{collections::BTreeMap, sync::Arc, time::Duration},
    tendermint::time::Time as TendermintTime,
};

#[derive(Debug)]
pub(super) struct IbcHandler<'a> {
    state: IbcState<'a>,
    metadata: &'a mut IbcMetadata,
    current_slot: Slot,
    current_time: TendermintTime,
    slot_hashes: Arc<SlotHashes>,
    max_expected_time_per_block: Duration,
    module_by_id: BTreeMap<ModuleId, Box<dyn Module>>,
}

const COMMITMENT_PREFIX: &str = "ibc";

impl<'a> IbcHandler<'a> {
    pub(super) fn new(
        store: &'a IbcStore,
        metadata: &'a mut IbcMetadata,
        clock: &Clock,
        slot_hashes: Arc<SlotHashes>,
    ) -> anyhow::Result<Self> {
        let state = IbcState::new(store, clock.slot);
        let all_module_ids: AllModuleIds = state.get(&AllModulesPath)?.unwrap_or_default();
        let module_by_id = all_module_ids
            .modules
            .into_iter()
            .map(|module_id| {
                let program_id = pubkey_of_module_id(&module_id)?;
                Ok((module_id, SolanaModule { program_id }.into_box()))
            })
            .collect::<anyhow::Result<_>>()?;

        Ok(Self {
            state,
            metadata,
            current_slot: clock.slot,
            current_time: eclipse_chain::tendermint_time_from_clock(clock),
            slot_hashes,
            max_expected_time_per_block: eclipse_chain::MAX_EXPECTED_SLOT_TIME,
            module_by_id,
        })
    }

    fn consensus_state(&self, slot: Slot) -> Option<Box<dyn ConsensusState>> {
        let hash = self.slot_hashes.get(&slot)?;
        Some(Box::new(EclipseConsensusState {
            commitment_root: CommitmentRoot::from_bytes(hash.as_ref()),
            // TODO: Adjust the time based on the slot
            timestamp: self.current_time,
        }))
    }

    pub(super) fn commit(&mut self) -> anyhow::Result<()> {
        self.state.commit()
    }
}

impl<'a> ExecutionContext for IbcHandler<'a> {
    fn store_client_state(
        &mut self,
        client_state_path: ClientStatePath,
        client_state: Box<dyn ClientState>,
    ) -> Result<(), ContextError> {
        self.state
            .set(&client_state_path, encode_client_state(client_state)?);
        Ok(())
    }

    fn store_consensus_state(
        &mut self,
        consensus_state_path: ClientConsensusStatePath,
        consensus_state: Box<dyn ConsensusState>,
    ) -> Result<(), ContextError> {
        let ClientConsensusStatePath {
            client_id,
            epoch: revision_number,
            height: revision_height,
        } = &consensus_state_path;
        let height = Height::new(*revision_number, *revision_height)?;

        let consensus_heights_path = ConsensusHeightsPath(client_id.clone());
        self.state
            .update(
                &consensus_heights_path,
                |consensus_heights: &mut ConsensusHeights| {
                    consensus_heights.heights.insert(height);
                },
            )
            .map_err(|err| ClientError::Other {
                description: err.to_string(),
            })?;

        self.state.set(
            &consensus_state_path,
            encode_consensus_state(consensus_state)?,
        );
        Ok(())
    }

    fn increase_client_counter(&mut self) {
        self.metadata.client_id_counter += 1;
    }

    fn store_update_time(
        &mut self,
        client_id: ClientId,
        height: Height,
        timestamp: Timestamp,
    ) -> Result<(), ContextError> {
        let client_update_time_path = ClientUpdateTimePath(client_id, height);
        self.state.set(&client_update_time_path, timestamp);
        Ok(())
    }

    fn store_update_height(
        &mut self,
        client_id: ClientId,
        height: Height,
        host_height: Height,
    ) -> Result<(), ContextError> {
        let client_update_height_path = ClientUpdateHeightPath(client_id, height);
        self.state.set(&client_update_height_path, host_height);
        Ok(())
    }

    fn store_connection(
        &mut self,
        connection_path: &ConnectionPath,
        connection_end: ConnectionEnd,
    ) -> Result<(), ContextError> {
        self.state.set(connection_path, connection_end);
        Ok(())
    }

    fn store_connection_to_client(
        &mut self,
        client_connection_path: &ClientConnectionPath,
        connection_id: ConnectionId,
    ) -> Result<(), ContextError> {
        self.state
            .update(
                client_connection_path,
                |client_connections: &mut ClientConnections| {
                    client_connections.connections.insert(connection_id);
                },
            )
            .map_err(|err| ConnectionError::Other {
                description: err.to_string(),
            })?;
        Ok(())
    }

    fn increase_connection_counter(&mut self) {
        self.metadata.connection_id_counter += 1;
    }

    fn store_packet_commitment(
        &mut self,
        commitment_path: &CommitmentPath,
        commitment: PacketCommitment,
    ) -> Result<(), ContextError> {
        self.state.set(commitment_path, commitment);
        Ok(())
    }

    fn delete_packet_commitment(
        &mut self,
        commitment_path: &CommitmentPath,
    ) -> Result<(), ContextError> {
        self.state.remove(commitment_path);
        Ok(())
    }

    fn store_packet_receipt(
        &mut self,
        receipt_path: &ReceiptPath,
        receipt: Receipt,
    ) -> Result<(), ContextError> {
        self.state.set(receipt_path, receipt);
        Ok(())
    }

    fn store_packet_acknowledgement(
        &mut self,
        ack_path: &AckPath,
        ack_commitment: AcknowledgementCommitment,
    ) -> Result<(), ContextError> {
        self.state.set(ack_path, ack_commitment);
        Ok(())
    }

    fn delete_packet_acknowledgement(&mut self, ack_path: &AckPath) -> Result<(), ContextError> {
        self.state.remove(ack_path);
        Ok(())
    }

    fn store_channel(
        &mut self,
        channel_end_path: &ChannelEndPath,
        channel_end: ChannelEnd,
    ) -> Result<(), ContextError> {
        self.state.set(channel_end_path, channel_end);
        Ok(())
    }

    fn store_next_sequence_send(
        &mut self,
        seq_send_path: &SeqSendPath,
        seq: Sequence,
    ) -> Result<(), ContextError> {
        self.state.set(seq_send_path, seq);
        Ok(())
    }

    fn store_next_sequence_recv(
        &mut self,
        seq_recv_path: &SeqRecvPath,
        seq: Sequence,
    ) -> Result<(), ContextError> {
        self.state.set(seq_recv_path, seq);
        Ok(())
    }

    fn store_next_sequence_ack(
        &mut self,
        seq_ack_path: &SeqAckPath,
        seq: Sequence,
    ) -> Result<(), ContextError> {
        self.state.set(seq_ack_path, seq);
        Ok(())
    }

    fn increase_channel_counter(&mut self) {
        self.metadata.channel_id_counter += 1;
    }

    // TODO: Figure out where to emit IBC events
    fn emit_ibc_event(&mut self, event: IbcEvent) {
        msg!("{:?}", event);
    }

    // TODO: Figure out where to log IBC messages
    fn log_message(&mut self, message: String) {
        msg!(&message);
    }
}

impl<'a> ValidationContext for IbcHandler<'a> {
    fn client_state(&self, client_id: &ClientId) -> Result<Box<dyn ClientState>, ContextError> {
        let client_state_path = ClientStatePath::new(client_id);
        self.decode_client_state(
            self.state
                .get(&client_state_path)
                .map_err(|err| ClientError::Other {
                    description: err.to_string(),
                })?
                .ok_or_else(|| ClientError::ClientStateNotFound {
                    client_id: client_id.clone(),
                })?,
        )
    }

    fn decode_client_state(
        &self,
        client_state: protobuf::Any,
    ) -> Result<Box<dyn ClientState>, ContextError> {
        decode_client_state(client_state)
    }

    fn consensus_state(
        &self,
        client_consensus_path: &ClientConsensusStatePath,
    ) -> Result<Box<dyn ConsensusState>, ContextError> {
        let ClientConsensusStatePath {
            client_id,
            epoch: revision_number,
            height: revision_height,
        } = client_consensus_path;
        let height = Height::new(*revision_number, *revision_height)?;

        decode_consensus_state(
            self.state
                .get(client_consensus_path)
                .map_err(|err| ClientError::Other {
                    description: err.to_string(),
                })?
                .ok_or_else(|| ClientError::ConsensusStateNotFound {
                    client_id: client_id.clone(),
                    height,
                })?,
        )
    }

    fn next_consensus_state(
        &self,
        client_id: &ClientId,
        height: &Height,
    ) -> Result<Option<Box<dyn ConsensusState>>, ContextError> {
        let consensus_heights_path = ConsensusHeightsPath(client_id.clone());

        let consensus_heights: Option<ConsensusHeights> = self
            .state
            .get(&consensus_heights_path)
            .map_err(|err| ClientError::Other {
            description: err.to_string(),
        })?;

        let consensus_heights = match consensus_heights {
            None => return Ok(None),
            Some(consensus_heights) => consensus_heights,
        };

        let next_consensus_height = match consensus_heights
            .heights
            .range((Excluded(*height), Unbounded))
            .next()
        {
            None => return Ok(None),
            Some(next_consensus_height) => *next_consensus_height,
        };

        let client_consensus_path = ClientConsensusStatePath {
            client_id: client_id.clone(),
            epoch: next_consensus_height.revision_number(),
            height: next_consensus_height.revision_height(),
        };

        Ok(Some(decode_consensus_state(
            self.state
                .get(&client_consensus_path)
                .map_err(|err| ClientError::Other {
                    description: err.to_string(),
                })?
                .ok_or_else(|| ClientError::ConsensusStateNotFound {
                    client_id: client_id.clone(),
                    height: next_consensus_height,
                })?,
        )?))
    }

    fn prev_consensus_state(
        &self,
        client_id: &ClientId,
        height: &Height,
    ) -> Result<Option<Box<dyn ConsensusState>>, ContextError> {
        let consensus_heights_path = ConsensusHeightsPath(client_id.clone());

        let consensus_heights: Option<ConsensusHeights> = self
            .state
            .get(&consensus_heights_path)
            .map_err(|err| ClientError::Other {
            description: err.to_string(),
        })?;

        let consensus_heights = match consensus_heights {
            None => return Ok(None),
            Some(consensus_heights) => consensus_heights,
        };

        let next_consensus_height = match consensus_heights.heights.range(..*height).next_back() {
            None => return Ok(None),
            Some(next_consensus_height) => *next_consensus_height,
        };

        let client_consensus_path = ClientConsensusStatePath {
            client_id: client_id.clone(),
            epoch: next_consensus_height.revision_number(),
            height: next_consensus_height.revision_height(),
        };

        Ok(Some(decode_consensus_state(
            self.state
                .get(&client_consensus_path)
                .map_err(|err| ClientError::Other {
                    description: err.to_string(),
                })?
                .ok_or_else(|| ClientError::ConsensusStateNotFound {
                    client_id: client_id.clone(),
                    height: next_consensus_height,
                })?,
        )?))
    }

    fn host_height(&self) -> Result<Height, ContextError> {
        Ok(eclipse_chain::height_of_slot(self.current_slot)?)
    }

    fn host_timestamp(&self) -> Result<Timestamp, ContextError> {
        Ok(self.current_time.into())
    }

    fn host_consensus_state(
        &self,
        height: &Height,
    ) -> Result<Box<dyn ConsensusState>, ContextError> {
        let slot = eclipse_chain::slot_of_height(*height)?;
        Ok(self
            .consensus_state(slot)
            .ok_or(ClientError::MissingLocalConsensusState { height: *height })?)
    }

    fn client_counter(&self) -> Result<u64, ContextError> {
        Ok(self.metadata.client_id_counter)
    }

    fn connection_end(&self, connection_id: &ConnectionId) -> Result<ConnectionEnd, ContextError> {
        let connection_path = ConnectionPath(connection_id.clone());
        Ok(self
            .state
            .get(&connection_path)
            .map_err(|err| ConnectionError::Other {
                description: err.to_string(),
            })?
            .ok_or_else(|| ConnectionError::ConnectionNotFound {
                connection_id: connection_id.clone(),
            })?)
    }

    fn validate_self_client(
        &self,
        _counterparty_client_state: protobuf::Any,
    ) -> Result<(), ContextError> {
        // TODO: Figure out how to actually validate `counterparty_client_state`
        Ok(())
    }

    fn commitment_prefix(&self) -> CommitmentPrefix {
        COMMITMENT_PREFIX
            .to_owned()
            .into_bytes()
            .try_into()
            .expect("Prefix is not empty")
    }

    fn connection_counter(&self) -> Result<u64, ContextError> {
        Ok(self.metadata.connection_id_counter)
    }

    fn channel_end(&self, channel_end_path: &ChannelEndPath) -> Result<ChannelEnd, ContextError> {
        Ok(self
            .state
            .get(channel_end_path)
            .map_err(|err| ChannelError::Other {
                description: err.to_string(),
            })?
            .ok_or_else(|| {
                let ChannelEndPath(port_id, channel_id) = channel_end_path;
                ChannelError::ChannelNotFound {
                    port_id: port_id.clone(),
                    channel_id: channel_id.clone(),
                }
            })?)
    }

    fn get_next_sequence_send(
        &self,
        seq_send_path: &SeqSendPath,
    ) -> Result<Sequence, ContextError> {
        Ok(self
            .state
            .get(seq_send_path)
            // TODO: Fix the IBC library to include an error message
            .map_err(|_err| PacketError::ImplementationSpecific)?
            .ok_or_else(|| {
                let SeqSendPath(port_id, channel_id) = seq_send_path;
                PacketError::MissingNextSendSeq {
                    port_id: port_id.clone(),
                    channel_id: channel_id.clone(),
                }
            })?)
    }

    fn get_next_sequence_recv(
        &self,
        seq_recv_path: &SeqRecvPath,
    ) -> Result<Sequence, ContextError> {
        Ok(self
            .state
            .get(seq_recv_path)
            // TODO: Fix the IBC library to include an error message
            .map_err(|_err| PacketError::ImplementationSpecific)?
            .ok_or_else(|| {
                let SeqRecvPath(port_id, channel_id) = seq_recv_path;
                PacketError::MissingNextRecvSeq {
                    port_id: port_id.clone(),
                    channel_id: channel_id.clone(),
                }
            })?)
    }

    fn get_next_sequence_ack(&self, seq_ack_path: &SeqAckPath) -> Result<Sequence, ContextError> {
        Ok(self
            .state
            .get(seq_ack_path)
            // TODO: Fix the IBC library to include an error message
            .map_err(|_err| PacketError::ImplementationSpecific)?
            .ok_or_else(|| {
                let SeqAckPath(port_id, channel_id) = seq_ack_path;
                PacketError::MissingNextAckSeq {
                    port_id: port_id.clone(),
                    channel_id: channel_id.clone(),
                }
            })?)
    }

    fn get_packet_commitment(
        &self,
        commitment_path: &CommitmentPath,
    ) -> Result<PacketCommitment, ContextError> {
        Ok(self
            .state
            .get(commitment_path)
            // TODO: Fix the IBC library to include an error message
            .map_err(|_err| PacketError::ImplementationSpecific)?
            .ok_or_else(|| {
                let CommitmentPath {
                    port_id: _,
                    channel_id: _,
                    sequence,
                } = commitment_path;
                PacketError::PacketCommitmentNotFound {
                    sequence: *sequence,
                }
            })?)
    }

    fn get_packet_receipt(&self, receipt_path: &ReceiptPath) -> Result<Receipt, ContextError> {
        Ok(self
            .state
            .get(receipt_path)
            // TODO: Fix the IBC library to include an error message
            .map_err(|_err| PacketError::ImplementationSpecific)?
            .ok_or_else(|| {
                let ReceiptPath {
                    port_id: _,
                    channel_id: _,
                    sequence,
                } = receipt_path;
                PacketError::PacketReceiptNotFound {
                    sequence: *sequence,
                }
            })?)
    }

    fn get_packet_acknowledgement(
        &self,
        ack_path: &AckPath,
    ) -> Result<AcknowledgementCommitment, ContextError> {
        Ok(self
            .state
            .get(ack_path)
            // TODO: Fix the IBC library to include an error message
            .map_err(|_err| PacketError::ImplementationSpecific)?
            .ok_or_else(|| {
                let AckPath {
                    port_id: _,
                    channel_id: _,
                    sequence,
                } = ack_path;
                PacketError::PacketAcknowledgementNotFound {
                    sequence: *sequence,
                }
            })?)
    }

    fn client_update_time(
        &self,
        client_id: &ClientId,
        height: &Height,
    ) -> Result<Timestamp, ContextError> {
        let client_update_time_path = ClientUpdateTimePath(client_id.clone(), *height);
        Ok(self
            .state
            .get(&client_update_time_path)
            .map_err(|err| ChannelError::Other {
                description: err.to_string(),
            })?
            .ok_or_else(|| ChannelError::ProcessedTimeNotFound {
                client_id: client_id.clone(),
                height: *height,
            })?)
    }

    fn client_update_height(
        &self,
        client_id: &ClientId,
        height: &Height,
    ) -> Result<Height, ContextError> {
        let client_update_height_path = ClientUpdateHeightPath(client_id.clone(), *height);
        Ok(self
            .state
            .get(&client_update_height_path)
            .map_err(|err| ChannelError::Other {
                description: err.to_string(),
            })?
            .ok_or_else(|| ChannelError::ProcessedHeightNotFound {
                client_id: client_id.clone(),
                height: *height,
            })?)
    }

    fn channel_counter(&self) -> Result<u64, ContextError> {
        Ok(self.metadata.channel_id_counter)
    }

    fn max_expected_time_per_block(&self) -> Duration {
        self.max_expected_time_per_block
    }
}

fn module_id_of_pubkey(pubkey: &Pubkey) -> ModuleId {
    ModuleId::new(hex::encode(pubkey.as_ref()).into())
        .expect("Hex pubkeys should always be alphanumeric")
}

fn pubkey_of_module_id(module_id: &ModuleId) -> anyhow::Result<Pubkey> {
    Pubkey::try_from(hex::decode(module_id.to_string())?)
        .map_err(|bytes| anyhow!("Failed to decode pubkey from bytes: {bytes:?}"))
}

impl<'a> Router for IbcHandler<'a> {
    fn get_route(&self, module_id: &ModuleId) -> Option<&dyn Module> {
        self.module_by_id.get(module_id).map(|m| &**m)
    }

    fn get_route_mut(&mut self, module_id: &ModuleId) -> Option<&mut dyn Module> {
        // Must be manually expanded due to a compiler bug
        match self.module_by_id.get_mut(module_id) {
            Some(m) => Some(&mut **m),
            None => None,
        }
    }

    fn has_route(&self, module_id: &ModuleId) -> bool {
        self.module_by_id.contains_key(module_id)
    }

    fn lookup_module_by_port(&self, port_id: &PortId) -> Option<ModuleId> {
        let port_path = PortPath(port_id.clone());
        self.state.get(&port_path).ok().flatten()
    }
}

impl<'a> IbcHandler<'a> {
    pub(super) fn bind_port(&mut self, port_id: &PortId, pubkey: &Pubkey) -> Result<(), PortError> {
        let port_path = PortPath(port_id.clone());
        let module_id = module_id_of_pubkey(pubkey);
        if self.lookup_module_by_port(port_id).is_none() {
            self.state.set(&port_path, module_id.clone());
            self.state
                .update(&AllModulesPath, |all_module_ids: &mut AllModuleIds| {
                    all_module_ids.modules.insert(module_id);
                })
                .map_err(|_err| PortError::ImplementationSpecific)?;

            Ok(())
        } else {
            Err(PortError::ImplementationSpecific)
        }
    }

    pub(super) fn release_port(
        &mut self,
        port_id: &PortId,
        pubkey: &Pubkey,
    ) -> Result<(), PortError> {
        let port_path = PortPath(port_id.clone());
        let module_id = module_id_of_pubkey(pubkey);
        match self.lookup_module_by_port(port_id) {
            Some(curr_module_id) => {
                if module_id == curr_module_id {
                    self.state.remove(&port_path);
                    self.state
                        .update(&AllModulesPath, |all_module_ids: &mut AllModuleIds| {
                            all_module_ids.modules.remove(&module_id);
                        })
                        .map_err(|_err| PortError::ImplementationSpecific)?;

                    Ok(())
                } else {
                    Err(PortError::ImplementationSpecific)
                }
            }
            None => Err(PortError::UnknownPort {
                port_id: port_id.clone(),
            }),
        }
    }
}

#[derive(Debug)]
struct SolanaModule {
    program_id: Pubkey,
}

impl Module for SolanaModule {
    fn on_chan_open_init_validate(
        &self,
        order: Order,
        connection_hops: &[ConnectionId],
        port_id: &PortId,
        channel_id: &ChannelId,
        counterparty: &Counterparty,
        version: &Version,
    ) -> Result<Version, ChannelError> {
        let ibc_module_instruction =
            IbcModuleInstruction::OnChanOpenInitValidate(OnChanOpenInitValidate {
                order,
                connection_hops: connection_hops.to_vec(),
                port_id: port_id.clone(),
                channel_id: channel_id.clone(),
                counterparty: counterparty.clone(),
                version: version.clone(),
            });
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        invoke(&instruction, &[]).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })?;

        let (_, return_data) = get_return_data().ok_or(ChannelError::Other {
            description: "Return data missing".to_owned(),
        })?;

        bincode::deserialize(&return_data).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })
    }

    fn on_chan_open_init_execute(
        &mut self,
        order: Order,
        connection_hops: &[ConnectionId],
        port_id: &PortId,
        channel_id: &ChannelId,
        counterparty: &Counterparty,
        version: &Version,
    ) -> Result<(ModuleExtras, Version), ChannelError> {
        let ibc_module_instruction =
            IbcModuleInstruction::OnChanOpenInitExecute(OnChanOpenInitExecute {
                order,
                connection_hops: connection_hops.to_vec(),
                port_id: port_id.clone(),
                channel_id: channel_id.clone(),
                counterparty: counterparty.clone(),
                version: version.clone(),
            });
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        invoke(&instruction, &[]).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })?;

        let (_, return_data) = get_return_data().ok_or(ChannelError::Other {
            description: "Return data missing".to_owned(),
        })?;

        bincode::deserialize(&return_data).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })
    }

    fn on_chan_open_try_validate(
        &self,
        order: Order,
        connection_hops: &[ConnectionId],
        port_id: &PortId,
        channel_id: &ChannelId,
        counterparty: &Counterparty,
        counterparty_version: &Version,
    ) -> Result<Version, ChannelError> {
        let ibc_module_instruction =
            IbcModuleInstruction::OnChanOpenTryValidate(OnChanOpenTryValidate {
                order,
                connection_hops: connection_hops.to_vec(),
                port_id: port_id.clone(),
                channel_id: channel_id.clone(),
                counterparty: counterparty.clone(),
                counterparty_version: counterparty_version.clone(),
            });
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        invoke(&instruction, &[]).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })?;

        let (_, return_data) = get_return_data().ok_or(ChannelError::Other {
            description: "Return data missing".to_owned(),
        })?;

        bincode::deserialize(&return_data).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })
    }

    fn on_chan_open_try_execute(
        &mut self,
        order: Order,
        connection_hops: &[ConnectionId],
        port_id: &PortId,
        channel_id: &ChannelId,
        counterparty: &Counterparty,
        counterparty_version: &Version,
    ) -> Result<(ModuleExtras, Version), ChannelError> {
        let ibc_module_instruction =
            IbcModuleInstruction::OnChanOpenTryExecute(OnChanOpenTryExecute {
                order,
                connection_hops: connection_hops.to_vec(),
                port_id: port_id.clone(),
                channel_id: channel_id.clone(),
                counterparty: counterparty.clone(),
                counterparty_version: counterparty_version.clone(),
            });
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        invoke(&instruction, &[]).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })?;

        let (_, return_data) = get_return_data().ok_or(ChannelError::Other {
            description: "Return data missing".to_owned(),
        })?;

        bincode::deserialize(&return_data).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })
    }

    fn on_chan_open_ack_validate(
        &self,
        port_id: &PortId,
        channel_id: &ChannelId,
        counterparty_version: &Version,
    ) -> Result<(), ChannelError> {
        let ibc_module_instruction =
            IbcModuleInstruction::OnChanOpenAckValidate(OnChanOpenAckValidate {
                port_id: port_id.clone(),
                channel_id: channel_id.clone(),
                counterparty_version: counterparty_version.clone(),
            });
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        invoke(&instruction, &[]).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })?;

        Ok(())
    }

    fn on_chan_open_ack_execute(
        &mut self,
        port_id: &PortId,
        channel_id: &ChannelId,
        counterparty_version: &Version,
    ) -> Result<ModuleExtras, ChannelError> {
        let ibc_module_instruction =
            IbcModuleInstruction::OnChanOpenAckExecute(OnChanOpenAckExecute {
                port_id: port_id.clone(),
                channel_id: channel_id.clone(),
                counterparty_version: counterparty_version.clone(),
            });
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        invoke(&instruction, &[]).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })?;

        let (_, return_data) = get_return_data().ok_or(ChannelError::Other {
            description: "Return data missing".to_owned(),
        })?;

        bincode::deserialize(&return_data).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })
    }

    fn on_chan_open_confirm_validate(
        &self,
        port_id: &PortId,
        channel_id: &ChannelId,
    ) -> Result<(), ChannelError> {
        let ibc_module_instruction =
            IbcModuleInstruction::OnChanOpenConfirmValidate(OnChanOpenConfirmValidate {
                port_id: port_id.clone(),
                channel_id: channel_id.clone(),
            });
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        invoke(&instruction, &[]).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })?;

        Ok(())
    }

    fn on_chan_open_confirm_execute(
        &mut self,
        port_id: &PortId,
        channel_id: &ChannelId,
    ) -> Result<ModuleExtras, ChannelError> {
        let ibc_module_instruction =
            IbcModuleInstruction::OnChanOpenConfirmExecute(OnChanOpenConfirmExecute {
                port_id: port_id.clone(),
                channel_id: channel_id.clone(),
            });
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        invoke(&instruction, &[]).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })?;

        let (_, return_data) = get_return_data().ok_or(ChannelError::Other {
            description: "Return data missing".to_owned(),
        })?;

        bincode::deserialize(&return_data).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })
    }

    fn on_chan_close_init_validate(
        &self,
        port_id: &PortId,
        channel_id: &ChannelId,
    ) -> Result<(), ChannelError> {
        let ibc_module_instruction =
            IbcModuleInstruction::OnChanCloseInitValidate(OnChanCloseInitValidate {
                port_id: port_id.clone(),
                channel_id: channel_id.clone(),
            });
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        invoke(&instruction, &[]).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })?;

        Ok(())
    }

    fn on_chan_close_init_execute(
        &mut self,
        port_id: &PortId,
        channel_id: &ChannelId,
    ) -> Result<ModuleExtras, ChannelError> {
        let ibc_module_instruction =
            IbcModuleInstruction::OnChanCloseInitExecute(OnChanCloseInitExecute {
                port_id: port_id.clone(),
                channel_id: channel_id.clone(),
            });
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        invoke(&instruction, &[]).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })?;

        let (_, return_data) = get_return_data().ok_or(ChannelError::Other {
            description: "Return data missing".to_owned(),
        })?;

        bincode::deserialize(&return_data).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })
    }

    fn on_chan_close_confirm_validate(
        &self,
        port_id: &PortId,
        channel_id: &ChannelId,
    ) -> Result<(), ChannelError> {
        let ibc_module_instruction =
            IbcModuleInstruction::OnChanCloseConfirmValidate(OnChanCloseConfirmValidate {
                port_id: port_id.clone(),
                channel_id: channel_id.clone(),
            });
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        invoke(&instruction, &[]).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })?;

        Ok(())
    }

    fn on_chan_close_confirm_execute(
        &mut self,
        port_id: &PortId,
        channel_id: &ChannelId,
    ) -> Result<ModuleExtras, ChannelError> {
        let ibc_module_instruction =
            IbcModuleInstruction::OnChanCloseConfirmExecute(OnChanCloseConfirmExecute {
                port_id: port_id.clone(),
                channel_id: channel_id.clone(),
            });
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        invoke(&instruction, &[]).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })?;

        let (_, return_data) = get_return_data().ok_or(ChannelError::Other {
            description: "Return data missing".to_owned(),
        })?;

        bincode::deserialize(&return_data).map_err(|err| ChannelError::Other {
            description: err.to_string(),
        })
    }

    fn on_recv_packet_execute(
        &mut self,
        packet: &Packet,
        relayer: &Signer,
    ) -> (ModuleExtras, Acknowledgement) {
        let ibc_module_instruction =
            IbcModuleInstruction::OnRecvPacketExecute(OnRecvPacketExecute {
                packet: packet.clone(),
                relayer: relayer.clone(),
            });
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        // TODO: Check if `.unwrap` makes sense
        invoke(&instruction, &[]).unwrap();

        let (_, return_data) = get_return_data().expect("Return data missing");

        bincode::deserialize(&return_data).unwrap()
    }

    fn on_acknowledgement_packet_validate(
        &self,
        packet: &Packet,
        acknowledgement: &Acknowledgement,
        relayer: &Signer,
    ) -> Result<(), PacketError> {
        let ibc_module_instruction = IbcModuleInstruction::OnAcknowledgementPacketValidate(
            OnAcknowledgementPacketValidate {
                packet: packet.clone(),
                acknowledgement: acknowledgement.clone(),
                relayer: relayer.clone(),
            },
        );
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        invoke(&instruction, &[]).map_err(|_err| {
            // TODO: Fix the IBC library to include an error message
            PacketError::ImplementationSpecific
        })?;

        Ok(())
    }

    fn on_acknowledgement_packet_execute(
        &mut self,
        packet: &Packet,
        acknowledgement: &Acknowledgement,
        relayer: &Signer,
    ) -> (ModuleExtras, Result<(), PacketError>) {
        let ibc_module_instruction =
            IbcModuleInstruction::OnAcknowledgementPacketExecute(OnAcknowledgementPacketExecute {
                packet: packet.clone(),
                acknowledgement: acknowledgement.clone(),
                relayer: relayer.clone(),
            });
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        let result = invoke(&instruction, &[]).map_err(|_err| {
            // TODO: Fix the IBC library to include an error message
            PacketError::ImplementationSpecific
        });

        // TODO: Fix `ModuleExtras` deserialization upstream
        (ModuleExtras::empty(), result)
    }

    fn on_timeout_packet_validate(
        &self,
        packet: &Packet,
        relayer: &Signer,
    ) -> Result<(), PacketError> {
        let ibc_module_instruction =
            IbcModuleInstruction::OnTimeoutPacketValidate(OnTimeoutPacketValidate {
                packet: packet.clone(),
                relayer: relayer.clone(),
            });
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        invoke(&instruction, &[]).map_err(|_err| {
            // TODO: Fix the IBC library to include an error message
            PacketError::ImplementationSpecific
        })?;

        Ok(())
    }

    fn on_timeout_packet_execute(
        &mut self,
        packet: &Packet,
        relayer: &Signer,
    ) -> (ModuleExtras, Result<(), PacketError>) {
        let ibc_module_instruction =
            IbcModuleInstruction::OnTimeoutPacketExecute(OnTimeoutPacketExecute {
                packet: packet.clone(),
                relayer: relayer.clone(),
            });
        let instruction =
            Instruction::new_with_bincode(self.program_id, &ibc_module_instruction, vec![]);

        let result = invoke(&instruction, &[]).map_err(|_err| {
            // TODO: Fix the IBC library to include an error message
            PacketError::ImplementationSpecific
        });

        // TODO: Fix `ModuleExtras` deserialization upstream
        (ModuleExtras::empty(), result)
    }
}

impl SolanaModule {
    fn into_box(self) -> Box<dyn Module> {
        Box::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_id_of_pubkey_to_string() {
        let pubkey = Pubkey::new_unique();
        assert_eq!(
            module_id_of_pubkey(&pubkey).to_string(),
            "0000000000000001000000000000000000000000000000000000000000000000",
        );
    }
}
