use super::*;
use std::io::Write;
use std::path::PathBuf;

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
fn test_cli_parse_all_branches() {
    let args = vec![
        "spin-rs".to_string(),
        "--search".to_string(),
        "bfs".to_string(),
        "model.pml".to_string(),
    ];
    let cli = Cli::parse_from(args);
    assert_eq!(cli.search, "bfs");

    let args = vec![
        "spin-rs".to_string(),
        "--search".to_string(),
        "dfs".to_string(),
        "model.pml".to_string(),
    ];
    let cli = Cli::parse_from(args);
    assert_eq!(cli.search, "dfs");

    let args = vec![
        "spin-rs".to_string(),
        "--search".to_string(),
        "default".to_string(),
        "model.pml".to_string(),
    ];
    let cli = Cli::parse_from(args);
    assert_eq!(cli.search, "default");

    let args = vec![
        "spin-rs".to_string(),
        "--storage".to_string(),
        "exact".to_string(),
        "model.pml".to_string(),
    ];
    let cli = Cli::parse_from(args);
    assert_eq!(cli.storage, "exact");

    let args = vec![
        "spin-rs".to_string(),
        "--storage".to_string(),
        "bitstate".to_string(),
        "model.pml".to_string(),
    ];
    let cli = Cli::parse_from(args);
    assert_eq!(cli.storage, "bitstate");

    let args = vec![
        "spin-rs".to_string(),
        "--storage".to_string(),
        "collapse".to_string(),
        "model.pml".to_string(),
    ];
    let cli = Cli::parse_from(args);
    assert_eq!(cli.storage, "collapse");

    let args = vec![
        "spin-rs".to_string(),
        "--storage".to_string(),
        "unknown".to_string(),
        "model.pml".to_string(),
    ];
    let cli = Cli::parse_from(args);
    assert_eq!(cli.storage, "unknown");

    let args = vec![
        "spin-rs".to_string(),
        "--por".to_string(),
        "model.pml".to_string(),
    ];
    let cli = Cli::parse_from(args);
    assert!(cli.por);

    let args = vec![
        "spin-rs".to_string(),
        "-v".to_string(),
        "model.pml".to_string(),
    ];
    let cli = Cli::parse_from(args);
    assert!(cli.verbose);

    let args = vec![
        "spin-rs".to_string(),
        "--no-assertions".to_string(),
        "model.pml".to_string(),
    ];
    let cli = Cli::parse_from(args);
    assert!(cli.no_assertions);

    let args = vec![
        "spin-rs".to_string(),
        "--trail-file".to_string(),
        "custom.trail".to_string(),
        "model.pml".to_string(),
    ];
    let cli = Cli::parse_from(args);
    assert_eq!(cli.trail_file, "custom.trail");
}

#[test]
fn test_cli_run_nonexistent_file() {
    let args = vec!["spin-rs".to_string(), "_.nonexistent_.pml".to_string()];
    let result = run(&args);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Cannot read model file"));
}

#[test]
fn test_cli_generate_output() {
    let dir = std::env::temp_dir();
    let file_path = dir.join("test_generate.pml");
    let mut f = std::fs::File::create(&file_path).unwrap();
    write!(f, "active proctype P() {{ byte x; x = 1; }}").unwrap();
    drop(f);

    let args = vec![
        "spin-rs".to_string(),
        "-a".to_string(),
        file_path.to_string_lossy().to_string(),
    ];
    let result = run(&args);
    assert!(result.is_ok());
    std::fs::remove_file(&file_path).ok();
}

#[test]
fn test_cli_verify_simple() {
    let dir = std::env::temp_dir();
    let file_path = dir.join("test_verify.pml");
    let mut f = std::fs::File::create(&file_path).unwrap();
    write!(f, "active proctype P() {{ byte x; x = 1; }}").unwrap();
    drop(f);

    let args = vec![
        "spin-rs".to_string(),
        file_path.to_string_lossy().to_string(),
    ];
    let result = run(&args);
    assert!(result.is_ok());
    std::fs::remove_file(&file_path).ok();
}

#[test]
fn test_cli_run_with_options() {
    let dir = std::env::temp_dir();
    let file_path = dir.join("test_opts.pml");
    let mut f = std::fs::File::create(&file_path).unwrap();
    write!(f, "active proctype P() {{ byte x; x = 1; }}").unwrap();
    drop(f);

    let args = vec![
        "spin-rs".to_string(),
        "--search".to_string(),
        "bfs".to_string(),
        "--storage".to_string(),
        "bitstate".to_string(),
        "--max-states".to_string(),
        "100".to_string(),
        "--max-depth".to_string(),
        "50".to_string(),
        file_path.to_string_lossy().to_string(),
    ];
    let result = run(&args);
    assert!(result.is_ok());
    std::fs::remove_file(&file_path).ok();
}

#[test]
fn test_cli_ltl_error() {
    let args = vec![
        "spin-rs".to_string(),
        "--ltl".to_string(),
        "p0".to_string(),
        "[](x == 1)".to_string(),
        "_.nonexistent_.pml".to_string(),
    ];
    let result = run(&args);
    assert!(result.is_err());
}

#[test]
fn test_cli_with_por() {
    let dir = std::env::temp_dir();
    let file_path = dir.join("test_por.pml");
    let mut f = std::fs::File::create(&file_path).unwrap();
    write!(f, "active proctype P() {{ byte x; x = 1; }}").unwrap();
    drop(f);

    let args = vec![
        "spin-rs".to_string(),
        "--por".to_string(),
        file_path.to_string_lossy().to_string(),
    ];
    let result = run(&args);
    assert!(result.is_ok());
    std::fs::remove_file(&file_path).ok();
}

#[test]
fn test_cli_run_with_no_assertions() {
    let dir = std::env::temp_dir();
    let file_path = dir.join("test_no_assert.pml");
    let mut f = std::fs::File::create(&file_path).unwrap();
    write!(f, "active proctype P() {{ assert(false); }}").unwrap();
    drop(f);

    let args = vec![
        "spin-rs".to_string(),
        "--no-assertions".to_string(),
        file_path.to_string_lossy().to_string(),
    ];
    let result = run(&args);
    assert!(result.is_ok());
    std::fs::remove_file(&file_path).ok();
}

#[test]
fn test_print_result_error() {
    let result = CheckResult {
        states_explored: 100,
        states_stored: 50,
        transitions: 200,
        depth_reached: 10,
        errors: 1,
        elapsed_secs: 0.5,
        violations: vec![crate::engine::checker::Violation {
            property_name: "safety".to_string(),
            description: "assertion failed".to_string(),
            trail: vec!["state 1".to_string(), "state 2".to_string()],
        }],
    };
    print_result(&result);
}

#[test]
fn test_print_result_success() {
    let result = CheckResult {
        states_explored: 10,
        states_stored: 5,
        transitions: 20,
        depth_reached: 3,
        errors: 0,
        elapsed_secs: 0.1,
        violations: vec![],
    };
    print_result(&result);
}

#[test]
fn test_print_result_many_errors() {
    let mut violations = Vec::new();
    for i in 0..10 {
        violations.push(crate::engine::checker::Violation {
            property_name: format!("p{}", i),
            description: "error".to_string(),
            trail: vec!["state 1".to_string()],
        });
    }
    let result = CheckResult {
        states_explored: 100,
        states_stored: 50,
        transitions: 200,
        depth_reached: 10,
        errors: 10,
        elapsed_secs: 0.5,
        violations,
    };
    print_result(&result);
}

#[test]
fn test_cli_options() {
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
