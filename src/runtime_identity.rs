use std::env;
use std::fs;
use std::path::Path;

const RUNTIME_ENV_PATH: &str = "/etc/neebles/runtime.env";

fn parse_identity(uid: &str, gid: &str) -> Result<(libc::uid_t, libc::gid_t), String> {
    let uid = uid
        .trim()
        .parse::<u32>()
        .map_err(|error| format!("invalid NEEBLES_DESKTOP_UID: {error}"))?;

    let gid = gid
        .trim()
        .parse::<u32>()
        .map_err(|error| format!("invalid NEEBLES_DESKTOP_GID: {error}"))?;

    Ok((uid as libc::uid_t, gid as libc::gid_t))
}

fn persisted_identity(path: &Path) -> Result<Option<(libc::uid_t, libc::gid_t)>, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,

        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }

        Err(error) => {
            return Err(format!(
                "could not read runtime identity {}: {error}",
                path.display()
            ));
        }
    };

    let mut uid = None;
    let mut gid = None;

    for line in raw.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(value) = line.strip_prefix("NEEBLES_DESKTOP_UID=") {
            uid = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("NEEBLES_DESKTOP_GID=") {
            gid = Some(value.trim().to_string());
        }
    }

    match (uid, gid) {
        (Some(uid), Some(gid)) => parse_identity(&uid, &gid).map(Some),

        (None, None) => Err(format!(
            "runtime identity {} does not define NEEBLES_DESKTOP_UID and NEEBLES_DESKTOP_GID",
            path.display()
        )),

        _ => Err(
            "NEEBLES_DESKTOP_UID and NEEBLES_DESKTOP_GID must be configured together".to_string(),
        ),
    }
}

pub fn desktop_identity() -> Result<(libc::uid_t, libc::gid_t), String> {
    let uid = env::var("NEEBLES_DESKTOP_UID");
    let gid = env::var("NEEBLES_DESKTOP_GID");

    match (uid, gid) {
        (Ok(uid), Ok(gid)) => {
            return parse_identity(&uid, &gid);
        }

        (Err(env::VarError::NotPresent), Err(env::VarError::NotPresent)) => {}

        _ => {
            return Err(
                "NEEBLES_DESKTOP_UID and NEEBLES_DESKTOP_GID must be configured together"
                    .to_string(),
            );
        }
    }

    if let Some(identity) = persisted_identity(Path::new(RUNTIME_ENV_PATH))? {
        return Ok(identity);
    }

    Ok(unsafe { (libc::geteuid(), libc::getegid()) })
}

#[cfg(test)]
mod certification_tests {
    use super::*;

    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_runtime_env(label: &str) -> std::path::PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);

        std::env::temp_dir().join(format!(
            "neebles-runtime-identity-{label}-{}-{id}.env",
            std::process::id()
        ))
    }

    #[test]
    fn certification_runtime_identity_reads_persisted_pair() {
        let path = temp_runtime_env("pair");

        fs::write(
            &path,
            "NEEBLES_DESKTOP_UID=2001\nNEEBLES_DESKTOP_GID=2002\n",
        )
        .unwrap();

        assert_eq!(persisted_identity(&path).unwrap(), Some((2001, 2002)));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn certification_runtime_identity_rejects_partial_pair() {
        let path = temp_runtime_env("partial");

        fs::write(&path, "NEEBLES_DESKTOP_UID=2001\n").unwrap();

        let error = persisted_identity(&path).expect_err("partial identity must fail");

        assert!(error.contains("must be configured together"));

        let _ = fs::remove_file(path);
    }
}
