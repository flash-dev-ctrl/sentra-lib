use std::path::Path;

use crate::SentraResult;
use crate::agents::codebuddy::{cron, surface};
use crate::agents::object::{AssetCore, impl_erased_asset};
use crate::interfaces::{Asset, AssetType, CronData};

#[derive(Debug, Clone)]
pub(super) struct CronAsset {
    core: AssetCore,
}

impl CronAsset {
    pub(super) fn new(
        agent_name: impl Into<String>,
        agent_home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            core: AssetCore::new(agent_name, agent_home),
        }
    }
}

impl_erased_asset!(CronAsset, AssetType::Cron, Vec<CronData>);

impl Asset<Vec<CronData>> for CronAsset {
    fn get_data(&self) -> SentraResult<Vec<CronData>> {
        cron_data(self.core.agent_name(), self.core.agent_home())
    }
}

fn cron_data(agent_name: &str, agent_home: &Path) -> SentraResult<Vec<CronData>> {
    let mut out = Vec::new();
    for app_root in surface::ide_data_roots(agent_name, agent_home) {
        out.extend(cron::automation_database_crons(
            &app_root.join("automations").join("automations.db"),
        )?);
    }
    Ok(dedup_crons(out))
}

fn dedup_crons(items: Vec<CronData>) -> Vec<CronData> {
    let mut out = Vec::new();
    for item in items {
        if out.iter().any(|seen: &CronData| seen.id == item.id) {
            continue;
        }
        out.push(item);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_codebuddy_ide_automation_database() {
        let dir = tempfile::tempdir().unwrap();
        let agent_home = dir
            .path()
            .join("AppData")
            .join("Roaming")
            .join("CodeBuddy CN");
        let database_dir = agent_home.join("automations");
        std::fs::create_dir_all(&database_dir).unwrap();
        let database = rusqlite::Connection::open(database_dir.join("automations.db")).unwrap();
        database
            .execute(
                "CREATE TABLE automations (
                    id text PRIMARY KEY,
                    name text,
                    prompt text,
                    status text,
                    schedule_type text,
                    next_run_at text,
                    last_run_at text,
                    cwds text,
                    rrule text,
                    scheduled_at text,
                    created_at,
                    updated_at,
                    deleted_at
                )",
                [],
            )
            .unwrap();
        database
            .execute(
                "INSERT INTO automations
                 (id, name, prompt, status, schedule_type, next_run_at, last_run_at, cwds, rrule,
                  scheduled_at, created_at, updated_at, deleted_at)
                 VALUES
                 ('ide-task', 'IDE task', 'run from IDE', 'enabled', 'once',
                  NULL, NULL, '[\"C:/code\"]', NULL, '2026-07-23T10:00:00Z',
                  NULL, NULL, NULL)",
                [],
            )
            .unwrap();

        let tasks = cron_data("codebuddy-cn-ide", &agent_home).unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "ide-task");
        assert_eq!(tasks[0].cron_type, Some(crate::interfaces::CronType::At));
        assert_eq!(tasks[0].schedule.as_deref(), Some("2026-07-23T10:00:00Z"));
    }
}
