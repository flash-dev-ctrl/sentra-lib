use std::path::Path;

use crate::interfaces::{AssetType, ErasedAsset};

mod meta;
mod skill;

pub(crate) fn discover_agents(user_home: impl AsRef<Path>) -> Vec<crate::agents::Agent> {
    let mut results = crate::agents::discovery::discover_entry_agents(
        user_home.as_ref(),
        crate::agents::entries::GENERAL_AGENT_ENTRIES,
    );
    for agent in
        crate::agents::discovery::discover_system_agents(crate::agents::entries::SYSTEM_AGENT_PATHS)
    {
        if !results
            .iter()
            .any(|item| item.name() == agent.name() && item.home() == agent.home())
        {
            results.push(agent);
        }
    }
    results
}

pub(crate) fn asset_for_type(
    agent_name: &str,
    agent_home: &std::path::Path,
    asset_type: AssetType,
) -> Vec<Box<dyn ErasedAsset>> {
    match asset_type {
        AssetType::Meta => vec![Box::new(meta::MetaAsset::new(agent_name, agent_home))],
        AssetType::Skill => vec![Box::new(skill::SkillAsset::new(agent_name, agent_home))],
        AssetType::Plugin => Vec::new(),
        _ => Vec::new(),
    }
}
