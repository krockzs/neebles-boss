use crate::lifecycle::LifecycleContract;
use crate::lifecycle_battlefield::Battlefield;
use crate::lifecycle_ir::{self, BattleplanIr};

/*
 * Lifecycle conventional open / close discovery.
 *
 * Technical responsibility:
 * Expose whether the module declares the conventional top-level
 * Lifecycle intentions "open" and "close", and compile them through
 * the same generic transition compiler already used by Lifecycle.
 *
 * This layer does not:
 * - require open
 * - require close
 * - require both together
 * - execute anything
 * - know UI, tray or launcher semantics
 * - know what open or close physically do
 *
 * The module owns intention.
 * Boss only discovers and compiles the declared transition.
 */

const OPEN_TRANSITION: &str = "open";
const CLOSE_TRANSITION: &str = "close";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OpenCloseAvailability {
    open: bool,
    close: bool,
}

impl OpenCloseAvailability {
    pub fn open(&self) -> bool {
        self.open
    }

    pub fn close(&self) -> bool {
        self.close
    }

    pub fn any(&self) -> bool {
        self.open || self.close
    }
}

pub fn availability(contract: &LifecycleContract) -> OpenCloseAvailability {
    OpenCloseAvailability {
        open: contract.transitions.contains_key(OPEN_TRANSITION),
        close: contract.transitions.contains_key(CLOSE_TRANSITION),
    }
}

fn compile_optional_transition(
    contract: &LifecycleContract,
    battlefield: &Battlefield,
    transition_id: &str,
) -> Result<Option<BattleplanIr>, String> {
    if !contract.transitions.contains_key(transition_id) {
        return Ok(None);
    }

    lifecycle_ir::compile_transition(contract, transition_id, battlefield).map(Some)
}

pub fn compile_open(
    contract: &LifecycleContract,
    battlefield: &Battlefield,
) -> Result<Option<BattleplanIr>, String> {
    compile_optional_transition(contract, battlefield, OPEN_TRANSITION)
}

pub fn compile_close(
    contract: &LifecycleContract,
    battlefield: &Battlefield,
) -> Result<Option<BattleplanIr>, String> {
    compile_optional_transition(contract, battlefield, CLOSE_TRANSITION)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::lifecycle::{Battleplan, Operation};

    use std::collections::BTreeMap;

    fn operation(objective: &str) -> Operation {
        Operation {
            artillery: "synthetic-artillery".to_string(),
            objective: objective.to_string(),
            munition: BTreeMap::new(),
            tactics: BTreeMap::new(),
            intelligence: BTreeMap::new(),
        }
    }

    fn battleplan(objective: &str) -> Battleplan {
        Battleplan {
            operations: BTreeMap::from([("synthetic-operation".to_string(), operation(objective))]),
        }
    }

    #[test]
    fn neither_open_nor_close_is_valid() {
        let contract = LifecycleContract::default();
        let state = availability(&contract);

        assert!(!state.open());
        assert!(!state.close());
        assert!(!state.any());
    }

    #[test]
    fn open_can_exist_without_close() {
        let mut contract = LifecycleContract::default();

        contract
            .transitions
            .insert("open".to_string(), battleplan("future.open.behavior"));

        let state = availability(&contract);

        assert!(state.open());
        assert!(!state.close());
        assert!(state.any());
    }

    #[test]
    fn close_can_exist_without_open() {
        let mut contract = LifecycleContract::default();

        contract
            .transitions
            .insert("close".to_string(), battleplan("future.close.behavior"));

        let state = availability(&contract);

        assert!(!state.open());
        assert!(state.close());
    }

    #[test]
    fn open_and_close_can_exist_together() {
        let mut contract = LifecycleContract::default();

        contract
            .transitions
            .insert("open".to_string(), battleplan("future.open.behavior"));

        contract
            .transitions
            .insert("close".to_string(), battleplan("future.close.behavior"));

        let state = availability(&contract);

        assert!(state.open());
        assert!(state.close());
    }

    #[test]
    fn unrelated_transition_does_not_imply_open_or_close() {
        let mut contract = LifecycleContract::default();

        contract
            .transitions
            .insert("whatever".to_string(), battleplan("future.whatever"));

        let state = availability(&contract);

        assert!(!state.open());
        assert!(!state.close());
    }

    #[test]
    fn missing_open_compiles_to_none() {
        let contract = LifecycleContract::default();
        let battlefield = Battlefield::new();

        let result = compile_open(&contract, &battlefield).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn missing_close_compiles_to_none() {
        let contract = LifecycleContract::default();
        let battlefield = Battlefield::new();

        let result = compile_close(&contract, &battlefield).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn declared_open_uses_generic_transition_compiler() {
        let mut contract = LifecycleContract::default();

        contract
            .transitions
            .insert("open".to_string(), battleplan("future.open.behavior"));

        let battlefield = Battlefield::new();

        let compiled = compile_open(&contract, &battlefield).unwrap().unwrap();

        assert!(compiled.contains("synthetic-operation"));

        assert_eq!(
            compiled.get("synthetic-operation").unwrap().objective,
            "future.open.behavior"
        );
    }

    #[test]
    fn declared_close_uses_generic_transition_compiler() {
        let mut contract = LifecycleContract::default();

        contract
            .transitions
            .insert("close".to_string(), battleplan("future.close.behavior"));

        let battlefield = Battlefield::new();

        let compiled = compile_close(&contract, &battlefield).unwrap().unwrap();

        assert!(compiled.contains("synthetic-operation"));

        assert_eq!(
            compiled.get("synthetic-operation").unwrap().objective,
            "future.close.behavior"
        );
    }

    #[test]
    fn open_objective_remains_opaque() {
        let mut contract = LifecycleContract::default();

        contract
            .transitions
            .insert("open".to_string(), battleplan("anything.module.decides"));

        let battlefield = Battlefield::new();

        let compiled = compile_open(&contract, &battlefield).unwrap().unwrap();

        assert_eq!(
            compiled.get("synthetic-operation").unwrap().objective,
            "anything.module.decides"
        );
    }

    #[test]
    fn close_objective_remains_opaque() {
        let mut contract = LifecycleContract::default();

        contract
            .transitions
            .insert("close".to_string(), battleplan("unknown.future.close"));

        let battlefield = Battlefield::new();

        let compiled = compile_close(&contract, &battlefield).unwrap().unwrap();

        assert_eq!(
            compiled.get("synthetic-operation").unwrap().objective,
            "unknown.future.close"
        );
    }

    #[test]
    fn discovery_does_not_execute_transition() {
        let mut contract = LifecycleContract::default();

        contract
            .transitions
            .insert("open".to_string(), battleplan("nothing.executes.here"));

        let state = availability(&contract);

        assert!(state.open());
    }
}
