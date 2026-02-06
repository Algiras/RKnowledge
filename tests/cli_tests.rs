//! End-to-end CLI tests using `assert_cmd`.
//!
//! These tests invoke the actual compiled binary and verify exit codes
//! and output. They do NOT require Neo4j or an LLM to be running
//! (except tests marked #[ignore]).

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn cmd() -> Command {
    Command::cargo_bin("rknowledge").unwrap()
}

// ─── Help / version ─────────────────────────────────────────────────────

#[test]
fn test_help_shows_commands() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("build"))
        .stdout(predicate::str::contains("query"))
        .stdout(predicate::str::contains("export"))
        .stdout(predicate::str::contains("viz"))
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("auth"));
}

#[test]
fn test_version_shows_semver() {
    cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("rknowledge"));
}

// ─── Build subcommand argument validation ───────────────────────────────

#[test]
fn test_build_help() {
    cmd()
        .args(["build", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PATH"))
        .stdout(predicate::str::contains("--provider"))
        .stdout(predicate::str::contains("--model"))
        .stdout(predicate::str::contains("--output"));
}

#[test]
fn test_build_requires_path() {
    cmd()
        .arg("build")
        .assert()
        .failure()
        .stderr(predicate::str::contains("PATH"));
}

#[test]
fn test_build_rejects_invalid_provider() {
    cmd()
        .args(["build", "/tmp", "--provider", "invalid_provider"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

#[test]
fn test_build_rejects_invalid_output() {
    cmd()
        .args(["build", "/tmp", "--output", "mongodb"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

// ─── Export subcommand argument validation ───────────────────────────────

#[test]
fn test_export_help() {
    cmd()
        .args(["export", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--format"))
        .stdout(predicate::str::contains("--output"));
}

#[test]
fn test_export_requires_output() {
    cmd()
        .args(["export"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--output"));
}

#[test]
fn test_export_rejects_invalid_format() {
    cmd()
        .args(["export", "--format", "xml", "--output", "/tmp/out"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

// ─── Query subcommand ───────────────────────────────────────────────────

#[test]
fn test_query_help() {
    cmd()
        .args(["query", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("QUERY"));
}

// ─── Auth subcommand ────────────────────────────────────────────────────

#[test]
fn test_auth_list_flag() {
    cmd()
        .args(["auth", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--list"))
        .stdout(predicate::str::contains("--provider"));
}

// ─── Build with JSON output (no Neo4j needed) ──────────────────────────
// These require Ollama running, so we gate them.

#[test]
#[ignore] // Run with: cargo test -- --ignored
fn test_build_json_output_with_ollama() {
    let dir = tempdir().unwrap();
    let input_file = dir.path().join("test.txt");
    fs::write(
        &input_file,
        "Rust is a systems programming language. Tokio is an async runtime for Rust. \
         Cargo is the build system and package manager for Rust.",
    )
    .unwrap();

    cmd()
        .args([
            "build",
            input_file.to_str().unwrap(),
            "--provider",
            "ollama",
            "--model",
            "ministral-3:8b",
            "--output",
            "json",
        ])
        .timeout(std::time::Duration::from_secs(120))
        .assert()
        .success()
        .stdout(predicate::str::contains("Extracting knowledge"));
}

#[test]
#[ignore] // Run with: cargo test -- --ignored
fn test_build_csv_output_with_ollama() {
    let dir = tempdir().unwrap();
    let input_file = dir.path().join("test.txt");
    fs::write(
        &input_file,
        "Machine learning is a subset of artificial intelligence. \
         Deep learning uses neural networks with many layers.",
    )
    .unwrap();

    cmd()
        .args([
            "build",
            input_file.to_str().unwrap(),
            "--provider",
            "ollama",
            "--model",
            "ministral-3:8b",
            "--output",
            "csv",
        ])
        .timeout(std::time::Duration::from_secs(120))
        .assert()
        .success();
}

// ─── Integration: Neo4j query (requires running Neo4j) ──────────────────

#[test]
#[ignore]
fn test_query_with_neo4j() {
    cmd()
        .args(["query", "artificial intelligence"])
        .timeout(std::time::Duration::from_secs(10))
        .assert()
        .success()
        .stdout(predicate::str::contains("Searching"));
}

#[test]
#[ignore]
fn test_cypher_query_with_neo4j() {
    cmd()
        .args(["query", "cypher: MATCH (n:Concept) RETURN count(n) AS count"])
        .timeout(std::time::Duration::from_secs(10))
        .assert()
        .success()
        .stdout(predicate::str::contains("Results"));
}

// ─── Integration: export from Neo4j ─────────────────────────────────────

#[test]
#[ignore]
fn test_export_json_from_neo4j() {
    let dir = tempdir().unwrap();
    let output = dir.path().join("graph.json");

    cmd()
        .args([
            "export",
            "--format",
            "json",
            "--output",
            output.to_str().unwrap(),
        ])
        .timeout(std::time::Duration::from_secs(10))
        .assert()
        .success();

    assert!(output.exists());
    let content = fs::read_to_string(&output).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed["nodes"].is_array());
}

#[test]
#[ignore]
fn test_export_all_formats_from_neo4j() {
    let dir = tempdir().unwrap();

    for format in ["json", "csv", "graphml", "cypher"] {
        let output = dir.path().join(format!("graph.{}", format));
        cmd()
            .args([
                "export",
                "--format",
                format,
                "--output",
                output.to_str().unwrap(),
            ])
            .timeout(std::time::Duration::from_secs(10))
            .assert()
            .success();
    }
}
