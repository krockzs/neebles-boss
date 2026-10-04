use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use crate::lifecycle_capabilities::{CapabilityFuture, CapabilityHandler};
use crate::lifecycle_execution::ExecutionPayload;
use crate::lifecycle_operation::PreparedOperation;

#[derive(Clone)]
pub struct AvailableCapability {
    implementation_id: String,
    handler: CapabilityHandler,
}

impl AvailableCapability {
    pub fn new(
        implementation_id: impl Into<String>,
        handler: CapabilityHandler,
    ) -> Result<Self, String> {
        let implementation_id = implementation_id.into();

        if implementation_id.trim().is_empty() {
            return Err("available capability implementation id cannot be empty".to_string());
        }

        Ok(Self {
            implementation_id,
            handler,
        })
    }

    pub fn implementation_id(&self) -> &str {
        &self.implementation_id
    }

    pub fn handler(&self) -> CapabilityHandler {
        self.handler.clone()
    }
}

#[derive(Default)]
pub struct AvailableCapabilityCatalog {
    capabilities: BTreeMap<String, AvailableCapability>,
}

impl AvailableCapabilityCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        artillery: impl Into<String>,
        capability: AvailableCapability,
    ) -> Result<(), String> {
        let artillery = artillery.into();

        if artillery.trim().is_empty() {
            return Err("available capability artillery id cannot be empty".to_string());
        }

        if self.capabilities.contains_key(&artillery) {
            return Err(format!(
                "available capability artillery '{}' is already registered",
                artillery
            ));
        }

        self.capabilities.insert(artillery, capability);

        Ok(())
    }

    pub fn resolve(&self, artillery: &str) -> Result<&AvailableCapability, String> {
        if artillery.trim().is_empty() {
            return Err("available capability artillery id cannot be empty".to_string());
        }

        self.capabilities.get(artillery).ok_or_else(|| {
            format!(
                "Boss has no available Rust capability for artillery '{}'",
                artillery
            )
        })
    }

    pub fn contains(&self, artillery: &str) -> bool {
        self.capabilities.contains_key(artillery)
    }
}

const WORKSPACE_ARTILLERY: &str = "boss.workspace_execution";

const WORKSPACE_OBJECTIVE: &str = "construction.step";

const WORKSPACE_IMPLEMENTATION: &str = "rust.boss.workspace_execution";

fn workspace_operation_identity(operation: &PreparedOperation) -> Result<(String, String), String> {
    if operation.objective != WORKSPACE_OBJECTIVE {
        return Err(format!(
            "artillery '{}' does not support objective '{}'; expected '{}'",
            WORKSPACE_ARTILLERY, operation.objective, WORKSPACE_OBJECTIVE,
        ));
    }

    for key in operation.munition.keys() {
        if key != "subject" && key != "step" {
            return Err(format!(
                "workspace execution munition contains unknown key '{}'",
                key
            ));
        }
    }

    if operation.munition.len() != 2 {
        return Err(
            "workspace execution requires exactly munition.subject and munition.step".to_string(),
        );
    }

    let subject = operation
        .munition
        .get("subject")
        .ok_or_else(|| "workspace execution munition.subject is missing".to_string())?
        .clone();

    let step = operation
        .munition
        .get("step")
        .ok_or_else(|| "workspace execution munition.step is missing".to_string())?
        .clone();

    if subject.trim().is_empty() {
        return Err("workspace execution munition.subject cannot be empty".to_string());
    }

    if subject.trim() != subject {
        return Err(
            "workspace execution munition.subject cannot contain surrounding whitespace"
                .to_string(),
        );
    }

    if step.trim().is_empty() {
        return Err("workspace execution munition.step cannot be empty".to_string());
    }

    if step.trim() != step {
        return Err(
            "workspace execution munition.step cannot contain surrounding whitespace".to_string(),
        );
    }

    Ok((subject, step))
}

/*
 * DomesticConstruction already owns the canonical conversion from
 * subject + step into the authenticated WorkspaceExecutionRequest.
 *
 * Lifecycle must not recreate that policy.
 *
 * The resulting std::process::Command is already fully composed by
 * the domestic boundary engine and has env_clear semantics.
 *
 * We reproduce only the finished process envelope using
 * async_process so Lifecycle does not turn one long domestic step
 * into a synchronous blocker for unrelated asynchronous work.
 */
fn async_domestic_command(command: std::process::Command) -> async_process::Command {
    let mut asynchronous = async_process::Command::new(command.get_program());

    asynchronous.env_clear();

    asynchronous.args(command.get_args());

    if let Some(directory) = command.get_current_dir() {
        asynchronous.current_dir(directory);
    }

    for (key, value) in command.get_envs() {
        if let Some(value) = value {
            asynchronous.env(key, value);
        }
    }

    asynchronous
}

async fn execute_workspace_operation(
    operation: PreparedOperation,
) -> Result<ExecutionPayload, String> {
    let (subject, step) = workspace_operation_identity(&operation)?;

    let declaration =
        neebles_backend::domestic_construction::load_canonical_domestic_construction_declaration(
            &subject,
        )?;

    let registry =
        neebles_backend::domestic_authority_supply_process::process_supplied_authority_registry()?;

    let execution = declaration.step(&step)?.execution;

    let mut command = neebles_backend::domestic_construction::build_construction_step_command(
        &declaration,
        &step,
        registry,
    )?;

    match execution {
        neebles_backend::domestic_construction::DomesticConstructionExecution::Foreground => {
            let status = async_domestic_command(command)
                .status()
                .await
                .map_err(|error| {
                    format!(
                        "domestic workspace execution failed to start: subject={} step={} error={}",
                        subject, step, error,
                    )
                })?;

            let exit_code = status.code().unwrap_or(125);

            if !status.success() {
                return Err(format!(
                    "domestic workspace execution failed: subject={} step={} exit_code={}",
                    subject, step, exit_code,
                ));
            }

            Ok(ExecutionPayload::from([
                ("subject".to_string(), subject),
                ("step".to_string(), step),
                ("execution".to_string(), "foreground".to_string()),
                ("exit_code".to_string(), exit_code.to_string()),
            ]))
        }

        neebles_backend::domestic_construction::DomesticConstructionExecution::Persistent => {
            let mut child = command.spawn().map_err(|error| {
                format!(
                    "persistent domestic workspace failed to start: subject={} step={} error={}",
                    subject, step, error,
                )
            })?;

            let pid = child.id();

            std::thread::spawn(move || {
                let _ = child.wait();
            });

            Ok(ExecutionPayload::from([
                ("subject".to_string(), subject),
                ("step".to_string(), step),
                ("execution".to_string(), "persistent".to_string()),
                ("pid".to_string(), pid.to_string()),
            ]))
        }
    }
}

fn workspace_execution_handler(operation: PreparedOperation) -> CapabilityFuture {
    Box::pin(execute_workspace_operation(operation))
}

static PRODUCTIVE_CATALOG: OnceLock<Arc<AvailableCapabilityCatalog>> = OnceLock::new();

fn build_productive_catalog() -> Result<AvailableCapabilityCatalog, String> {
    /*
     * Productive Rust arsenal entrypoint.
     *
     * Lifecycle does not know module semantics and does not know
     * individual Rust crates.
     *
     * The stable generic execution boundary is Boss domestic
     * workspace execution. Modules provide only declarative
     * subject + step identities through Lifecycle.
     *
     * Construction, AuthoritySupply, world resolution and
     * filesystem boundaries remain owned by their existing
     * canonical layers.
     */
    let mut catalog = AvailableCapabilityCatalog::new();

    catalog.register(
        WORKSPACE_ARTILLERY,
        AvailableCapability::new(
            WORKSPACE_IMPLEMENTATION,
            Arc::new(workspace_execution_handler),
        )?,
    )?;

    Ok(catalog)
}

pub fn productive_catalog() -> Result<Arc<AvailableCapabilityCatalog>, String> {
    if let Some(catalog) = PRODUCTIVE_CATALOG.get() {
        return Ok(Arc::clone(catalog));
    }

    let catalog = Arc::new(build_productive_catalog()?);

    match PRODUCTIVE_CATALOG.set(Arc::clone(&catalog)) {
        Ok(()) => Ok(catalog),

        Err(_) => PRODUCTIVE_CATALOG.get().cloned().ok_or_else(|| {
            "productive Lifecycle capability catalog initialization failed".to_string()
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    fn capability(implementation: &str) -> AvailableCapability {
        AvailableCapability::new(
            implementation,
            Arc::new(|_| Box::pin(async { Ok(ExecutionPayload::new()) })),
        )
        .unwrap()
    }

    #[test]
    fn catalog_starts_empty() {
        let catalog = AvailableCapabilityCatalog::new();

        assert!(!catalog.contains("anything"));
    }

    #[test]
    fn catalog_accepts_arbitrary_artillery() {
        let mut catalog = AvailableCapabilityCatalog::new();

        catalog
            .register("future.tool", capability("rust.future.tool"))
            .unwrap();

        assert!(catalog.contains("future.tool"));
    }

    #[test]
    fn catalog_keeps_implementation_private_to_boss() {
        let mut catalog = AvailableCapabilityCatalog::new();

        catalog
            .register("module.requests.this", capability("boss.private.adapter"))
            .unwrap();

        assert_eq!(
            catalog
                .resolve("module.requests.this")
                .unwrap()
                .implementation_id(),
            "boss.private.adapter"
        );
    }

    #[test]
    fn unavailable_artillery_fails_explicitly() {
        let catalog = AvailableCapabilityCatalog::new();

        let error = match catalog.resolve("missing.tool") {
            Ok(_) => {
                panic!("missing capability unexpectedly resolved");
            }

            Err(error) => error,
        };

        assert!(error.contains("no available Rust capability"));
    }

    #[test]
    fn productive_catalog_exposes_generic_workspace_execution() {
        let catalog = build_productive_catalog().expect("productive catalog must build");

        let available = catalog
            .resolve(WORKSPACE_ARTILLERY)
            .expect("workspace artillery must be AVAILABLE");

        assert_eq!(available.implementation_id(), WORKSPACE_IMPLEMENTATION,);
    }

    #[test]
    fn workspace_execution_rejects_unknown_objective_before_authority_use() {
        let operation = PreparedOperation {
            artillery: WORKSPACE_ARTILLERY.to_string(),

            objective: "future.unknown".to_string(),

            munition: BTreeMap::from([
                ("subject".to_string(), "fixture".to_string()),
                ("step".to_string(), "run".to_string()),
            ]),

            tactics: BTreeMap::new(),

            intelligence: BTreeMap::new(),
        };

        let error = futures_lite::future::block_on(execute_workspace_operation(operation))
            .expect_err("unknown objective must fail before execution");

        assert!(error.contains("does not support objective"));
    }

    #[test]
    fn workspace_execution_requires_exact_subject_and_step_munition() {
        let missing = PreparedOperation {
            artillery: WORKSPACE_ARTILLERY.to_string(),

            objective: WORKSPACE_OBJECTIVE.to_string(),

            munition: BTreeMap::from([("subject".to_string(), "fixture".to_string())]),

            tactics: BTreeMap::new(),

            intelligence: BTreeMap::new(),
        };

        let error = futures_lite::future::block_on(execute_workspace_operation(missing))
            .expect_err("missing step must fail before execution");

        assert!(error.contains("exactly munition.subject and munition.step"));

        let extra = PreparedOperation {
            artillery: WORKSPACE_ARTILLERY.to_string(),

            objective: WORKSPACE_OBJECTIVE.to_string(),

            munition: BTreeMap::from([
                ("subject".to_string(), "fixture".to_string()),
                ("step".to_string(), "run".to_string()),
                ("technology".to_string(), "forbidden".to_string()),
            ]),

            tactics: BTreeMap::new(),

            intelligence: BTreeMap::new(),
        };

        let error = futures_lite::future::block_on(execute_workspace_operation(extra))
            .expect_err("unknown munition must fail before execution");

        assert!(error.contains("unknown key"));
    }
}
