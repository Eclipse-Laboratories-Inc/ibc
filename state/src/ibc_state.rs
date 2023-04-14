use {
    crate::IbcStore,
    core::{
        fmt::{self, Debug},
        mem,
    },
    ibc_proto::ibc::core::commitment::v1::MerkleRoot,
    ics23::ExistenceProof,
    jmt::Sha256Jmt,
    known_proto::KnownProto,
    sha2::Sha256,
    solana_sdk::clock::Slot,
    std::collections::BTreeMap,
};

pub struct IbcState<'a> {
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
    pub fn new(state_store: &'a IbcStore, slot: Slot) -> Self {
        Self {
            state_jmt: Sha256Jmt::new(state_store),
            pending_changes: BTreeMap::new(),
            // Slots map directly to versions
            version: slot,
        }
    }

    #[allow(unused)]
    pub fn get_root(&self) -> anyhow::Result<MerkleRoot> {
        let jmt::RootHash(root_hash) = self.state_jmt.get_root_hash(self.version)?;
        Ok(MerkleRoot {
            hash: root_hash.to_vec(),
        })
    }

    pub fn get<T>(&self, key: &str) -> anyhow::Result<Option<T>>
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

    pub fn get_raw<T>(&self, key: &str) -> anyhow::Result<Option<T>>
    where
        T: Default + prost::Message,
    {
        let key_hash = jmt::KeyHash::with::<Sha256>(key);
        if let Some(owned_value) = self.pending_changes.get(&key_hash) {
            return Ok(owned_value
                .as_ref()
                .map(|value| T::decode(&**value))
                .transpose()?);
        }

        Ok(self
            .state_jmt
            .get(key_hash, self.version)?
            .map(|owned_value| T::decode(&*owned_value))
            .transpose()?)
    }

    #[allow(unused)]
    pub fn get_proof(&self, key: &str) -> anyhow::Result<ExistenceProof> {
        self.state_jmt
            .get_with_ics23_proof(key.as_bytes().to_vec(), self.version)
    }

    pub fn set<T>(&mut self, key: &str, value: T)
    where
        T: KnownProto,
    {
        let key_hash = jmt::KeyHash::with::<Sha256>(key);
        self.pending_changes
            .insert(key_hash, Some(T::encode(value)));
    }

    pub fn update<T, F>(&mut self, key: &str, f: F) -> anyhow::Result<()>
    where
        T: Default + KnownProto,
        F: FnOnce(&mut T),
    {
        let mut value: T = self.get(key)?.unwrap_or_default();
        f(&mut value);
        self.set(key, value);
        Ok(())
    }

    pub fn remove(&mut self, key: &str) {
        let key_hash = jmt::KeyHash::with::<Sha256>(key);
        self.pending_changes.insert(key_hash, None);
    }

    pub fn commit(&mut self) -> anyhow::Result<()> {
        let pending_changes = mem::take(&mut self.pending_changes);
        self.state_jmt
            .put_value_set(pending_changes, self.version)?;
        Ok(())
    }
}