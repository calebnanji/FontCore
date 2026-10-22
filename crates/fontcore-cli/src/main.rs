use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use fontcore::{AddOptions, AddOutcome, FontEntry, Vault};

/// Manage the FontCore font vault shared by the storytold (ArtCraft) suite.
#[derive(Parser)]
#[command(name = "fontcore", version)]
struct Cli {
    /// Vault directory [default: $FONTCORE_HOME or the platform data dir]
    #[arg(long, global = true)]
    vault: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create the vault if it does not exist
    Init,
    /// Print the vault directory
    Path,
    /// Import font files, or every .ttf/.otf/.ttc/.otc under a directory
    Add {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Only expose the font to this app (repeatable)
        #[arg(long = "app")]
        apps: Vec<String>,
        /// Attach a tag (repeatable)
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// License identifier, e.g. OFL-1.1
        #[arg(long)]
        license: Option<String>,
    },
    /// Delete a font from the vault
    Remove { id: String },
    /// List fonts in the vault
    List {
        /// Only fonts that this app would load
        #[arg(long)]
        app: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show the full manifest entry for a font
    Info { id: String },
    /// Let apps load a font again
    Enable { id: String },
    /// Keep a font in the vault but hide it from apps
    Disable { id: String },
    /// Restrict a font to specific apps; pass none to make it global
    Scope { id: String, apps: Vec<String> },
    /// Check files against the manifest
    Verify,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> fontcore::Result<ExitCode> {
    let root = match cli.vault {
        Some(root) => root,
        None => Vault::default_root()?,
    };
    match cli.command {
        Command::Init => {
            let vault = Vault::init(root)?;
            println!("vault ready at {}", vault.root().display());
        }
        Command::Path => println!("{}", root.display()),
        Command::Add {
            paths,
            apps,
            tags,
            license,
        } => {
            let vault = Vault::init(root)?;
            let options = AddOptions {
                apps,
                tags,
                license,
            };
            let mut failed = false;
            for path in collect_fonts(&paths, &mut failed) {
                match vault.add(&path, &options) {
                    Ok(AddOutcome::Added(e)) => println!("added   {}  {}", e.id, describe(&e)),
                    Ok(AddOutcome::AlreadyPresent(e)) => {
                        println!("exists  {}  {}", e.id, describe(&e))
                    }
                    Err(e) => {
                        eprintln!("skipped {}: {e}", path.display());
                        failed = true;
                    }
                }
            }
            if failed {
                return Ok(ExitCode::FAILURE);
            }
        }
        Command::Remove { id } => {
            let e = Vault::open(root)?.remove(&id)?;
            println!("removed {}  {}", e.id, describe(&e));
        }
        Command::List { app, json } => {
            let vault = Vault::open(root)?;
            let fonts: Vec<FontEntry> = match &app {
                Some(app) => vault
                    .fonts_for_app(app)?
                    .fonts
                    .into_iter()
                    .map(|f| f.entry)
                    .collect(),
                None => vault.manifest()?.fonts,
            };
            let text = if json {
                serde_json::to_string_pretty(&fonts).expect("serializable") + "\n"
            } else {
                format_table(&fonts)
            };
            // Ignore write errors so `fontcore list | head` exits quietly.
            let _ = io::stdout().lock().write_all(text.as_bytes());
        }
        Command::Info { id } => {
            let e = Vault::open(root)?.get(&id)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&e).expect("serializable")
            );
        }
        Command::Enable { id } => {
            let e = Vault::open(root)?.set_enabled(&id, true)?;
            println!("enabled  {}  {}", e.id, describe(&e));
        }
        Command::Disable { id } => {
            let e = Vault::open(root)?.set_enabled(&id, false)?;
            println!("disabled {}  {}", e.id, describe(&e));
        }
        Command::Scope { id, apps } => {
            let e = Vault::open(root)?.set_apps(&id, apps)?;
            println!("{}  apps: {}", e.id, apps_label(&e));
        }
        Command::Verify => {
            let issues = Vault::open(root)?.verify()?;
            if issues.is_empty() {
                println!("ok");
            } else {
                for issue in &issues {
                    println!("{issue}");
                }
                return Ok(ExitCode::FAILURE);
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Expands directories into the font files beneath them. Explicit file
/// arguments are passed through so unsupported formats get a clear error.
fn collect_fonts(paths: &[PathBuf], failed: &mut bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in paths {
        if path.is_dir() {
            walk(path, &mut out, failed);
        } else {
            out.push(path.clone());
        }
    }
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>, failed: &mut bool) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("skipped {}: {e}", dir.display());
            *failed = true;
            return;
        }
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            walk(&path, out, failed);
        } else if has_font_extension(&path) {
            out.push(path);
        }
    }
}

fn has_font_extension(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        ["ttf", "otf", "ttc", "otc"]
            .iter()
            .any(|x| e.eq_ignore_ascii_case(x))
    })
}

fn describe(e: &FontEntry) -> String {
    match e.faces.as_slice() {
        [face] => format!("{} {}", face.family, face.subfamily),
        faces => format!("{} ({} faces)", e.family(), faces.len()),
    }
}

fn apps_label(e: &FontEntry) -> String {
    if e.apps.is_empty() {
        "all".into()
    } else {
        e.apps.join(",")
    }
}

fn format_table(fonts: &[FontEntry]) -> String {
    if fonts.is_empty() {
        return "no fonts\n".into();
    }
    let rows: Vec<[String; 5]> = fonts
        .iter()
        .map(|e| {
            [
                e.id.clone(),
                describe(e),
                e.format.extension().into(),
                apps_label(e),
                if e.enabled { "on" } else { "off" }.into(),
            ]
        })
        .collect();
    let header = ["ID", "FONT", "FORMAT", "APPS", "STATE"].map(String::from);
    let mut widths = header.clone().map(|h| h.len());
    for row in &rows {
        for (w, cell) in widths.iter_mut().zip(row) {
            *w = (*w).max(cell.chars().count());
        }
    }
    let mut out = String::new();
    for row in std::iter::once(&header).chain(&rows) {
        let line: Vec<String> = row
            .iter()
            .zip(widths)
            .map(|(cell, w)| format!("{cell:<w$}"))
            .collect();
        out.push_str(line.join("  ").trim_end());
        out.push('\n');
    }
    out
}
