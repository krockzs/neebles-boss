use neebles_backend::domestic_test::test_category;
use std::env;
use std::path::PathBuf;
use std::process;

fn usage() {
    eprintln!(
        "Usage: neebles-domestic-test \
--contract <file> \
--root <domestic-root> \
--category <String> \
--value <String> \
--target <relative-path> [--target <relative-path> ...] \
[--json]"
    );
}

fn require_next(args: &[String], index: usize, flag: &str) -> String {
    match args.get(index + 1) {
        Some(value) if !value.trim().is_empty() => value.clone(),

        _ => {
            eprintln!("DOMESTIC TEST ERROR :: {flag} requires a value");

            usage();

            process::exit(2);
        }
    }
}

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();

    if args.is_empty() || args.iter().any(|arg| arg == "--help") {
        usage();
        return;
    }

    let mut contract = None::<PathBuf>;

    let mut root = None::<PathBuf>;

    let mut category = None::<String>;

    let mut value = None::<String>;

    let mut targets = Vec::<PathBuf>::new();

    let mut json_output = false;

    let mut index = 0usize;

    while index < args.len() {
        match args[index].as_str() {
            "--contract" => {
                contract = Some(PathBuf::from(require_next(&args, index, "--contract")));

                index += 2;
            }

            "--root" => {
                root = Some(PathBuf::from(require_next(&args, index, "--root")));

                index += 2;
            }

            "--category" => {
                category = Some(require_next(&args, index, "--category"));

                index += 2;
            }

            "--value" => {
                value = Some(require_next(&args, index, "--value"));

                index += 2;
            }

            "--target" => {
                targets.push(PathBuf::from(require_next(&args, index, "--target")));

                index += 2;
            }

            "--json" => {
                json_output = true;
                index += 1;
            }

            other => {
                eprintln!("DOMESTIC TEST ERROR :: unknown argument {other}");

                usage();

                process::exit(2);
            }
        }
    }

    let contract = contract.unwrap_or_else(|| {
        eprintln!("DOMESTIC TEST ERROR :: missing --contract");

        process::exit(2);
    });

    let root = root.unwrap_or_else(|| {
        eprintln!("DOMESTIC TEST ERROR :: missing --root");

        process::exit(2);
    });

    let category = category.unwrap_or_else(|| {
        eprintln!("DOMESTIC TEST ERROR :: missing --category");

        process::exit(2);
    });

    let value = value.unwrap_or_else(|| {
        eprintln!("DOMESTIC TEST ERROR :: missing --value");

        process::exit(2);
    });

    let report = match test_category(&contract, &root, &category, &value, &targets) {
        Ok(report) => report,

        Err(error) => {
            eprintln!("DOMESTIC TEST ERROR :: {error}");

            process::exit(2);
        }
    };

    if json_output {
        match serde_json::to_string_pretty(&report) {
            Ok(encoded) => {
                println!("{encoded}");
            }

            Err(error) => {
                eprintln!("DOMESTIC TEST ERROR :: could not serialize report: {error}");

                process::exit(2);
            }
        }
    } else {
        println!(
            "DOMESTIC TEST :: {}",
            if report.ok { "PASS" } else { "FAIL" }
        );

        println!("ROOT :: {}", report.root);

        println!("CATEGORY :: {}", report.category);

        println!("VALUE :: {}", report.value);

        for target in &report.targets {
            println!(
                "TARGET :: {} -> {} :: {}",
                target.declared_target,
                target.resolved_target,
                if target.exists { "PASS" } else { "FAIL" }
            );
        }
    }

    if report.ok {
        process::exit(0);
    }

    process::exit(1);
}
