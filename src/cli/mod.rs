//! CLI module: Spin-compatible command-line interface.
//!
//! Supports:
//! - `spin-rs -a model.pml` — generate verifier (prints Lua code)
//! - `spin-rs -run model.pml` — run verification
//! - `spin-rs -ltl name 'formula' model.pml` — verify LTL property
//! - `spin-rs -help` — show help

use clap::Parser;
use std::path::PathBuf;

use crate::codegen;
use crate::engine::checker::{CheckResult, CheckerBuilder, SearchMode, StorageMode};
use crate::parser;
use crate::property;
use crate::runtime;
use crate::trail::{ErrorTrail, TrailStats};

/// Spin-compatible model checker CLI.
#[derive(Parser, Debug)]
#[command(name = "spin-rs")]
#[command(
    author,
    version,
    about = "A Rust-native Promela model checker with Lua runtime"
)]
#[command(long_about = None)]
pub struct Cli {
    /// Promela model file to verify
    #[arg(required = true)]
    pub model_file: PathBuf,

    /// Generate verifier (print Lua code)
    #[arg(short = 'a', long)]
    pub generate: bool,

    /// Run verification
    #[arg(short = 'r', long = "run")]
    pub run: bool,

    /// LTL property to verify: name 'formula'
    #[arg(long = "ltl", value_names = ["NAME", "FORMULA"])]
    pub ltl_property: Option<Vec<String>>,

    /// Search mode: dfs or bfs
    #[arg(long, default_value = "dfs")]
    pub search: String,

    /// Storage mode: exact, bitstate, or collapse
    #[arg(long, default_value = "exact")]
    pub storage: String,

    /// Maximum number of states to store
    #[arg(long, default_value = "1000000")]
    pub max_states: usize,

    /// Maximum search depth
    #[arg(long, default_value = "100000")]
    pub max_depth: usize,

    /// Enable partial order reduction
    #[arg(long)]
    pub por: bool,

    /// Disable assertion checking
    #[arg(long)]
    pub no_assertions: bool,

    /// Output trail file path
    #[arg(long, default_value = "spin.trail")]
    pub trail_file: String,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

/// Run the CLI with the given arguments.
pub fn run(args: &[String]) -> Result<(), anyhow::Error> {
    let cli = Cli::parse_from(args);

    // Read model file
    let source = std::fs::read_to_string(&cli.model_file).map_err(|e| {
        anyhow::anyhow!(
            "Cannot read model file '{}': {}",
            cli.model_file.display(),
            e
        )
    })?;

    if cli.generate {
        // Generate mode: parse and emit Lua code
        let model = parser::parse(&source)?;
        let generated = codegen::generate(&model);
        println!("{}", generated.source);
        return Ok(());
    }

    if cli.ltl_property.is_some() {
        // LTL verification mode
        let ltl_args = cli.ltl_property.unwrap();
        if ltl_args.len() < 2 {
            anyhow::bail!("LTL property requires name and formula: --ltl name 'formula'");
        }
        let name = &ltl_args[0];
        let formula = &ltl_args[1];

        println!("Verifying LTL property: {} = {}", name, formula);

        let violation = property::verify_ltl(&source, formula, name)?;
        if let Some(v) = violation {
            println!("\n❌ Property violated: {}", v.property_name);
            println!("Description: {}", v.description);
            println!("\nError trail:");
            for (i, step) in v.trail.iter().enumerate() {
                println!("  {:3}: {}", i + 1, step);
            }
        } else {
            println!("✅ Property holds");
        }
        return Ok(());
    }

    // Default: run verification
    if !cli.run && !cli.generate {
        // If neither -a nor -run specified, default to -run
    }

    println!("Verifying model: {}", cli.model_file.display());

    // Parse model
    let model = parser::parse(&source)?;

    // Generate Lua code
    let _generated = codegen::generate(&model);

    // Create runtime and load Lua
    let lua_model = runtime::LuaModel::from_model(&model)?;

    // Configure checker
    let search_mode = match cli.search.as_str() {
        "bfs" => SearchMode::BreadthFirst,
        "dfs" | _ => SearchMode::DepthFirst,
    };

    let storage_mode = match cli.storage.as_str() {
        "bitstate" => StorageMode::Bitstate,
        "collapse" => StorageMode::Collapse,
        "exact" | _ => StorageMode::Exact,
    };

    let mut builder = CheckerBuilder::new()
        .model(lua_model)
        .max_states(cli.max_states)
        .max_depth(cli.max_depth)
        .search_mode(search_mode)
        .storage_mode(storage_mode)
        .check_assertions(!cli.no_assertions);

    if cli.por {
        builder = builder.por_enabled(true);
    }

    let checker = builder.build();

    // Run verification
    let result = checker.check_dfs();

    // Print results
    print_result(&result);

    // Generate trail if violations found
    if result.errors > 0 && !result.violations.is_empty() {
        let violation = &result.violations[0];
        let trail = ErrorTrail::new(
            violation.clone(),
            vec![], // State hashes would be collected during DFS
            result.states_explored,
            result.depth_reached,
        );

        let trail_path = PathBuf::from(&cli.trail_file);
        trail.save_spin_format(&trail_path)?;
        println!("\nTrail saved to: {}", trail_path.display());

        if cli.verbose {
            let stats = TrailStats::compute(&trail);
            stats.print_spin_format();
        }
    }

    Ok(())
}

/// Print verification results in Spin-compatible format.
fn print_result(result: &CheckResult) {
    println!("\n=== Verification Results ===");
    println!("states explored:    {}", result.states_explored);
    println!("states stored:      {}", result.states_stored);
    println!("transitions:        {}", result.transitions);
    println!("depth reached:      {}", result.depth_reached);
    println!("errors:             {}", result.errors);
    println!("elapsed time:       {:.3}s", result.elapsed_secs);

    if result.errors > 0 {
        println!("\n❌ Verification failed with {} error(s)", result.errors);
        for (i, v) in result.violations.iter().take(5).enumerate() {
            println!("\nError {}:", i + 1);
            println!("  Property: {}", v.property_name);
            println!("  Description: {}", v.description);
            println!("  Trail length: {}", v.trail.len());
        }
        if result.violations.len() > 5 {
            println!("\n... and {} more errors", result.violations.len() - 5);
        }
    } else {
        println!("\n✅ Verification successful - no errors found");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_basic() {
        let args = vec!["spin-rs".to_string(), "model.pml".to_string()];
        let cli = Cli::parse_from(args);
        assert_eq!(cli.model_file, PathBuf::from("model.pml"));
        assert!(!cli.generate);
        assert!(!cli.run);
    }

    #[test]
    fn test_cli_parse_generate() {
        let args = vec![
            "spin-rs".to_string(),
            "-a".to_string(),
            "model.pml".to_string(),
        ];
        let cli = Cli::parse_from(args);
        assert!(cli.generate);
    }

    #[test]
    fn test_cli_parse_run() {
        let args = vec![
            "spin-rs".to_string(),
            "--run".to_string(),
            "model.pml".to_string(),
        ];
        let cli = Cli::parse_from(args);
        assert!(cli.run);
    }

    #[test]
    fn test_cli_parse_ltl() {
        let args = vec![
            "spin-rs".to_string(),
            "--ltl".to_string(),
            "p0".to_string(),
            "[]x == 0".to_string(),
            "model.pml".to_string(),
        ];
        let cli = Cli::parse_from(args);
        assert!(cli.ltl_property.is_some());
        let props = cli.ltl_property.unwrap();
        assert_eq!(props[0], "p0");
        assert_eq!(props[1], "[]x == 0");
    }

    #[test]
    fn test_cli_parse_options() {
        let args = vec![
            "spin-rs".to_string(),
            "--search".to_string(),
            "bfs".to_string(),
            "--storage".to_string(),
            "bitstate".to_string(),
            "--max-states".to_string(),
            "50000".to_string(),
            "--max-depth".to_string(),
            "5000".to_string(),
            "--por".to_string(),
            "--no-assertions".to_string(),
            "--trail-file".to_string(),
            "out.trail".to_string(),
            "-v".to_string(),
            "model.pml".to_string(),
        ];
        let cli = Cli::parse_from(args);
        assert_eq!(cli.search, "bfs");
        assert_eq!(cli.storage, "bitstate");
        assert_eq!(cli.max_states, 50000);
        assert_eq!(cli.max_depth, 5000);
        assert!(cli.por);
        assert!(cli.no_assertions);
        assert_eq!(cli.trail_file, "out.trail");
        assert!(cli.verbose);
    }
}
