use std::env;

pub fn desktop_identity() -> Result<(libc::uid_t, libc::gid_t), String> {
    let uid = env::var("NEEBLES_DESKTOP_UID");
    let gid = env::var("NEEBLES_DESKTOP_GID");

    match (uid, gid) {
        (Ok(uid), Ok(gid)) => {
            let uid = uid
                .parse::<u32>()
                .map_err(|error| format!("invalid NEEBLES_DESKTOP_UID: {error}"))?;

            let gid = gid
                .parse::<u32>()
                .map_err(|error| format!("invalid NEEBLES_DESKTOP_GID: {error}"))?;

            Ok((uid as libc::uid_t, gid as libc::gid_t))
        }

        (Err(env::VarError::NotPresent), Err(env::VarError::NotPresent)) => {
            Ok(unsafe { (libc::geteuid(), libc::getegid()) })
        }

        _ => Err(
            "NEEBLES_DESKTOP_UID and NEEBLES_DESKTOP_GID must be configured together".to_string(),
        ),
    }
}
