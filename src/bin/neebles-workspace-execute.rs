use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use neebles_backend::domestic_authority_supply::build_authority_grant_set;
use neebles_backend::domestic_platform_control::{
    load_platform_controlled_authority_supply, register_platform_controlled_authorities,
};
use neebles_backend::domestic_workspace_execution::{
    build_workspace_execution_command, WorkspaceExecutionRequest, WorkspaceReadonlyGrant,
    WorkspaceWritableGrant,
};

#[derive(Debug)]
struct WritableInput {
    authority: String,
    source: PathBuf,
    destination: PathBuf,
}

fn require_value(args: &[OsString], index: &mut usize, option: &str) -> Result<OsString, String> {
    *index += 1;

    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn require_text(args: &[OsString], index: &mut usize, option: &str) -> Result<String, String> {
    let value = require_value(args, index, option)?
        .to_string_lossy()
        .to_string();

    if value.trim().is_empty() {
        return Err(format!("{option} value cannot be empty"));
    }

    Ok(value)
}

fn parse_writable(value: &str) -> Result<WritableInput, String> {
    let fields = value.splitn(3, '=').collect::<Vec<_>>();

    if fields.len() != 3 {
        return Err("--writable must use authority=source=destination".to_string());
    }

    if fields.iter().any(|field| field.trim().is_empty()) {
        return Err("--writable fields cannot be empty".to_string());
    }

    Ok(WritableInput {
        authority: fields[0].to_string(),
        source: PathBuf::from(fields[1]),
        destination: PathBuf::from(fields[2]),
    })
}

fn main_result() -> Result<i32, String> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();

    let mut authority_supply_path = None::<PathBuf>;
    let mut manifest_path = None::<PathBuf>;
    let mut world = None::<String>;
    let mut readonly_authorities = Vec::<String>::new();
    let mut writable_inputs = Vec::<WritableInput>::new();
    let mut chdir = None::<PathBuf>;
    let mut mount_proc = false;
    let mut mount_dev = false;
    let mut mount_tmp = false;
    let mut arguments = Vec::<OsString>::new();

    let mut index = 0usize;

    while index < args.len() {
        let current = args[index].to_string_lossy().to_string();

        match current.as_str() {
            "--authority-supply" => {
                authority_supply_path = Some(PathBuf::from(require_value(
                    &args,
                    &mut index,
                    "--authority-supply",
                )?));
            }

            "--manifest" => {
                manifest_path = Some(PathBuf::from(require_value(
                    &args,
                    &mut index,
                    "--manifest",
                )?));
            }

            "--world" => {
                world = Some(require_text(&args, &mut index, "--world")?);
            }

            "--readonly" => {
                readonly_authorities.push(require_text(&args, &mut index, "--readonly")?);
            }

            "--writable" => {
                let value = require_text(&args, &mut index, "--writable")?;

                writable_inputs.push(parse_writable(&value)?);
            }

            "--chdir" => {
                chdir = Some(PathBuf::from(require_value(&args, &mut index, "--chdir")?));
            }

            "--proc" => {
                mount_proc = true;
            }

            "--dev" => {
                mount_dev = true;
            }

            "--tmp" => {
                mount_tmp = true;
            }

            "--" => {
                arguments.extend(args.iter().skip(index + 1).cloned());

                break;
            }

            _ => {
                return Err(format!("unknown argument: {current}"));
            }
        }

        index += 1;
    }

    let authority_supply_path =
        authority_supply_path.ok_or_else(|| "missing --authority-supply".to_string())?;

    let supplied = load_platform_controlled_authority_supply(&authority_supply_path)?;

    let registry = register_platform_controlled_authorities(&supplied)?;

    let mut requested = vec!["platform.filesystem_boundary".to_string()];

    requested.extend(readonly_authorities.iter().cloned());

    requested.extend(writable_inputs.iter().map(|input| input.authority.clone()));

    let grants = build_authority_grant_set(&registry, &requested)?;

    let platform_descriptor_path =
        grants.descriptor_path(&registry, "platform.filesystem_boundary")?;

    let readonly = readonly_authorities
        .into_iter()
        .map(|authority| {
            let descriptor_path = grants.descriptor_path(&registry, &authority)?;

            Ok(WorkspaceReadonlyGrant {
                authority,
                descriptor_path,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let writable = writable_inputs
        .into_iter()
        .map(|input| {
            let descriptor_path = grants.descriptor_path(&registry, &input.authority)?;

            Ok(WorkspaceWritableGrant {
                authority: input.authority,
                descriptor_path,
                source: input.source,
                destination: input.destination,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let request = WorkspaceExecutionRequest {
        manifest_path: manifest_path.ok_or_else(|| "missing --manifest".to_string())?,
        world: world.ok_or_else(|| "missing --world".to_string())?,
        platform_descriptor_path,
        readonly,
        writable,
        mount_proc,
        mount_dev,
        mount_tmp,
        chdir,
        arguments,
    };

    let status = build_workspace_execution_command(&request)?
        .status()
        .map_err(|error| format!("could not execute domestic workspace: {error}"))?;

    Ok(status.code().unwrap_or(125))
}

fn main() -> ExitCode {
    match main_result() {
        Ok(0) => ExitCode::SUCCESS,

        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(125)),

        Err(error) => {
            eprintln!("FATAL :: {error}");
            ExitCode::from(125)
        }
    }
}
