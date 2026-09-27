use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::domestic_authority_supply::SuppliedAuthorityRegistry;
use crate::domestic_platform_control::{
    load_platform_controlled_authority_supply, register_platform_controlled_authorities,
};

pub const AUTHORITY_SUPPLY_PROCESS_ARGUMENT: &str = "--authority-supply";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoritySupplyProcessInput {
    supply_path: PathBuf,
}

impl AuthoritySupplyProcessInput {
    pub fn supply_path(&self) -> &Path {
        &self.supply_path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoritySupplyProcessState {
    supply_path: PathBuf,
    registry: SuppliedAuthorityRegistry,
}

impl AuthoritySupplyProcessState {
    pub fn supply_path(&self) -> &Path {
        &self.supply_path
    }

    pub fn registry(&self) -> &SuppliedAuthorityRegistry {
        &self.registry
    }
}

static PROCESS_AUTHORITY_SUPPLY: OnceLock<AuthoritySupplyProcessState> = OnceLock::new();

pub fn split_authority_supply_process_argument(
    arguments: Vec<String>,
) -> Result<(Option<AuthoritySupplyProcessInput>, Vec<String>), String> {
    let positions = arguments
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            if value == AUTHORITY_SUPPLY_PROCESS_ARGUMENT {
                Some(index)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if positions.is_empty() {
        return Ok((None, arguments));
    }

    if positions.len() != 1 {
        return Err(format!(
            "{} must be supplied at most once",
            AUTHORITY_SUPPLY_PROCESS_ARGUMENT
        ));
    }

    if positions[0] != 0 {
        return Err(format!(
            "{} is a global process argument and must appear before the Boss command",
            AUTHORITY_SUPPLY_PROCESS_ARGUMENT
        ));
    }

    let raw_path = arguments
        .get(1)
        .ok_or_else(|| format!("{} requires a path", AUTHORITY_SUPPLY_PROCESS_ARGUMENT))?;

    if raw_path.trim().is_empty() {
        return Err(format!(
            "{} path cannot be empty",
            AUTHORITY_SUPPLY_PROCESS_ARGUMENT
        ));
    }

    let supply_path = PathBuf::from(raw_path);

    if !supply_path.is_absolute() {
        return Err(format!(
            "{} path must be absolute: {}",
            AUTHORITY_SUPPLY_PROCESS_ARGUMENT,
            supply_path.display()
        ));
    }

    let remaining = arguments.into_iter().skip(2).collect::<Vec<_>>();

    Ok((Some(AuthoritySupplyProcessInput { supply_path }), remaining))
}

pub fn load_authority_supply_process_state(
    input: &AuthoritySupplyProcessInput,
) -> Result<AuthoritySupplyProcessState, String> {
    let supplied = load_platform_controlled_authority_supply(input.supply_path())?;
    let registry = register_platform_controlled_authorities(&supplied)?;

    Ok(AuthoritySupplyProcessState {
        supply_path: supplied.supply_path().to_path_buf(),
        registry,
    })
}

pub fn initialize_authority_supply_process(
    input: AuthoritySupplyProcessInput,
) -> Result<(), String> {
    let state = load_authority_supply_process_state(&input)?;

    PROCESS_AUTHORITY_SUPPLY
        .set(state)
        .map_err(|_| "AuthoritySupply process input was initialized more than once".to_string())
}

pub fn authority_supply_process_state() -> Result<&'static AuthoritySupplyProcessState, String> {
    PROCESS_AUTHORITY_SUPPLY
        .get()
        .ok_or_else(|| "AuthoritySupply was not supplied to this Boss process".to_string())
}

pub fn process_supplied_authority_registry() -> Result<&'static SuppliedAuthorityRegistry, String> {
    Ok(authority_supply_process_state()?.registry())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fixture clock must work")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "neebles-authority-process-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    #[test]
    fn absent_global_argument_preserves_cli_arguments() {
        let arguments = vec!["socket".to_string(), "serve".to_string()];

        let (input, remaining) = split_authority_supply_process_argument(arguments.clone())
            .expect("ordinary CLI arguments must remain valid");

        assert!(input.is_none());
        assert_eq!(remaining, arguments);
    }

    #[test]
    fn leading_global_argument_extracts_supply_and_preserves_command() {
        let (input, remaining) = split_authority_supply_process_argument(vec![
            "--authority-supply".to_string(),
            "/usr/lib/neebles/fixture/supply.json".to_string(),
            "socket".to_string(),
            "serve".to_string(),
        ])
        .expect("leading global authority supply must parse");

        let input = input.expect("authority supply input must exist");

        assert_eq!(
            input.supply_path(),
            Path::new("/usr/lib/neebles/fixture/supply.json")
        );

        assert_eq!(remaining, vec!["socket".to_string(), "serve".to_string()]);
    }

    #[test]
    fn malformed_global_argument_is_rejected() {
        assert!(split_authority_supply_process_argument(vec![
            "--authority-supply".to_string(),
            "relative.json".to_string(),
            "socket".to_string(),
            "serve".to_string(),
        ])
        .is_err());

        assert!(split_authority_supply_process_argument(vec![
            "socket".to_string(),
            "serve".to_string(),
            "--authority-supply".to_string(),
            "/usr/lib/neebles/fixture/supply.json".to_string(),
        ])
        .is_err());

        assert!(
            split_authority_supply_process_argument(vec!["--authority-supply".to_string(),])
                .is_err()
        );
    }

    #[test]
    fn explicit_user_owned_supply_is_transportable_but_not_authenticatable() {
        let base = fixture_root("user");

        fs::create_dir_all(&base).expect("fixture base must exist");

        let supply_path = base.join("supply.json");

        fs::write(
            &supply_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "1",
                "name": "neebles-authority-supply",
                "entries": []
            }))
            .expect("fixture supply must serialize"),
        )
        .expect("fixture supply must exist");

        let (input, remaining) = split_authority_supply_process_argument(vec![
            "--authority-supply".to_string(),
            supply_path.to_string_lossy().into_owned(),
            "socket".to_string(),
            "serve".to_string(),
        ])
        .expect("transport syntax must accept an explicit absolute candidate");

        assert_eq!(remaining, vec!["socket".to_string(), "serve".to_string()]);

        assert!(load_authority_supply_process_state(
            &input.expect("transport must expose path candidate")
        )
        .is_err());

        fs::remove_dir_all(&base).expect("fixture must clean");
    }
}
