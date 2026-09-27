use goblin::elf::Elf;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchReferenceAuthority {
    WorldAbsolute,
    ElfOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedSearchReference {
    pub authority: SearchReferenceAuthority,
    pub reference: PathBuf,
}

pub fn project_search_reference(raw: &str) -> Result<ProjectedSearchReference, String> {
    if raw.is_empty() {
        return Err("empty ELF search reference".to_string());
    }

    let token = format!("{}ORIGIN", 36u8 as char);

    if raw == token {
        return Ok(ProjectedSearchReference {
            authority: SearchReferenceAuthority::ElfOrigin,
            reference: PathBuf::from("."),
        });
    }

    let origin_prefix = format!("{}/", token);

    if let Some(relative) = raw.strip_prefix(&origin_prefix) {
        if relative.is_empty() {
            return Err("empty ELF origin-relative search reference".to_string());
        }

        return Ok(ProjectedSearchReference {
            authority: SearchReferenceAuthority::ElfOrigin,
            reference: PathBuf::from(relative),
        });
    }

    if raw.starts_with(36u8 as char) {
        return Err(format!(
            "unknown dynamic loader token in search reference: {raw}"
        ));
    }

    let path = PathBuf::from(raw);

    if path.is_absolute() {
        return Ok(ProjectedSearchReference {
            authority: SearchReferenceAuthority::WorldAbsolute,
            reference: path,
        });
    }

    Err(format!(
        "bare relative ELF search reference has no explicit authority: {raw}"
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSearchCandidate {
    pub search_index: usize,
    pub path: PathBuf,
}

pub fn resolve_search_candidate(
    world_root: &Path,
    elf_path: &Path,
    searches: &[ProjectedSearchReference],
    needed: &str,
) -> Result<ResolvedSearchCandidate, String> {
    if needed.is_empty() {
        return Err("empty ELF needed name".to_string());
    }

    let elf_directory = elf_path
        .parent()
        .ok_or_else(|| format!("ELF has no parent directory: {}", elf_path.display()))?;

    for (search_index, search) in searches.iter().enumerate() {
        let reference = search.reference.join(needed);

        let physical_candidate = match search.authority {
            SearchReferenceAuthority::WorldAbsolute => {
                let relative = reference.strip_prefix("/").map_err(|error| {
                    format!(
                        "absolute search reference could not be normalized {}: {error}",
                        reference.display()
                    )
                })?;

                world_root.join(relative)
            }

            SearchReferenceAuthority::ElfOrigin => elf_directory.join(&reference),
        };

        match fs::symlink_metadata(&physical_candidate) {
            Ok(_) => {}

            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                continue;
            }

            Err(error) => {
                return Err(format!(
                    "could not inspect search candidate {}: {error}",
                    physical_candidate.display()
                ));
            }
        }

        let base = match search.authority {
            SearchReferenceAuthority::WorldAbsolute => world_root,

            SearchReferenceAuthority::ElfOrigin => elf_directory,
        };

        let resolved =
            crate::domestic_world::resolve_world_reference(world_root, base, &reference)?;

        return Ok(ResolvedSearchCandidate {
            search_index,
            path: resolved,
        });
    }

    Err(format!(
        "ELF needed object was not found in declared search references: {needed}"
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectSearchSource {
    Rpath,
    Runpath,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectSearchPolicy {
    pub source: DirectSearchSource,
    pub entries: Vec<PathBuf>,
}

pub fn direct_search_policy(metadata: &DomesticElfMetadata) -> DirectSearchPolicy {
    if !metadata.runpath.is_empty() {
        return DirectSearchPolicy {
            source: DirectSearchSource::Runpath,
            entries: metadata.runpath.clone(),
        };
    }

    if !metadata.rpath.is_empty() {
        return DirectSearchPolicy {
            source: DirectSearchSource::Rpath,
            entries: metadata.rpath.clone(),
        };
    }

    DirectSearchPolicy {
        source: DirectSearchSource::None,
        entries: Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransitiveSearchSource {
    Rpath,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransitiveSearchContext {
    pub source: TransitiveSearchSource,
    pub entries: Vec<PathBuf>,
}

pub fn transitive_search_inheritance(policy: &DirectSearchPolicy) -> TransitiveSearchContext {
    match policy.source {
        DirectSearchSource::Rpath => TransitiveSearchContext {
            source: TransitiveSearchSource::Rpath,
            entries: policy.entries.clone(),
        },

        DirectSearchSource::Runpath | DirectSearchSource::None => TransitiveSearchContext {
            source: TransitiveSearchSource::None,
            entries: Vec::new(),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveSearchComposition {
    pub entries: Vec<PathBuf>,
    pub next_transitive: TransitiveSearchContext,
}

pub fn normalize_search_entries(entries: &[PathBuf]) -> Vec<PathBuf> {
    let mut normalized = Vec::new();

    for entry in entries {
        if !normalized.contains(entry) {
            normalized.push(entry.clone());
        }
    }

    normalized
}

pub fn effective_search_composition(
    inherited: &TransitiveSearchContext,
    current: &DirectSearchPolicy,
) -> EffectiveSearchComposition {
    let mut entries = Vec::new();

    let mut next_entries = inherited.entries.clone();

    match current.source {
        DirectSearchSource::Rpath => {
            entries.extend(current.entries.iter().cloned());

            entries.extend(inherited.entries.iter().cloned());

            let mut combined = current.entries.clone();

            combined.extend(inherited.entries.iter().cloned());

            next_entries = combined;
        }

        DirectSearchSource::Runpath => {
            entries.extend(inherited.entries.iter().cloned());

            entries.extend(current.entries.iter().cloned());
        }

        DirectSearchSource::None => {
            entries.extend(inherited.entries.iter().cloned());
        }
    }

    let entries = normalize_search_entries(&entries);

    let next_entries = normalize_search_entries(&next_entries);

    let next_transitive = if next_entries.is_empty() {
        TransitiveSearchContext {
            source: TransitiveSearchSource::None,
            entries: Vec::new(),
        }
    } else {
        TransitiveSearchContext {
            source: TransitiveSearchSource::Rpath,
            entries: next_entries,
        }
    };

    EffectiveSearchComposition {
        entries,
        next_transitive,
    }
}

pub fn effective_search_authorities(
    composition: &EffectiveSearchComposition,
    controlled_defaults: &[PathBuf],
) -> Result<Vec<ProjectedSearchReference>, String> {
    composition
        .entries
        .iter()
        .chain(controlled_defaults.iter())
        .map(|entry| project_search_reference(&entry.to_string_lossy()))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomesticElfMetadata {
    pub path: PathBuf,
    pub interpreter: Option<PathBuf>,
    pub needed: Vec<String>,
    pub raw_rpath: Vec<String>,
    pub raw_runpath: Vec<String>,
    pub rpath: Vec<PathBuf>,
    pub runpath: Vec<PathBuf>,
}

pub fn project_search_path_entries(raw: &str) -> Result<Vec<ProjectedSearchReference>, String> {
    parse_search_path_entries(raw)
        .iter()
        .map(|entry| project_search_reference(&entry.to_string_lossy()))
        .collect()
}

pub fn parse_search_path_entries(value: &str) -> Vec<PathBuf> {
    value.split(':').map(PathBuf::from).collect()
}

pub fn inspect_elf(path: &Path) -> Result<DomesticElfMetadata, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("could not read ELF {}: {error}", path.display()))?;

    let elf = Elf::parse(&bytes)
        .map_err(|error| format!("could not parse ELF {}: {error}", path.display()))?;

    let interpreter = elf.interpreter.map(PathBuf::from);

    let needed = elf
        .libraries
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();

    let raw_rpath = elf
        .rpaths
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();

    let raw_runpath = elf
        .runpaths
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();

    let rpath = raw_rpath
        .iter()
        .flat_map(|value| parse_search_path_entries(value))
        .collect::<Vec<_>>();

    let runpath = raw_runpath
        .iter()
        .flat_map(|value| parse_search_path_entries(value))
        .collect::<Vec<_>>();

    Ok(DomesticElfMetadata {
        path: path.to_path_buf(),
        interpreter,
        needed,
        raw_rpath,
        raw_runpath,
        rpath,
        runpath,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ElfClosureState {
    pub path: PathBuf,
    pub inherited: TransitiveSearchContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedNeededAuthorities {
    pub dependencies: Vec<ResolvedSearchCandidate>,
    pub next_transitive: TransitiveSearchContext,
}

pub fn resolve_needed_authority(
    world_root: &Path,
    metadata: &DomesticElfMetadata,
    inherited: &TransitiveSearchContext,
    controlled_defaults: &[PathBuf],
    needed: &str,
) -> Result<ResolvedSearchCandidate, String> {
    let direct = direct_search_policy(metadata);

    let composition = effective_search_composition(inherited, &direct);

    let authorities = effective_search_authorities(&composition, controlled_defaults)?;

    resolve_search_candidate(world_root, &metadata.path, &authorities, needed)
}

pub fn resolve_needed_authorities(
    world_root: &Path,
    metadata: &DomesticElfMetadata,
    inherited: &TransitiveSearchContext,
    controlled_defaults: &[PathBuf],
) -> Result<ResolvedNeededAuthorities, String> {
    let direct = direct_search_policy(metadata);

    let composition = effective_search_composition(inherited, &direct);

    let authorities = effective_search_authorities(&composition, controlled_defaults)?;

    let mut dependencies = Vec::new();

    for needed in &metadata.needed {
        dependencies.push(resolve_search_candidate(
            world_root,
            &metadata.path,
            &authorities,
            needed,
        )?);
    }

    Ok(ResolvedNeededAuthorities {
        dependencies,
        next_transitive: composition.next_transitive,
    })
}

pub fn resolve_elf_recursive_closure<F>(
    world_root: &Path,
    root: &Path,
    initial_inherited: &TransitiveSearchContext,
    controlled_defaults: &[PathBuf],
    metadata_for: F,
) -> Result<std::collections::BTreeSet<ElfClosureState>, String>
where
    F: Fn(&Path) -> Result<DomesticElfMetadata, String>,
{
    let certified_root =
        crate::domestic_world::certify_world_path(world_root, root, "ELF recursive root")?;

    let root_state = ElfClosureState {
        path: certified_root,
        inherited: initial_inherited.clone(),
    };

    crate::domestic_world::certify_stateful_recursive_closure(root_state, |state| {
        let metadata = metadata_for(&state.path)?;

        if metadata.path != state.path {
            return Err(format!(
                "ELF metadata path does not match recursive state: state={} metadata={}",
                state.path.display(),
                metadata.path.display()
            ));
        }

        let frontier = resolve_needed_authorities(
            world_root,
            &metadata,
            &state.inherited,
            controlled_defaults,
        )?;

        Ok(frontier
            .dependencies
            .into_iter()
            .map(|dependency| ElfClosureState {
                path: dependency.path,
                inherited: frontier.next_transitive.clone(),
            })
            .collect())
    })
}

pub fn observed_needed_authorities(metadata: &DomesticElfMetadata) -> Vec<String> {
    metadata
        .needed
        .iter()
        .map(|needed| format!("elf.needed:{needed}"))
        .collect()
}

pub fn resolve_interpreter_authority(
    world_root: &Path,
    metadata: &DomesticElfMetadata,
) -> Result<Option<PathBuf>, String> {
    let Some(interpreter) = metadata.interpreter.as_ref() else {
        return Ok(None);
    };

    if !interpreter.is_absolute() {
        return Err(format!(
            "ELF interpreter must be absolute: {}",
            interpreter.display()
        ));
    }

    let resolved =
        crate::domestic_world::resolve_world_reference(world_root, world_root, interpreter)?;

    Ok(Some(resolved))
}

pub fn observed_interpreter_authorities(metadata: &DomesticElfMetadata) -> Vec<String> {
    metadata
        .interpreter
        .as_ref()
        .map(|interpreter| vec![format!("elf.interpreter:{}", interpreter.display())])
        .unwrap_or_default()
}
