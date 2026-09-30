use neebles_backend::domestic_runtime_authority::{
    current_boss_runtime_manifest, resolve_boss_executable,
};

use serde_json::json;

use std::collections::BTreeMap;

fn run() -> Result<(), String> {
    let worlds = [
        "boss.curl",
        "boss.env",
        "boss.git",
        "boss.pkexec",
        "boss.qdbus6",
        "boss.setpriv",
        "boss.systemctl",
        "boss.systemd-run",
        "boss.tar",
    ];

    let manifest = current_boss_runtime_manifest()?;

    let mut resolved = BTreeMap::<String, String>::new();

    for world in worlds {
        let target = resolve_boss_executable(world)?;

        resolved.insert(world.to_string(), target.to_string_lossy().to_string());
    }

    let output = json!({
        "schema": "1",
        "name": "neebles-boss-runtime-self-certification",
        "manifest": manifest,
        "count": resolved.len(),
        "resolved": resolved
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&output).map_err(|error| {
            format!("could not serialize Boss runtime self certification: {error}")
        })?
    );

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("FATAL :: {error}");

        std::process::exit(1);
    }
}
