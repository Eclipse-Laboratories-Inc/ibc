use {
    anyhow::{anyhow, bail},
    core::fmt::Debug,
    jmt::storage::{HasPreimage, TreeReader, TreeWriter},
    serde::{Deserialize, Serialize},
    std::{
        collections::{BTreeMap, HashMap},
        sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
    },
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct InnerStore {
    nodes: BTreeMap<jmt::storage::NodeKey, jmt::storage::Node>,
    value_history: HashMap<jmt::KeyHash, BTreeMap<jmt::Version, Option<jmt::OwnedValue>>>,
    preimages: HashMap<jmt::KeyHash, Vec<u8>>,
    versions: Vec<jmt::Version>,
}

impl InnerStore {
    pub fn latest_version(&self) -> Option<jmt::Version> {
        self.versions.last().copied()
    }

    pub fn find_version(&self, max_version: jmt::Version) -> Option<jmt::Version> {
        let first_version_past = self
            .versions
            .partition_point(|&version| version <= max_version);
        (first_version_past > 0).then(|| self.versions[first_version_past - 1])
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(transparent)]
pub struct IbcStore {
    inner: RwLock<InnerStore>,
}

impl IbcStore {
    pub fn read(&self) -> anyhow::Result<RwLockReadGuard<'_, InnerStore>> {
        self.inner.read().map_err(|err| anyhow!("{err}"))
    }

    fn write(&self) -> anyhow::Result<RwLockWriteGuard<'_, InnerStore>> {
        self.inner.write().map_err(|err| anyhow!("{err}"))
    }

    pub fn find_key_version(
        &self,
        max_version: jmt::Version,
        key_hash: jmt::KeyHash,
    ) -> anyhow::Result<Option<jmt::Version>> {
        Ok(self
            .read()?
            .value_history
            .get(&key_hash)
            .and_then(|version_history| {
                version_history
                    .range(..=max_version)
                    .next_back()
                    .map(|(&version, _)| version)
            }))
    }

    pub fn insert_preimage(&self, key_hash: jmt::KeyHash, key: Vec<u8>) -> anyhow::Result<()> {
        self.write()?.preimages.insert(key_hash, key);
        Ok(())
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

        for (&(version, key_hash), value) in node_batch.values() {
            let versions = inner.value_history.entry(key_hash).or_default();
            if let Some((&last_version, _)) = versions.last_key_value() {
                if version < last_version {
                    bail!(
                        "value must be latest version; last version: {}, new version: {}",
                        last_version,
                        version,
                    );
                }
            }
            versions.insert(version, value.clone());

            if inner
                .latest_version()
                .map_or(true, |latest_version| latest_version < version)
            {
                inner.versions.push(version);
            }
        }

        Ok(())
    }
}

impl HasPreimage for IbcStore {
    fn preimage(&self, key_hash: jmt::KeyHash) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self.read()?.preimages.get(&key_hash).cloned())
    }
}
