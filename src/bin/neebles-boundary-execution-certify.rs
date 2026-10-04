use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use neebles_backend::domestic_boundary_execution::{
    build_pure_materialized_boundary_execution_command,
    compose_materialized_boundary_execution_plan,
};
use neebles_backend::domestic_external_data_authority::{
    load_external_data_authority_descriptor, ExternalDataAuthorityDescriptor,
};
use neebles_backend::domestic_platform_authority::load_platform_authority_descriptor;

fn require_value(args: &[OsString], index: &mut usize, option: &str) -> Result<OsString, String> {
    *index += 1;

    if *index >= args.len() {
        return Err(format!("{option} requires a value"));
    }

    Ok(args[*index].clone())
}

fn main_result() -> Result<i32, String> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();

    let mut manifest = None::<PathBuf>;
    let mut world = None::<String>;
    let mut platform_descriptor = None::<PathBuf>;
    let mut external_descriptors = Vec::<(String, PathBuf)>::new();
    let mut program_arguments = Vec::<OsString>::new();

    let mut index = 0usize;

    while index < args.len() {
        let argument = args[index].to_string_lossy().to_string();

        if argument == "--manifest" {
            manifest = Some(PathBuf::from(require_value(
                &args,
                &mut index,
                "--manifest",
            )?));

            index += 1;
            continue;
        }

        if argument == "--world" {
            world = Some(
                require_value(&args, &mut index, "--world")?
                    .to_string_lossy()
                    .to_string(),
            );

            index += 1;
            continue;
        }

        if argument == "--platform" {
            platform_descriptor = Some(PathBuf::from(require_value(
                &args,
                &mut index,
                "--platform",
            )?));

            index += 1;
            continue;
        }

        if argument == "--external" {
            let value = require_value(&args, &mut index, "--external")?
                .to_string_lossy()
                .to_string();

            let (authority, location) = value
                .split_once('=')
                .ok_or_else(|| "--external must use authority=descriptor".to_string())?;

            if authority.trim().is_empty() {
                return Err("external authority cannot be empty".to_string());
            }

            external_descriptors.push((authority.to_string(), PathBuf::from(location)));

            index += 1;
            continue;
        }

        if argument == "--" {
            program_arguments.extend(args.iter().skip(index + 1).cloned());

            break;
        }

        return Err(format!("unknown argument: {argument}"));
    }

    let manifest = manifest.ok_or_else(|| "missing --manifest".to_string())?;

    let world = world.ok_or_else(|| "missing --world".to_string())?;

    let platform_descriptor =
        platform_descriptor.ok_or_else(|| "missing --platform".to_string())?;

    let platform =
        load_platform_authority_descriptor(&platform_descriptor, "platform.filesystem_boundary")?;

    let mut external = Vec::<ExternalDataAuthorityDescriptor>::new();

    for (authority, location) in external_descriptors {
        external.push(load_external_data_authority_descriptor(
            &location, &authority,
        )?);
    }

    let plan = compose_materialized_boundary_execution_plan(
        &manifest,
        &world,
        &platform,
        &external,
        &std::collections::BTreeMap::new(),
        true,
        true,
        true,
        Some(Path::new("/tmp")),
    )?;

    println!("WORLD :: {}", world);

    println!("BOUNDARY ROOT :: {}", plan.boundary.root.display());

    println!("PROVIDER :: {}", plan.boundary.provider.display());

    println!(
        "INTERPRETER INSIDE :: {}",
        plan.interpreter_inside.display()
    );

    println!("EXECUTABLE INSIDE :: {}", plan.executable_inside.display());

    println!("LIBRARY PATHS INSIDE :: {:?}", plan.library_paths_inside);

    println!("EXTERNAL GRANTS :: {}", plan.boundary.readonly_data.len());

    let mut command =
        build_pure_materialized_boundary_execution_command(&plan, &program_arguments)?;

    let status = command
        .status()
        .map_err(|error| format!("could not execute boundary provider: {error}"))?;

    Ok(status.code().unwrap_or(125))
}

fn main() -> ExitCode {
    match main_result() {
        Ok(code) => {
            if code == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(u8::try_from(code).unwrap_or(125))
            }
        }

        Err(error) => {
            eprintln!("FATAL :: {error}");
            ExitCode::from(125)
        }
    }
}
