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
use crate::engine::checker::{CheckResult, CheckerBuilder, Violation};
use crate::engine::checker::{SearchMode, StorageMode};
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
    #[arg(short = 'k', long, default_value = "spin.trail")]
    pub trail_file: String,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Interactive simulation mode
    #[arg(short = 'i', long)]
    pub interactive: bool,

    /// Inspect state during trail replay
    #[arg(long)]
    pub inspect: bool,

    /// Replay a trail file (Spin-compatible format)
    #[arg(short = 't', long)]
    pub trail: bool,

    /// Swarm verification: N workers, M iterations (e.g. --swarm 4,1)
    #[arg(long)]
    pub swarm: Option<String>,

    /// Parallel BFS with N threads (e.g. --bfspar 4)
    #[arg(long)]
    pub bfspar: Option<usize>,

    /// Use hash-compact storage (memory-efficient, 64-bit hashes)
    #[arg(long)]
    pub hc: bool,

    /// Enable strong fairness constraints for liveness verification
    #[arg(long)]
    pub strong_fairness: bool,

    /// Optimization level 2: dead variable elimination
    #[arg(short = 'o', long = "opt2")]
    pub opt2: bool,

    /// Optimization level 3: statement merging
    #[arg(long = "opt3")]
    pub opt3: bool,

    /// Optimization level 4: rendezvous optimization
    #[arg(long = "opt4")]
    pub opt4: bool,
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

    if cli.interactive {
        return handle_interactive_mode(&source);
    }

    if cli.generate {
        return handle_generate_mode(&cli, &source);
    }

    if let Some(ltl_args) = &cli.ltl_property {
        return handle_ltl_mode(&source, ltl_args);
    }

    if cli.trail {
        return handle_trail_replay_mode(&cli, &source);
    }

    if let Some(swarm_arg) = &cli.swarm {
        return handle_swarm_mode(&cli, &source, swarm_arg);
    }

    if let Some(num_threads) = cli.bfspar {
        return handle_parallel_bfs_mode(&cli, &source, num_threads);
    }

    handle_verify_mode(&cli, &source)
}

fn handle_generate_mode(cli: &Cli, source: &str) -> Result<(), anyhow::Error> {
    let model = parser::parse(source)?;
    let opt = build_opt_level(cli);
    let model = if opt != codegen::optimize::OptLevel::none() {
        codegen::optimize::apply_to_model(&model, &opt)
    } else {
        model
    };
    let generated = codegen::generate(&model);
    println!("{}", generated.source);
    Ok(())
}

fn handle_interactive_mode(source: &str) -> Result<(), anyhow::Error> {
    let model = parser::parse(source)?;
    let lua_model = runtime::LuaModel::from_model(&model)?;

    let mut sim = crate::engine::interactive::InteractiveSimulator::new(lua_model);
    sim.run_interactive();
    Ok(())
}

fn handle_trail_replay_mode(cli: &Cli, source: &str) -> Result<(), anyhow::Error> {
    let model = parser::parse(source)?;
    let lua_model = runtime::LuaModel::from_model(&model)?;

    let trail_path = std::path::PathBuf::from(&cli.trail_file);
    if !trail_path.exists() {
        anyhow::bail!("Trail file not found: {}", trail_path.display());
    }

    let trail = crate::trail::ErrorTrail::load_spin_format(&trail_path)?;
    println!(
        "Loaded trail: {} ({} steps)",
        trail.property_name,
        trail.len()
    );

    let replayer = crate::trail::TrailReplayer::new(lua_model, trail);

    if cli.inspect {
        replayer.replay_with_inspect()?;
    } else {
        let states = replayer.replay()?;
        println!("Trail replay complete: {} states visited", states.len());
    }

    Ok(())
}

fn handle_swarm_mode(cli: &Cli, source: &str, swarm_arg: &str) -> Result<(), anyhow::Error> {
    // Parse "N,M" format
    let parts: Vec<&str> = swarm_arg.split(',').collect();
    if parts.is_empty() || parts.len() > 2 {
        anyhow::bail!("Invalid swarm format. Use --swarm N,M where N=workers, M=iterations");
    }
    let num_workers: usize = parts[0]
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid worker count: {}", parts[0]))?;
    let iterations_per_worker: usize = if parts.len() > 1 {
        parts[1]
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid iteration count: {}", parts[1]))?
    } else {
        1
    };

    println!(
        "Swarm verification: {} workers, {} iterations each",
        num_workers, iterations_per_worker
    );

    let _model = parser::parse(source)?;
    let source_clone = source.to_string();

    let config = crate::engine::swarm::SwarmConfig {
        num_workers,
        iterations_per_worker,
        base_max_states: cli.max_states,
        base_max_depth: cli.max_depth,
    };

    let result = crate::engine::swarm::run_swarm(
        move || {
            let model = parser::parse(&source_clone).unwrap();
            runtime::LuaModel::from_model(&model).unwrap()
        },
        &config,
    );
    print_result(&result);

    Ok(())
}

fn handle_parallel_bfs_mode(
    cli: &Cli,
    source: &str,
    num_threads: usize,
) -> Result<(), anyhow::Error> {
    println!("Parallel BFS: {} threads", num_threads);

    let source_clone = source.to_string();

    let config = crate::engine::parallel_bfs::ParallelBfsConfig {
        num_threads,
        max_states: cli.max_states,
        max_depth: cli.max_depth,
        check_assertions: !cli.no_assertions,
        ..Default::default()
    };

    let result = crate::engine::parallel_bfs::run_parallel_bfs(
        move || {
            let model = parser::parse(&source_clone).unwrap();
            runtime::LuaModel::from_model(&model).unwrap()
        },
        &config,
    );
    print_result(&result);

    Ok(())
}

fn handle_ltl_mode(source: &str, ltl_args: &[String]) -> Result<(), anyhow::Error> {
    if ltl_args.len() < 2 {
        anyhow::bail!("LTL property requires name and formula: --ltl name 'formula'");
    }
    let name = &ltl_args[0];
    let formula = &ltl_args[1];

    println!("Verifying LTL property: {} = {}", name, formula);

    let violation = property::verify_ltl(source, formula, name)?;
    if let Some(v) = violation {
        print_ltl_violation(v);
    } else {
        println!("✅ Property holds");
    }
    Ok(())
}

fn print_ltl_violation(v: Violation) {
    println!("\n❌ Property violated: {}", v.property_name);
    println!("Description: {}", v.description);
    println!("\nError trail:");
    for (i, step) in v.trail.iter().enumerate() {
        println!("  {:3}: {}", i + 1, step);
    }
}

fn parse_search_mode(search: &str) -> SearchMode {
    match search {
        "bfs" => SearchMode::BreadthFirst,
        _ => SearchMode::DepthFirst,
    }
}

fn parse_storage_mode(storage: &str) -> StorageMode {
    match storage {
        "bitstate" => StorageMode::Bitstate,
        "collapse" => StorageMode::Collapse,
        "hashcompact" | "hc" => StorageMode::HashCompact,
        _ => StorageMode::Exact,
    }
}

fn build_opt_level(cli: &Cli) -> codegen::optimize::OptLevel {
    codegen::optimize::OptLevel {
        dataflow: cli.opt2 || cli.opt3 || cli.opt4,
        dead_var_elim: cli.opt2,
        stmt_merging: cli.opt3,
        rendezvous: cli.opt4,
    }
}

fn handle_verify_mode(cli: &Cli, source: &str) -> Result<(), anyhow::Error> {
    if !cli.run && !cli.generate {
        // If neither -a nor -run specified, default to -run
    }

    println!("Verifying model: {}", cli.model_file.display());

    // Parse model
    let model = parser::parse(source)?;

    // Apply optimizations if requested
    let opt = build_opt_level(cli);
    let model = if opt != codegen::optimize::OptLevel::none() {
        println!(
            "Applying optimizations: dataflow={}, dve={}, merging={}, rendezvous={}",
            opt.dataflow, opt.dead_var_elim, opt.stmt_merging, opt.rendezvous
        );
        codegen::optimize::apply_to_model(&model, &opt)
    } else {
        model
    };

    // Generate Lua code
    let _generated = codegen::generate(&model);

    // Create runtime and load Lua
    let lua_model = runtime::LuaModel::from_model(&model)?;

    // Configure checker
    let search_mode = parse_search_mode(&cli.search);
    let storage_mode = if cli.hc {
        StorageMode::HashCompact
    } else {
        parse_storage_mode(&cli.storage)
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

    if cli.strong_fairness {
        builder = builder.fairness_mode(crate::engine::fairness::FairnessMode::Strong);
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
mod tests;
