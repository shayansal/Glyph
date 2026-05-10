use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use glyphspace_core::{GlyphPatch, GlyphWorld, PolicyContext};
use glyphspace_personalization::{apply_patch, explain_patch};
use glyphspace_schema::{export_named_schema, validate_patch_json, validate_world_json};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
#[command(name = "glyphspace")]
#[command(about = "Validate, compile, patch, inspect, and snapshot Glyphspace apps.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Validate {
        file: PathBuf,
    },
    Compile {
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Patch {
        world: PathBuf,
        patch: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Explain {
        patch: PathBuf,
    },
    Inspect {
        world: PathBuf,
    },
    ExportSchema {
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Snapshot {
        world: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("warn").init();
    match Cli::parse().command {
        Command::Validate { file } => validate_command(&file),
        Command::Compile { input, out } => {
            let world: GlyphWorld = read_json(&input)?;
            write_json(&out, &world)?;
            println!(
                "compiled {} glyphs to {}",
                world.glyphs.len(),
                out.display()
            );
            Ok(())
        }
        Command::Patch { world, patch, out } => {
            let world: GlyphWorld = read_json(&world)?;
            let patch: GlyphPatch = read_json(&patch)?;
            let updated = apply_patch(&world, &patch, &PolicyContext::demo_user())?;
            write_json(&out, &updated)?;
            println!("patched world written to {}", out.display());
            Ok(())
        }
        Command::Explain { patch } => {
            let patch: GlyphPatch = read_json(&patch)?;
            println!("{}", explain_patch(&patch));
            Ok(())
        }
        Command::Inspect { world } => {
            let world: GlyphWorld = read_json(&world)?;
            println!(
                "{}: {} glyphs, {} edges, {} capabilities, hash {}",
                world.name,
                world.glyphs.len(),
                world.edges.len(),
                world.capabilities.len(),
                world.stable_layout_hash()
            );
            Ok(())
        }
        Command::ExportSchema { out } => export_schema_command(out.as_deref()),
        Command::Snapshot { world, out } => {
            let world: GlyphWorld = read_json(&world)?;
            let snapshot = serde_json::json!({
                "id": world.id,
                "name": world.name,
                "glyph_count": world.glyphs.len(),
                "edge_count": world.edges.len(),
                "layout_hash": world.stable_layout_hash(),
            });
            write_json(&out, &snapshot)?;
            println!("snapshot written to {}", out.display());
            Ok(())
        }
    }
}

fn validate_command(path: &Path) -> Result<()> {
    let value: serde_json::Value = read_json(path)?;
    let validation = if value.get("ops").is_some() {
        validate_patch_json(&value)?
    } else {
        validate_world_json(&value)?
    };
    println!(
        "valid: {}{}",
        validation.valid,
        if validation.warnings.is_empty() {
            String::new()
        } else {
            format!("; warnings: {}", validation.warnings.join(", "))
        }
    );
    Ok(())
}

fn export_schema_command(out: Option<&Path>) -> Result<()> {
    let schemas = [
        ("glyphspace.schema.json", "glyphspace"),
        ("capability.schema.json", "capability"),
        ("patch.schema.json", "patch"),
        ("lens.schema.json", "lens"),
        ("policy.schema.json", "policy"),
    ];
    if let Some(dir) = out {
        fs::create_dir_all(dir)?;
        for (file, name) in schemas {
            write_json(&dir.join(file), &export_named_schema(name)?)?;
        }
        println!("schemas exported to {}", dir.display());
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&export_named_schema("glyphspace")?)?
        );
    }
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(value)? + "\n")
        .with_context(|| format!("write {}", path.display()))
}
