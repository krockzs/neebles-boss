pub trait CriticalUpdateConsumer {
    fn accepts(&self, instruction: &str) -> bool;

    fn execute(&self, instruction: &str) -> Result<(), String>;
}

pub fn execute_instructions(
    instructions: &[String],
    consumers: &[&dyn CriticalUpdateConsumer],
) -> Result<usize, String> {
    let mut executed = 0usize;

    for instruction in instructions {
        if instruction.trim().is_empty() {
            return Err("Critical Update instruction must not be empty".to_string());
        }

        let matches: Vec<_> = consumers
            .iter()
            .filter(|consumer| consumer.accepts(instruction))
            .collect();

        if matches.is_empty() {
            return Err(format!(
                "Critical Update instruction has no consumer: {instruction}"
            ));
        }

        if matches.len() != 1 {
            return Err(format!(
                "Critical Update instruction has ambiguous consumers: {instruction}"
            ));
        }

        matches[0].execute(instruction)?;

        executed += 1;
    }

    Ok(executed)
}

pub struct NightmareConsumer {
    catalog: serde_json::Value,
}

impl NightmareConsumer {
    pub fn load() -> Result<Self, String> {
        Ok(Self {
            catalog: crate::nightmare::load_catalog()?,
        })
    }

    #[cfg(test)]
    fn from_catalog(catalog: serde_json::Value) -> Self {
        Self { catalog }
    }
}

impl CriticalUpdateConsumer for NightmareConsumer {
    fn accepts(&self, instruction: &str) -> bool {
        crate::nightmare::resolve_formula_from_catalog(&self.catalog, instruction)
            .ok()
            .and_then(|formula| formula.execution)
            .is_some()
    }

    fn execute(&self, instruction: &str) -> Result<(), String> {
        crate::nightmare::execute_formula_from_catalog(&self.catalog, instruction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Consumer {
        accept: bool,
        seen: RefCell<Vec<String>>,
        fail: bool,
    }

    impl CriticalUpdateConsumer for Consumer {
        fn accepts(&self, _instruction: &str) -> bool {
            self.accept
        }

        fn execute(&self, instruction: &str) -> Result<(), String> {
            self.seen.borrow_mut().push(instruction.to_string());

            if self.fail {
                Err("consumer failure".to_string())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn nightmare_consumer_accepts_only_executable_formula() {
        let consumer = NightmareConsumer::from_catalog(serde_json::json!({
            "formulas": {
                "nightmare.executable.v1": {
                    "reader": {},
                    "writer": {},
                    "execution": {}
                },
                "nightmare.passive.v1": {
                    "reader": {},
                    "writer": {}
                }
            }
        }));

        assert!(consumer.accepts("nightmare.executable.v1"));

        assert!(!consumer.accepts("nightmare.passive.v1"));

        assert!(!consumer.accepts("nightmare.missing.v1"));
    }

    #[test]
    fn empty_manifest_executes_nothing() {
        let consumers: Vec<&dyn CriticalUpdateConsumer> = Vec::new();

        assert_eq!(execute_instructions(&[], &consumers).unwrap(), 0);
    }

    #[test]
    fn opaque_instruction_is_preserved() {
        let consumer = Consumer {
            accept: true,
            seen: RefCell::new(Vec::new()),
            fail: false,
        };

        let instructions = vec!["orden.uno".to_string()];

        assert_eq!(
            execute_instructions(&instructions, &[&consumer]).unwrap(),
            1
        );

        assert_eq!(consumer.seen.borrow().as_slice(), ["orden.uno"]);
    }

    #[test]
    fn missing_consumer_is_rejected() {
        let instruction = vec!["orden.uno".to_string()];

        assert!(execute_instructions(&instruction, &[]).is_err());
    }

    #[test]
    fn ambiguous_consumers_are_rejected() {
        let first = Consumer {
            accept: true,
            seen: RefCell::new(Vec::new()),
            fail: false,
        };

        let second = Consumer {
            accept: true,
            seen: RefCell::new(Vec::new()),
            fail: false,
        };

        assert!(execute_instructions(&["orden.uno".to_string()], &[&first, &second]).is_err());
    }

    #[test]
    fn consumer_failure_is_propagated() {
        let consumer = Consumer {
            accept: true,
            seen: RefCell::new(Vec::new()),
            fail: true,
        };

        assert!(execute_instructions(&["orden.uno".to_string()], &[&consumer]).is_err());
    }
}
