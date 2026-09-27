use neebles_backend::domestic_elf::inspect_elf;
use std::path::PathBuf;

fn main() {
    let paths = std::env::args()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();

    if paths.is_empty() {
        eprintln!("usage: neebles-elf-inspect ELF...");

        std::process::exit(2);
    }

    let mut failed = false;

    for path in paths {
        println!(
            "===================================================================================================="
        );

        println!("ELF :: {}", path.display());

        match inspect_elf(&path) {
            Ok(metadata) => {
                println!(
                    "INTERPRETER :: {}",
                    metadata
                        .interpreter
                        .as_ref()
                        .map(|value| value.display().to_string())
                        .unwrap_or_else(|| "NONE".to_string())
                );

                println!("RAW_RPATH");
                if metadata.raw_rpath.is_empty() {
                    println!("  NONE");
                } else {
                    for value in &metadata.raw_rpath {
                        println!("  {:?}", value);
                    }
                }

                println!("RAW_RUNPATH");
                if metadata.raw_runpath.is_empty() {
                    println!("  NONE");
                } else {
                    for value in &metadata.raw_runpath {
                        println!("  {:?}", value);
                    }
                }

                println!("RPATH");

                if metadata.rpath.is_empty() {
                    println!("  NONE");
                } else {
                    for value in &metadata.rpath {
                        println!("  {}", value.display());
                    }
                }

                println!("RUNPATH");

                if metadata.runpath.is_empty() {
                    println!("  NONE");
                } else {
                    for value in &metadata.runpath {
                        println!("  {}", value.display());
                    }
                }

                println!("NEEDED");

                if metadata.needed.is_empty() {
                    println!("  NONE");
                } else {
                    for value in &metadata.needed {
                        println!("  {}", value);
                    }
                }
            }

            Err(error) => {
                println!("ERROR :: {}", error);

                failed = true;
            }
        }

        println!();
    }

    if failed {
        std::process::exit(1);
    }
}
