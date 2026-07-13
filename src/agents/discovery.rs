use std::fs;
use std::path::Path;

use crate::agents::{
    Agent,
    entries::{AgentEntry, SystemAgentPath, builtin_agent_entries},
};

fn titleize_agent_name(name: &str) -> String {
    name.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            if part.len() <= 3 {
                part.to_ascii_uppercase()
            } else {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn get_agent_title(agent_name: &str) -> String {
    builtin_agent_entries()
        .into_iter()
        .find(|entry| entry.name == agent_name)
        .and_then(|entry| entry.title.map(str::to_string))
        .unwrap_or_else(|| titleize_agent_name(agent_name))
}

pub fn discover_agents(user_home: impl AsRef<Path>) -> Vec<Agent> {
    let user_home = user_home.as_ref();
    let mut results = Vec::new();
    results.extend(crate::agents::codex::discover_agents(user_home));
    results.extend(crate::agents::claude_cli::discover_agents(user_home));
    results.extend(crate::agents::claude_app::discover_agents(user_home));
    results.extend(crate::agents::hermes::discover_agents(user_home));
    results.extend(crate::agents::openclaw::discover_agents(user_home));
    results.extend(crate::agents::opencode::discover_agents(user_home));
    results.extend(crate::agents::pi::discover_agents(user_home));
    results.extend(crate::agents::sentra::discover_agents(user_home));
    results.extend(crate::agents::general::discover_agents(user_home));
    results
}

pub(crate) fn discover_entry_agents(user_home: &Path, entries: &[AgentEntry]) -> Vec<Agent> {
    let mut results = Vec::new();
    for entry in entries {
        for segments in entry.homes {
            let mut home = user_home.to_path_buf();
            for segment in segments.iter() {
                home.push(segment);
            }
            if fs::metadata(&home)
                .map(|meta| meta.is_dir())
                .unwrap_or(false)
            {
                results.push(Agent::new(entry, home));
            }
        }
    }
    results
}

pub(crate) fn discover_system_agents(system_paths: &[SystemAgentPath]) -> Vec<Agent> {
    let mut results = Vec::new();
    for item in system_paths {
        let home = Path::new(item.system_path);
        if fs::metadata(home)
            .map(|meta| meta.is_dir())
            .unwrap_or(false)
            && !results
                .iter()
                .any(|agent: &Agent| agent.name() == item.entry.name && agent.home() == home)
        {
            results.push(Agent::new(item.entry, home));
        }
    }
    results
}
