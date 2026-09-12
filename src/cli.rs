use crate::config;
use crate::dispatcher;
use crate::languages;
use crate::modules;
use crate::notifications::{self, Severity};
use crate::privileges;
use crate::request::{ExecutionContext, ExecutionRequest};
use serde_json::Value;

pub fn run(args: Vec<String>) -> i32 {
    if args.is_empty() {
        return result(dispatcher::launch_ui());
    }

    match args[0].as_str() {
        "--version" | "-V" => {
            println!("N.E.E.B.L.E.S. Boss {}", crate::VERSION);
            0
        }
        "--help" | "-h" => {
            print_help();
            0
        }
        "--request-json" => request_json(&args),
        "start" => result(dispatcher::launch_ui()),
        "config" => config_command(&args[1..]),
        "i18n" => i18n_command(&args[1..]),
        "modules" => modules_command(&args[1..]),
        "notify" => notify_command(&args[1..]),
        target => module_command(target, &args[1..]),
    }
}

fn request_json(args: &[String]) -> i32 {
    let Some(raw) = args.get(1) else {
        eprintln!("N.E.E.B.L.E.S.: --request-json requires a JSON request");
        return 2;
    };

    let request: ExecutionRequest = match serde_json::from_str(raw) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("N.E.E.B.L.E.S.: invalid ExecutionRequest JSON: {error}");
            return 2;
        }
    };

    let response = dispatcher::dispatch(request);
    match serde_json::to_string(&response) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("N.E.E.B.L.E.S.: could not serialize response: {error}");
            return 1;
        }
    }
    response.code
}

fn config_command(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("show") => match config::load_or_initialize() {
            Ok(config) => print_json(&config),
            Err(error) => fail(error),
        },
        Some("set") => {
            let Some(key) = args.get(1) else {
                return fail("config set requires a key".to_string());
            };
            let Some(value) = args.get(2) else {
                return fail("config set requires a value".to_string());
            };
            let response = match key.as_str() {
                "language" => config::set_language(value),
                "tray_enabled" | "launcher_enabled" | "normal_notifications" => {
                    match parse_bool(value) {
                        Ok(value) => config::set_bool(key, value),
                        Err(error) => return fail(error),
                    }
                }
                _ => return fail(format!("unknown Boss config key: {key}")),
            };
            match response {
                Ok(config) => print_json(&config),
                Err(error) => fail(error),
            }
        }
        Some("module-update-notified") => {
            let Some(name) = args.get(1) else {
                return fail(
                    "config module-update-notified requires a module name"
                        .to_string()
                );
            };

            let Some(version) = args.get(2) else {
                return fail(
                    "config module-update-notified requires a version"
                        .to_string()
                );
            };

            match config::mark_module_update_notified(
                name,
                version,
            ) {
                Ok(config) => print_json(&config),
                Err(error) => fail(error),
            }
        }
        _ => fail(
            "usage: neebles config show | neebles config set <key> <value> | neebles config module-update-notified <module> <version>"
                .to_string()
        ),
    }
}

fn i18n_command(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("languages") => match languages::load_manifest() {
            Ok(manifest) => print_json(&manifest),
            Err(error) => fail(error),
        },
        Some("dump") => {
            let language = match config::load_or_initialize() {
                Ok(config) => config.language,
                Err(error) => return fail(error),
            };
            match languages::load_strings(&language) {
                Ok(strings) => print_json(&strings),
                Err(error) => fail(error),
            }
        }
        Some("text") => {
            let Some(key) = args.get(1) else {
                return fail("i18n text requires a key".to_string());
            };
            let language = match config::load_or_initialize() {
                Ok(config) => config.language,
                Err(error) => return fail(error),
            };
            match languages::load_strings(&language) {
                Ok(strings) => {
                    println!("{}", strings.get(key).cloned().unwrap_or_else(|| key.to_string()));
                    0
                }
                Err(error) => fail(error),
            }
        }
        _ => fail("usage: neebles i18n languages | dump | text <key>".to_string()),
    }
}

fn modules_command(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("available") | Some("list") => match modules::available_modules_json() {
            Ok(value) => print_value(value),
            Err(error) => fail(error),
        },
        Some("installed") => match modules::installed_modules_json() {
            Ok(value) => print_value(value),
            Err(error) => fail(error),
        },
        Some("install") => {
            let Some(name) = args.get(1) else {
                return fail("modules install requires a module name".to_string());
            };
            if let Err(error) = privileges::ensure_root(true) {
                return fail(error);
            }
            result(modules::install(name))
        }
        Some("update") => {
            let Some(name) = args.get(1) else {
                return fail("modules update requires a module name".to_string());
            };
            if let Err(error) = privileges::ensure_root(true) {
                return fail(error);
            }
            result(modules::update(name))
        }
        Some("uninstall") => {
            let Some(name) = args.get(1) else {
                return fail("modules uninstall requires a module name".to_string());
            };
            if let Err(error) = privileges::ensure_root(true) {
                return fail(error);
            }
            result(modules::uninstall(name))
        }
        Some("enable") => {
            let Some(name) = args.get(1) else {
                return fail("modules enable requires a module name".to_string());
            };
            result(modules::set_enabled(name, true))
        }
        Some("disable") => {
            let Some(name) = args.get(1) else {
                return fail("modules disable requires a module name".to_string());
            };
            result(modules::set_enabled(name, false))
        }
        _ => fail(
            "usage: neebles modules available|installed|install|update|uninstall|enable|disable".to_string(),
        ),
    }
}

fn notify_command(args: &[String]) -> i32 {
    let Some(severity) = args.first() else {
        return fail("notify requires a severity".to_string());
    };
    let title = args.get(1).cloned().unwrap_or_else(|| "N.E.E.B.L.E.S.".to_string());
    let message = if args.len() > 2 {
        args[2..].join(" ")
    } else {
        String::new()
    };
    match Severity::parse(severity).and_then(|severity| notifications::emit(severity, &title, &message)) {
        Ok(()) => 0,
        Err(error) => fail(error),
    }
}

fn module_command(target: &str, args: &[String]) -> i32 {
    let action = args.first().cloned();
    let rest = if args.is_empty() { Vec::new() } else { args[1..].to_vec() };
    let request = ExecutionRequest {
        target: target.to_string(),
        action,
        args: rest,
        context: ExecutionContext {
            caller: "cli".to_string(),
        },
    };

    let response = dispatcher::dispatch(request);
    if !response.ok {
        if let Some(error) = response.error {
            eprintln!("N.E.E.B.L.E.S.: {}", error.message);
        }
    }
    response.code
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(format!("invalid boolean value: {value}")),
    }
}

fn print_json<T: serde::Serialize>(value: &T) -> i32 {
    match serde_json::to_string(value) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(error) => fail(format!("could not serialize JSON: {error}")),
    }
}

fn print_value(value: Value) -> i32 {
    print_json(&value)
}

fn result(value: Result<(), String>) -> i32 {
    match value {
        Ok(()) => 0,
        Err(error) => fail(error),
    }
}

fn fail(error: String) -> i32 {
    eprintln!("N.E.E.B.L.E.S.: {error}");
    1
}

fn print_help() {
    println!("N.E.E.B.L.E.S. Boss {}", crate::VERSION);
    println!();
    println!("Usage:");
    println!("  neebles");
    println!("  neebles start");
    println!("  neebles --version");
    println!("  neebles --help");
    println!("  neebles <module> [arguments...]");
    println!();
    println!("Boss administration:");
    println!("  neebles config show");
    println!("  neebles config set <key> <value>");
    println!("  neebles config module-update-notified <module> <version>");
    println!("  neebles modules available|installed");
    println!("  neebles modules install|update|uninstall <module>");
    println!("  neebles modules enable|disable <module>");
    println!("  neebles notify <info|success|warning|critical|fatal> <title> <message>");
    println!("  neebles --request-json '<ExecutionRequest JSON>'");
}
