use neebles_backend::domestic_elf::{
    inspect_elf, observed_interpreter_authorities, observed_needed_authorities,
    project_search_path_entries, resolve_elf_recursive_closure, resolve_interpreter_authority,
    TransitiveSearchContext, TransitiveSearchSource,
};
use neebles_backend::domestic_observation::DomesticObservation;
use neebles_backend::domestic_search_authority::{
    resolve_execution_search_authorities, DomesticSearchAuthorityRegistry,
};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct SpellObservationReport {
    evidence_total: usize,
    findings_total: usize,
    evidence_frequencies: BTreeMap<String, usize>,
    finding_frequencies: BTreeMap<String, usize>,
    evidence_examples: Vec<String>,
    finding_examples: Vec<String>,
}

#[derive(Default)]
struct WorldReport {
    elf_total: usize,
    with_search_testimony: usize,
    legal_testimonies: usize,
    rejected_testimonies: usize,
    spell_observations: BTreeMap<String, SpellObservationReport>,
    read_errors: usize,
    skipped_paths: usize,
    skipped_examples: Vec<String>,
    symlink_paths: usize,
    symlink_examples: Vec<String>,
    symlink_to_elf: usize,
    symlink_inside_world: usize,
    symlink_outside_world: usize,
    symlink_broken: usize,
    symlink_elf_examples: Vec<String>,
    symlink_escape_examples: Vec<String>,
    rejection_families: BTreeMap<String, usize>,
    rejection_examples: BTreeMap<String, Vec<String>>,
}

fn record_spell_observation(
    report: &mut WorldReport,
    spell_reference: &str,
    subject: &str,
    observation: DomesticObservation,
) {
    let channel = report
        .spell_observations
        .entry(spell_reference.to_string())
        .or_default();

    channel.evidence_total += observation.evidence.len();
    channel.findings_total += observation.findings.len();

    for evidence in observation.evidence {
        *channel
            .evidence_frequencies
            .entry(evidence.clone())
            .or_insert(0) += 1;

        if channel.evidence_examples.len() < 30 {
            channel
                .evidence_examples
                .push(format!("{subject} :: {evidence}"));
        }
    }

    for finding in observation.findings {
        *channel
            .finding_frequencies
            .entry(finding.clone())
            .or_insert(0) += 1;

        if channel.finding_examples.len() < 30 {
            channel
                .finding_examples
                .push(format!("{subject} :: {finding}"));
        }
    }
}

fn is_elf(path: &Path) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return false;
    };

    bytes.starts_with(&[0x7f, b'E', b'L', b'F'])
}

fn walk(root: &Path, current: &Path, report: &mut WorldReport) -> Result<(), String> {
    let entries = match fs::read_dir(current) {
        Ok(entries) => entries,

        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            report.skipped_paths += 1;
            if report.skipped_examples.len() < 30 {
                report.skipped_examples.push(
                    current
                        .strip_prefix(root)
                        .unwrap_or(current)
                        .display()
                        .to_string(),
                );
            }

            return Ok(());
        }

        Err(error) => {
            return Err(format!(
                "could not read directory {}: {error}",
                current.display()
            ));
        }
    };

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "could not read directory entry in {}: {error}",
                current.display()
            )
        })?;

        let path = entry.path();

        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;

        if metadata.file_type().is_symlink() {
            report.symlink_paths += 1;

            if report.symlink_examples.len() < 30 {
                report.symlink_examples.push(
                    path.strip_prefix(root)
                        .unwrap_or(&path)
                        .display()
                        .to_string(),
                );
            }

            match neebles_backend::domestic_world::inspect_world_symlink(root, &path)? {
                neebles_backend::domestic_world::WorldSymlinkInspection::Inside(target) => {
                    report.symlink_inside_world += 1;

                    let world_semantic_elf = target.is_file() && is_elf(&target);

                    if world_semantic_elf {
                        report.symlink_to_elf += 1;

                        if report.symlink_elf_examples.len() < 30 {
                            let canonical_root = fs::canonicalize(root).map_err(|error| {
                                format!(
                                    "could not canonicalize world root {}: {error}",
                                    root.display()
                                )
                            })?;

                            report.symlink_elf_examples.push(format!(
                                "{} -> {}",
                                path.strip_prefix(root).unwrap_or(&path).display(),
                                target
                                    .strip_prefix(&canonical_root)
                                    .unwrap_or(&target)
                                    .display()
                            ));
                        }
                    }
                }

                neebles_backend::domestic_world::WorldSymlinkInspection::Outside(target) => {
                    report.symlink_outside_world += 1;

                    if report.symlink_escape_examples.len() < 30 {
                        report.symlink_escape_examples.push(format!(
                            "{} -> {}",
                            path.strip_prefix(root).unwrap_or(&path).display(),
                            target.display()
                        ));
                    }
                }

                neebles_backend::domestic_world::WorldSymlinkInspection::Broken(_) => {
                    report.symlink_broken += 1;
                }
            }

            continue;
        }

        if metadata.is_dir() {
            walk(root, &path, report)?;

            continue;
        }

        if !metadata.is_file() || !is_elf(&path) {
            continue;
        }

        report.elf_total += 1;

        let elf = match inspect_elf(&path) {
            Ok(value) => value,

            Err(_) => {
                report.read_errors += 1;
                continue;
            }
        };

        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string();

        let interpreter_evidence = observed_interpreter_authorities(&elf);

        let interpreter_findings = match resolve_interpreter_authority(root, &elf) {
            Ok(_) => Vec::new(),

            Err(error) => {
                vec![format!("elf.interpreter_resolution:{error}")]
            }
        };

        let interpreter_observation = DomesticObservation::with_evidence_and_findings(
            interpreter_evidence,
            interpreter_findings,
        );

        record_spell_observation(
            report,
            "boss.elf_interpreter_observation",
            &relative,
            interpreter_observation,
        );

        let needed_observation =
            DomesticObservation::with_evidence(observed_needed_authorities(&elf));

        record_spell_observation(
            report,
            "boss.elf_needed_observation",
            &relative,
            needed_observation,
        );

        let testimonies = elf
            .raw_rpath
            .iter()
            .map(|value| ("RPATH", value))
            .chain(elf.raw_runpath.iter().map(|value| ("RUNPATH", value)))
            .collect::<Vec<_>>();

        if !testimonies.is_empty() {
            report.with_search_testimony += 1;
        }

        for (kind, raw) in testimonies {
            match project_search_path_entries(raw) {
                Ok(_) => {
                    report.legal_testimonies += 1;
                }

                Err(error) => {
                    report.rejected_testimonies += 1;

                    let family = error.split(':').next().unwrap_or(&error).trim().to_string();

                    *report.rejection_families.entry(family.clone()).or_insert(0) += 1;

                    let examples = report.rejection_examples.entry(family).or_default();

                    if examples.len() < 20 {
                        examples.push(format!("{relative} :: {kind} :: {:?} :: {error}", raw));
                    }
                }
            }
        }
    }

    Ok(())
}

fn print_report(label: &str, root: &Path, report: &WorldReport) {
    println!("==============================================================================");

    println!("{label}");

    println!("==============================================================================");

    println!("ROOT :: {}", root.display());

    println!("ELF TOTAL :: {}", report.elf_total);

    println!(
        "ELF WITH SEARCH TESTIMONY :: {}",
        report.with_search_testimony
    );

    println!("LEGAL TESTIMONIES :: {}", report.legal_testimonies);

    println!("REJECTED TESTIMONIES :: {}", report.rejected_testimonies);

    println!("ELF READ ERRORS :: {}", report.read_errors);

    println!(
        "SPELL OBSERVATION CHANNELS :: {}",
        report.spell_observations.len()
    );

    println!("SKIPPED PATHS :: {}", report.skipped_paths);

    println!("SYMLINK PATHS :: {}", report.symlink_paths);

    println!("SYMLINK TO ELF :: {}", report.symlink_to_elf);

    println!("SYMLINK INSIDE WORLD :: {}", report.symlink_inside_world);

    println!("SYMLINK OUTSIDE WORLD :: {}", report.symlink_outside_world);

    println!("BROKEN SYMLINKS :: {}", report.symlink_broken);

    println!();

    println!("=== SKIPPED PATH EXAMPLES ===");

    if report.skipped_examples.is_empty() {
        println!("NONE");
    } else {
        for path in &report.skipped_examples {
            println!("{path}");
        }
    }

    println!();

    println!("=== SYMLINK PATH EXAMPLES ===");

    if report.symlink_examples.is_empty() {
        println!("NONE");
    } else {
        for path in &report.symlink_examples {
            println!("{path}");
        }
    }

    println!();

    println!("=== SYMLINK ELF EXAMPLES ===");

    if report.symlink_elf_examples.is_empty() {
        println!("NONE");
    } else {
        for value in &report.symlink_elf_examples {
            println!("{value}");
        }
    }

    println!();

    println!("=== SYMLINK ESCAPE EXAMPLES ===");

    if report.symlink_escape_examples.is_empty() {
        println!("NONE");
    } else {
        for value in &report.symlink_escape_examples {
            println!("{value}");
        }
    }

    println!();

    println!("=== SPELL OBSERVATIONS ===");

    if report.spell_observations.is_empty() {
        println!("NONE");
    } else {
        for (spell, observation) in &report.spell_observations {
            println!();
            println!("SPELL :: {spell}");
            println!("EVIDENCE :: {}", observation.evidence_total);
            println!(
                "UNIQUE EVIDENCE :: {}",
                observation.evidence_frequencies.len()
            );
            println!("FINDINGS :: {}", observation.findings_total);
            println!(
                "UNIQUE FINDINGS :: {}",
                observation.finding_frequencies.len()
            );

            println!("EVIDENCE EXAMPLES:");

            if observation.evidence_examples.is_empty() {
                println!("NONE");
            } else {
                for value in &observation.evidence_examples {
                    println!("{value}");
                }
            }

            println!("FINDING EXAMPLES:");

            if observation.finding_examples.is_empty() {
                println!("NONE");
            } else {
                for value in &observation.finding_examples {
                    println!("{value}");
                }
            }

            println!("EVIDENCE FREQUENCIES:");

            if observation.evidence_frequencies.is_empty() {
                println!("NONE");
            } else {
                for (value, count) in &observation.evidence_frequencies {
                    println!("{count} :: {value}");
                }
            }

            println!("FINDING FREQUENCIES:");

            if observation.finding_frequencies.is_empty() {
                println!("NONE");
            } else {
                for (value, count) in &observation.finding_frequencies {
                    println!("{count} :: {value}");
                }
            }
        }
    }

    println!();

    println!("=== REJECTION FAMILIES ===");

    if report.rejection_families.is_empty() {
        println!("NONE");
    } else {
        for (family, count) in &report.rejection_families {
            println!("{count} :: {family}");
        }
    }

    println!();

    println!("=== REJECTION EXAMPLES ===");

    if report.rejection_examples.is_empty() {
        println!("NONE");
    } else {
        for (family, examples) in &report.rejection_examples {
            println!();

            println!("[{family}]");

            for example in examples {
                println!("{example}");
            }
        }
    }

    println!();
}

#[derive(Default)]
struct CensusArguments {
    roots: Vec<PathBuf>,
    recursive_entries: Vec<PathBuf>,
    authority_registry: Option<PathBuf>,
    grants: Vec<String>,
    recursive_only: bool,
}

fn parse_arguments() -> Result<CensusArguments, String> {
    let mut parsed = CensusArguments::default();

    let mut arguments = env::args().skip(1).peekable();

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--entry" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| "--entry requires a domestic absolute path".to_string())?;

                let path = PathBuf::from(value);

                if !path.is_absolute() {
                    return Err(format!(
                        "recursive entry must be a domestic absolute path: {}",
                        path.display()
                    ));
                }

                parsed.recursive_entries.push(path);
            }

            "--authority-registry" => {
                if parsed.authority_registry.is_some() {
                    return Err("authority registry may be declared only once".to_string());
                }

                let value = arguments
                    .next()
                    .ok_or_else(|| "--authority-registry requires an absolute path".to_string())?;

                let path = PathBuf::from(value);

                if !path.is_absolute() {
                    return Err(format!(
                        "authority registry path must be absolute: {}",
                        path.display()
                    ));
                }

                parsed.authority_registry = Some(path);
            }

            "--grant" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| "--grant requires an authority name".to_string())?;

                let value = value.trim().to_string();

                if value.is_empty() {
                    return Err("granted authority name cannot be empty".to_string());
                }

                parsed.grants.push(value);
            }

            "--recursive-only" => {
                parsed.recursive_only = true;
            }

            "--help" => {
                return Err("help".to_string());
            }

            other if other.starts_with("--") => {
                return Err(format!("unknown census option: {other}"));
            }

            other => {
                parsed.roots.push(PathBuf::from(other));
            }
        }
    }

    if parsed.roots.is_empty() {
        return Err("at least one world root is required".to_string());
    }

    if !parsed.grants.is_empty() && parsed.authority_registry.is_none() {
        return Err("granted authorities require --authority-registry".to_string());
    }

    if parsed.recursive_only && parsed.recursive_entries.is_empty() {
        return Err("--recursive-only requires at least one --entry".to_string());
    }

    Ok(parsed)
}

fn print_usage() {
    eprintln!(
        "usage: neebles-elf-census ROOT... [--entry /domestic/path] [--authority-registry /absolute/file] [--grant authority] [--recursive-only]"
    );
}

fn observe_recursive_execution(
    world_root: &Path,
    entry: &Path,
    controlled_authorities: &[PathBuf],
) -> DomesticObservation {
    let physical_entry = match neebles_backend::domestic_world::resolve_world_reference(
        world_root, world_root, entry,
    ) {
        Ok(path) => path,

        Err(error) => {
            return DomesticObservation::with_findings(vec![format!(
                "elf.recursive_entry_resolution:{error}"
            )]);
        }
    };

    let initial = TransitiveSearchContext {
        source: TransitiveSearchSource::None,
        entries: Vec::new(),
    };

    match resolve_elf_recursive_closure(
        world_root,
        &physical_entry,
        &initial,
        controlled_authorities,
        inspect_elf,
    ) {
        Ok(states) => {
            let evidence = states
                .into_iter()
                .map(|state| {
                    let path = state
                        .path
                        .strip_prefix(world_root)
                        .map(|relative| PathBuf::from("/").join(relative))
                        .unwrap_or(state.path);

                    format!("elf.recursive_state:{}", path.display())
                })
                .collect();

            DomesticObservation::with_evidence(evidence)
        }

        Err(error) => {
            DomesticObservation::with_findings(vec![format!("elf.recursive_closure:{error}")])
        }
    }
}

fn main() {
    let arguments = match parse_arguments() {
        Ok(arguments) => arguments,

        Err(error) => {
            if error != "help" {
                eprintln!("{error}");
            }

            print_usage();

            std::process::exit(if error == "help" { 0 } else { 2 });
        }
    };

    let registry = match arguments.authority_registry.as_ref() {
        Some(path) => match DomesticSearchAuthorityRegistry::load(path) {
            Ok(registry) => registry,

            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        },

        None => DomesticSearchAuthorityRegistry::default(),
    };

    for root in arguments.roots {
        let controlled_authorities =
            match resolve_execution_search_authorities(&root, &registry, &arguments.grants) {
                Ok(authorities) => authorities,

                Err(error) => {
                    eprintln!("{} :: {error}", root.display());

                    std::process::exit(1);
                }
            };

        let mut report = WorldReport::default();

        let granted_observation = DomesticObservation::with_evidence(
            arguments
                .grants
                .iter()
                .map(|authority| format!("search.authority.granted:{authority}"))
                .collect(),
        );

        record_spell_observation(
            &mut report,
            "boss.search_authority_grants",
            "execution",
            granted_observation,
        );

        if !arguments.recursive_only {
            if let Err(error) = walk(&root, &root, &mut report) {
                eprintln!("{} :: {error}", root.display());

                std::process::exit(1);
            }
        }

        for entry in &arguments.recursive_entries {
            let observation = observe_recursive_execution(&root, entry, &controlled_authorities);

            record_spell_observation(
                &mut report,
                "boss.elf_recursive_closure",
                &entry.display().to_string(),
                observation,
            );
        }

        print_report("GOBLIN FIELD INTERROGATION", &root, &report);
    }
}
