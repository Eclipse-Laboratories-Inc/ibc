use {
    anyhow::{anyhow, bail},
    core::fmt::Debug,
    jmt::storage::{TreeReader, TreeWriter},
    serde::{Deserialize, Serialize},
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
pub struct IbcStore {
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
