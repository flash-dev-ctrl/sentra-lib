use crate::SentraResult;
use crate::agents::codebuddy::cron;
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
        cron::automation_database_crons(&self.core.agent_home().join("workbuddy.db"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_workbuddy_automation_database() {
        let dir = tempfile::tempdir().unwrap();
        let database_path = dir.path().join("workbuddy.db");
        let database = rusqlite::Connection::open(&database_path).unwrap();
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
                 ('work-task', 'Work task', 'prepare report', 'enabled', 'recurring',
                  NULL, NULL, '[\"C:/work\"]', 'FREQ=DAILY', NULL, NULL, NULL, NULL)",
                [],
            )
            .unwrap();

        let data =
            <CronAsset as Asset<Vec<CronData>>>::get_data(&CronAsset::new("workbuddy", dir.path()))
                .unwrap();

        assert_eq!(data.len(), 1);
        assert_eq!(data[0].id, "work-task");
        assert_eq!(data[0].schedule.as_deref(), Some("FREQ=DAILY"));
    }
}
