//! Kompilator treści: walidacja plików `content/` i generowanie `engine-content`
//! (`docs/03-tresc-gry.md`, rozdz. 8).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use content_compiler::codegen::{self, GENERATED};
use content_compiler::output::{self, hex};
use content_compiler::{Compiled, build, curves};

const USAGE: &str = "Użycie:
  content-compiler check   sprawdza treść i to, czy engine-content jest aktualny
  content-compiler build   sprawdza treść, generuje engine-content i pliki klienta (target/content)
  content-compiler curves [--weights AF,OF,AM,OM] [--combat 17] [--stamina 70] [--hp 75]
                          [--school poziom:szkoła=poziom_szkoły]...
                           wypisuje tabelę rozwoju postaci (poziomy 2–20) do wklejenia w avatars/*.toml";

/// Katalog główny repozytorium.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|command| command == "curves") {
        return curves(&args[1..]);
    }
    let command = match args.as_slice() {
        [command] if command == "check" || command == "build" => command.as_str(),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let root = root();
    let compiled = match build(&root.join("content")) {
        Ok(compiled) => compiled,
        Err(errors) => {
            for error in &errors {
                eprintln!("content/{error}");
            }
            eprintln!("Treść ma błędy: {}.", errors.len());
            return ExitCode::FAILURE;
        }
    };
    let result = if command == "check" {
        check(&compiled, &root)
    } else {
        write(&compiled, &root)
    };
    match result {
        Ok(()) => {
            println!(
                "Treść poprawna: {} kart, {} szablonów + {} dróg, {} zestawów startowych; content_version {}.",
                compiled.cards.len(),
                compiled
                    .avatars
                    .iter()
                    .filter(|a| a.src.path.is_none())
                    .count(),
                compiled
                    .avatars
                    .iter()
                    .filter(|a| a.src.path.is_some())
                    .count(),
                compiled.starters.len(),
                hex(&output::version(&compiled))
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("content-compiler: {error}");
            ExitCode::FAILURE
        }
    }
}

fn curves(args: &[String]) -> ExitCode {
    match curves::parse_args(args) {
        Ok(spec) => {
            let growth = curves::generate(&spec);
            let (combat, stamina, hp) = curves::totals(&growth);
            print!("{}", curves::to_toml(&growth));
            eprintln!("Przyrosty do poziomu 20: walka +{combat}, W +{stamina}, Ż +{hp}.");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "content-compiler curves: {error}

{USAGE}"
            );
            ExitCode::FAILURE
        }
    }
}

fn check(compiled: &Compiled, root: &Path) -> Result<(), String> {
    let expected = codegen::generate(compiled)?;
    let current = fs::read_to_string(root.join(GENERATED)).unwrap_or_default();
    // Porównanie bez względu na końce linii (Git na Windows może je zamienić).
    if current.replace("\r\n", "\n") == expected {
        Ok(())
    } else {
        Err(format!(
            "{GENERATED} jest nieaktualny; uruchom `cargo run -p content-compiler -- build`"
        ))
    }
}

fn write(compiled: &Compiled, root: &Path) -> Result<(), String> {
    let code = codegen::generate(compiled)?;
    fs::write(root.join(GENERATED), code).map_err(|error| format!("{GENERATED}: {error}"))?;
    let out = root.join("target").join("content");
    output::write_client_files(compiled, &out)
        .map_err(|error| format!("{}: {error}", out.display()))?;
    println!("Zapisano {GENERATED} i pliki klienta w target/content.");
    Ok(())
}
