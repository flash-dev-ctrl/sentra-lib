#[cfg(windows)]
use std::ffi::OsString;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserHome {
    pub user: String,
    pub home: PathBuf,
}

fn user_homes_under(base_dir: impl AsRef<Path>) -> Vec<UserHome> {
    let Ok(entries) = fs::read_dir(base_dir.as_ref()) else {
        return Vec::new();
    };

    entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let file_type = entry.file_type().ok()?;
            if !file_type.is_dir() {
                return None;
            }

            Some(UserHome {
                user: entry.file_name().to_string_lossy().to_string(),
                home: entry.path(),
            })
        })
        .collect()
}

pub fn list_local_users() -> Vec<UserHome> {
    #[cfg(windows)]
    {
        return user_homes_under(windows_users_root(std::env::var_os("SystemDrive")));
    }

    #[cfg(target_os = "macos")]
    {
        return user_homes_under("/Users");
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut users = Vec::new();
        if Path::new("/root").exists() {
            users.push(UserHome {
                user: "root".to_string(),
                home: PathBuf::from("/root"),
            });
        }
        users.extend(user_homes_under("/home"));
        return users;
    }

    #[allow(unreachable_code)]
    Vec::new()
}

#[cfg(windows)]
fn windows_users_root(system_drive: Option<OsString>) -> PathBuf {
    let Some(system_drive) = system_drive.filter(|value| !value.is_empty()) else {
        return PathBuf::from(r"C:\Users");
    };

    let mut root = PathBuf::from(system_drive);
    if !root.has_root() {
        root.push(r"\");
    }
    root.join("Users")
}

pub fn list_users() -> Vec<UserHome> {
    let mut users = list_local_users();
    extend_missing_users(&mut users, list_container_users());
    users
}

fn extend_missing_users(users: &mut Vec<UserHome>, extra_users: Vec<UserHome>) {
    for user in extra_users {
        if users.iter().any(|existing| existing.home == user.home) {
            continue;
        }
        users.push(user);
    }
}

#[cfg(target_os = "linux")]
pub fn list_container_users() -> Vec<UserHome> {
    let Ok(host_mnt_ns) = fs::read_link("/proc/1/ns/mnt") else {
        return Vec::new();
    };
    let Ok(host_pid_ns) = fs::read_link("/proc/1/ns/pid") else {
        return Vec::new();
    };
    let Ok(proc_entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };

    let mut container_pids: Vec<(PathBuf, String)> = Vec::new();
    for entry in proc_entries.flatten() {
        let pid = entry.file_name().to_string_lossy().to_string();
        if !pid.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }

        let ns_dir = Path::new("/proc").join(&pid).join("ns");
        let Ok(mnt_ns) = fs::read_link(ns_dir.join("mnt")) else {
            continue;
        };
        let Ok(pid_ns) = fs::read_link(ns_dir.join("pid")) else {
            continue;
        };

        if mnt_ns != host_mnt_ns
            && pid_ns != host_pid_ns
            && !container_pids
                .iter()
                .any(|(existing_mnt_ns, _)| *existing_mnt_ns == mnt_ns)
        {
            container_pids.push((mnt_ns, pid));
        }
    }

    let mut results = Vec::new();
    for (_, pid) in container_pids {
        let proc_root = Path::new("/proc").join(pid).join("root");
        let root_home = proc_root.join("root");
        if root_home.exists() {
            results.push(UserHome {
                user: "root".to_string(),
                home: root_home,
            });
        }
        results.extend(user_homes_under(proc_root.join("home")));
    }
    results
}

#[cfg(not(target_os = "linux"))]
pub fn list_container_users() -> Vec<UserHome> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn user_homes_under_returns_direct_child_directories() {
        let temp = tempfile::tempdir().expect("create temp dir");
        fs::create_dir(temp.path().join("alice")).expect("create alice dir");
        fs::create_dir(temp.path().join("bob")).expect("create bob dir");
        fs::write(temp.path().join("not-a-user"), "ignored").expect("create file");

        let mut users = user_homes_under(temp.path());
        users.sort_by(|left, right| left.user.cmp(&right.user));

        assert_eq!(
            users,
            vec![
                UserHome {
                    user: "alice".to_string(),
                    home: temp.path().join("alice"),
                },
                UserHome {
                    user: "bob".to_string(),
                    home: temp.path().join("bob"),
                },
            ]
        );
    }

    #[test]
    fn user_homes_under_returns_empty_for_missing_directory() {
        let temp = tempfile::tempdir().expect("create temp dir");

        assert!(user_homes_under(temp.path().join("missing")).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn windows_users_root_uses_system_drive() {
        assert_eq!(
            windows_users_root(Some(OsString::from("D:"))),
            PathBuf::from(r"D:\Users"),
        );
        assert_eq!(
            windows_users_root(Some(OsString::from(r"D:\"))),
            PathBuf::from(r"D:\Users"),
        );
        assert_eq!(windows_users_root(None), PathBuf::from(r"C:\Users"));
        assert_eq!(
            windows_users_root(Some(OsString::from(""))),
            PathBuf::from(r"C:\Users"),
        );
    }

    #[test]
    fn extend_missing_users_deduplicates_by_home() {
        let home = PathBuf::from("/home/alice");
        let mut users = vec![UserHome {
            user: "alice".to_string(),
            home: home.clone(),
        }];

        extend_missing_users(
            &mut users,
            vec![
                UserHome {
                    user: "alice-container".to_string(),
                    home,
                },
                UserHome {
                    user: "bob".to_string(),
                    home: PathBuf::from("/home/bob"),
                },
            ],
        );

        assert_eq!(users.len(), 2);
        assert_eq!(users[1].user, "bob");
    }
}
