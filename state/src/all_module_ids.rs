use {
    anyhow::anyhow,
    core::str::FromStr,
    eclipse_ibc_proto::eclipse::ibc::client::v1::AllModuleIds as RawAllModuleIds,
    ibc::core::ics26_routing::context::{InvalidModuleId, ModuleId},
    known_proto::KnownProtoWithFrom,
    std::collections::HashSet,
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
    type Error = anyhow::Error;

    fn try_from(RawAllModuleIds { modules }: RawAllModuleIds) -> Result<Self, Self::Error> {
        Ok(Self {
            modules: modules
                .iter()
                .map(|raw_module| {
                    ModuleId::from_str(raw_module)
                        .map_err(|InvalidModuleId| anyhow!("Invalid module ID: {raw_module}"))
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

impl KnownProtoWithFrom for AllModuleIds {
    type RawWithFrom = RawAllModuleIds;
}
