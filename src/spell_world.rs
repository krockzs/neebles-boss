use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellReference {
    pub world: String,
    pub spell: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedSpell {
    pub reference: SpellReference,
    pub world_root: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct SpellWorldRegistry {
    worlds: BTreeMap<String, PathBuf>,
}

impl SpellReference {
    pub fn parse(value: &str) -> Result<Self, String> {
        let (world, spell) = value
            .split_once('.')
            .ok_or_else(|| format!("spell reference must use <world>.<spell>: {value}"))?;

        if world.trim().is_empty() {
            return Err("spell reference world cannot be empty".to_string());
        }

        if spell.trim().is_empty() {
            return Err("spell reference spell cannot be empty".to_string());
        }

        Ok(Self {
            world: world.to_string(),
            spell: spell.to_string(),
        })
    }
}

impl SpellWorldRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        name: impl Into<String>,
        absolute_path: impl Into<PathBuf>,
    ) -> Result<(), String> {
        let name = name.into();
        let absolute_path = absolute_path.into();

        if name.trim().is_empty() {
            return Err("spell world name cannot be empty".to_string());
        }

        if name.contains('.') {
            return Err(format!("spell world name cannot contain dot: {name}"));
        }

        if !absolute_path.is_absolute() {
            return Err(format!(
                "spell world path must be absolute: {}",
                absolute_path.display()
            ));
        }

        if self.worlds.contains_key(&name) {
            return Err(format!("spell world already registered: {name}"));
        }

        self.worlds.insert(name, absolute_path);

        Ok(())
    }

    pub fn world_root(&self, name: &str) -> Result<&Path, String> {
        self.worlds
            .get(name)
            .map(PathBuf::as_path)
            .ok_or_else(|| format!("unknown spell world: {name}"))
    }

    pub fn locate(&self, value: &str) -> Result<LocatedSpell, String> {
        let reference = SpellReference::parse(value)?;

        let world_root = self.world_root(&reference.world)?.to_path_buf();

        Ok(LocatedSpell {
            reference,
            world_root,
        })
    }
}
