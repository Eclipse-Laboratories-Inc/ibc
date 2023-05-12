use {
    core::convert::Infallible, eclipse_ibc_known_proto::KnownProtoWithFrom,
    eclipse_ibc_proto::eclipse::ibc::client::v1::AllModuleIds as RawAllModuleIds,
    ibc::core::router::ModuleId, std::collections::HashSet,
};

#[derive(Clone, Debug, Default)]
pub struct AllModuleIds {
    pub modules: HashSet<ModuleId>,
}

impl From<AllModuleIds> for RawAllModuleIds {
    fn from(AllModuleIds { modules }: AllModuleIds) -> Self {
        Self {
            modules: modules.iter().map(ModuleId::to_string).collect(),
        }
    }
}

impl TryFrom<RawAllModuleIds> for AllModuleIds {
    type Error = Infallible;

    fn try_from(RawAllModuleIds { modules }: RawAllModuleIds) -> Result<Self, Self::Error> {
        Ok(Self {
            modules: modules.into_iter().map(ModuleId::new).collect(),
        })
    }
}

impl KnownProtoWithFrom for AllModuleIds {
    type RawWithFrom = RawAllModuleIds;
}
