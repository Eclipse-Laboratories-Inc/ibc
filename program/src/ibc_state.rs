use {
    anyhow::{anyhow, bail},
    core::{
        fmt::{self, Debug},
        mem,
    },
    ibc_proto::ibc::core::commitment::v1::MerkleRoot,
    ics23::ExistenceProof,
    jmt::{
        storage::{TreeReader, TreeWriter},
        Sha256Jmt,
    },
    known_proto::KnownProto,
    serde::{Deserialize, Serialize},
    sha2::Sha256,
    solana_program_runtime::{ic_msg, invoke_context::InvokeContext},
    solana_sdk::{
        clock::Slot, instruction::InstructionError, transaction_context::BorrowedAccount,
    },
    std::{
        collections::{BTreeMap, HashMap},
        sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
    },
};

#[derive(Debug, Default, Deserialize, Serialize)]
struct InnerStore {
    #[serde(with = "store_nodes")]
    nodes: BTreeMap<jmt::storage::NodeKey, jmt::storage::Node>,
    value_history: HashMap<jmt::KeyHash, BTreeMap<jmt::Version, Option<jmt::OwnedValue>>>,
}

mod store_nodes {
    use {
        serde::{
            de::{self, MapAccess, Visitor},
            ser::{self, SerializeMap},
            Deserializer, Serializer,
        },
        std::{collections::BTreeMap, fmt},
    };

    type Value = BTreeMap<jmt::storage::NodeKey, jmt::storage::Node>;

    pub(super) fn serialize<S>(value: &Value, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(value.len()))?;
        for (node_key, node) in value {
            let node_key = node_key
                .encode()
                .map_err(|err| ser::Error::custom(format!("failed to encode node key: {err:?}")))?;
            let node = node
                .encode()
                .map_err(|err| ser::Error::custom(format!("failed to encode node: {err:?}")))?;
            map.serialize_entry(&node_key, &node)?;
        }
        map.end()
    }

    struct ValueVisitor;

    impl<'de> Visitor<'de> for ValueVisitor {
        type Value = Value;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a map with Jellyfish Merkle Tree node keys and nodes")
        }

        fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            let mut map = BTreeMap::new();
            while let Some((node_key, node)) = access.next_entry::<&[u8], &[u8]>()? {
                let node_key = jmt::storage::NodeKey::decode(node_key).map_err(|err| {
                    de::Error::invalid_value(
                        de::Unexpected::Bytes(node_key),
                        &&*format!("failed to decode node key: {err}"),
                    )
                })?;
                let node = jmt::storage::Node::decode(node).map_err(|err| {
                    de::Error::invalid_value(
                        de::Unexpected::Bytes(node),
                        &&*format!("failed to decode node: {err}"),
                    )
                })?;
                map.insert(node_key, node);
            }
            Ok(map)
        }
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ValueVisitor)
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(transparent)]
pub(super) struct IbcStore {
    inner: RwLock<InnerStore>,
}

impl IbcStore {
    fn read(&self) -> anyhow::Result<RwLockReadGuard<'_, InnerStore>> {
        self.inner.read().map_err(|err| anyhow!("{err}"))
    }

    fn write(&self) -> anyhow::Result<RwLockWriteGuard<'_, InnerStore>> {
        self.inner.write().map_err(|err| anyhow!("{err}"))
    }
}

impl TreeReader for IbcStore {
    fn get_node_option(
        &self,
        node_key: &jmt::storage::NodeKey,
    ) -> anyhow::Result<Option<jmt::storage::Node>> {
        Ok(self.read()?.nodes.get(node_key).cloned())
    }

    fn get_value_option(
        &self,
        max_version: jmt::Version,
        key_hash: jmt::KeyHash,
    ) -> anyhow::Result<Option<jmt::OwnedValue>> {
        Ok(self
            .read()?
            .value_history
            .get(&key_hash)
            .and_then(|version_history| {
                version_history
                    .range(..=max_version)
                    .next_back()
                    .and_then(|(_, value)| value.clone())
            }))
    }

    fn get_rightmost_leaf(
        &self,
    ) -> anyhow::Result<Option<(jmt::storage::NodeKey, jmt::storage::LeafNode)>> {
        Ok(self
            .read()?
            .nodes
            .iter()
            .rev()
            .find_map(|(node_key, node)| match node {
                jmt::storage::Node::Leaf(leaf_node) => Some((node_key.clone(), leaf_node.clone())),
                _ => None,
            }))
    }
}

impl TreeWriter for IbcStore {
    fn write_node_batch(&self, node_batch: &jmt::storage::NodeBatch) -> anyhow::Result<()> {
        let mut inner = self.write()?;
        for (node_key, node) in node_batch.nodes() {
            inner.nodes.insert(node_key.clone(), node.clone());
        }

        for ((version, key_hash), value) in node_batch.values() {
            let versions = inner.value_history.entry(*key_hash).or_default();
            if let Some((&last_version, _)) = versions.last_key_value() {
                if *version < last_version {
                    bail!(
                        "value must be latest version; last version: {}, new version: {}",
                        last_version,
                        *version,
                    );
                }
            }
            versions.insert(*version, value.clone());
        }

        Ok(())
    }
}

pub(super) struct IbcState<'a> {
    state_jmt: Sha256Jmt<'a, IbcStore>,
    pending_changes: BTreeMap<jmt::KeyHash, Option<Vec<u8>>>,
    version: jmt::Version,
}

impl Debug for IbcState<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("IbcState")
            .field("state_jmt", &"<opaque>")
            .field("pending_changes", &self.pending_changes)
            .field("version", &self.version)
            .finish()
    }
}

impl<'a> IbcState<'a> {
    #[must_use]
    pub(super) fn new(state_store: &'a IbcStore, slot: Slot) -> Self {
        Self {
            state_jmt: Sha256Jmt::new(state_store),
            pending_changes: BTreeMap::new(),
            // Slots map directly to versions
            version: slot,
        }
    }

    #[allow(unused)]
    pub(super) fn get_root(&self) -> anyhow::Result<MerkleRoot> {
        let jmt::RootHash(root_hash) = self.state_jmt.get_root_hash(self.version)?;
        Ok(MerkleRoot {
            hash: root_hash.to_vec(),
        })
    }

    pub(super) fn get<T>(&self, key: &str) -> anyhow::Result<Option<T>>
    where
        T: KnownProto,
    {
        let key_hash = jmt::KeyHash::with::<Sha256>(key);
        if let Some(owned_value) = self.pending_changes.get(&key_hash) {
            return owned_value
                .as_ref()
                .map(|value| T::decode(&**value))
                .transpose();
        }

        self.state_jmt
            .get(key_hash, self.version)?
            .map(|owned_value| T::decode(&*owned_value))
            .transpose()
    }

    #[allow(unused)]
    pub(super) fn get_proof(&self, key: &str) -> anyhow::Result<ExistenceProof> {
        self.state_jmt
            .get_with_ics23_proof(key.as_bytes().to_vec(), self.version)
    }

    pub(super) fn set<T>(&mut self, key: &str, value: T)
    where
        T: KnownProto,
    {
        let key_hash = jmt::KeyHash::with::<Sha256>(key);
        self.pending_changes
            .insert(key_hash, Some(T::encode(value)));
    }

    pub(super) fn update<T, F>(&mut self, key: &str, f: F) -> anyhow::Result<()>
    where
        T: Default + KnownProto,
        F: FnOnce(&mut T),
    {
        let mut value: T = self.get(key)?.unwrap_or_default();
        f(&mut value);
        self.set(key, value);
        Ok(())
    }

    pub(super) fn remove(&mut self, key: &str) {
        let key_hash = jmt::KeyHash::with::<Sha256>(key);
        self.pending_changes.insert(key_hash, None);
    }

    pub(super) fn commit(&mut self) -> anyhow::Result<()> {
        let pending_changes = mem::take(&mut self.pending_changes);
        self.state_jmt
            .put_value_set(pending_changes, self.version)?;
        Ok(())
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct IbcMetadata {
    pub client_id_counter: u64,
    pub connection_id_counter: u64,
    pub channel_id_counter: u64,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct IbcAccountData {
    pub store: IbcStore,
    pub metadata: IbcMetadata,
}

impl IbcAccountData {
    pub(super) fn read_from_account(
        account: &BorrowedAccount<'_>,
        invoke_context: &InvokeContext,
    ) -> Result<Self, InstructionError> {
        let account_data = account.get_data();
        bincode::deserialize::<Self>(account_data).map_err(|err| {
            ic_msg!(
                invoke_context,
                "failed to deserialize IBC account data: {:?}",
                err,
            );
            InstructionError::InvalidAccountData
        })
    }

    pub(super) fn write_to_account(
        &self,
        account: &mut BorrowedAccount<'_>,
        invoke_context: &InvokeContext,
    ) -> Result<(), InstructionError> {
        let account_data = bincode::serialize(&self).map_err(|err| {
            ic_msg!(
                invoke_context,
                "failed to serialize new IBC account data: {:?}",
                err,
            );
            InstructionError::InvalidAccountData
        })?;
        account.set_data(account_data)?;
        Ok(())
    }
}
