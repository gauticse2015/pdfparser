//! P1.7: optional `--use-export-ir` uses pdfparser-export for text JSON only.
use std::path::PathBuf;
use std::process::Command;

fn pdfparser_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pdfparser"))
}

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmark/corpus")
        .join(name)
}

fn extract_json(args: &[&str]) -> serde_json::Value {
    let pdf = corpus("01_simple_text.pdf");
    let mut cmd = pdfparser_bin();
    cmd.arg("extract").args(args).arg(&pdf);
    let out = cmd.output().expect("run pdfparser");
    assert!(
        out.status.success(),
        "cli failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("json stdout")
}

#[test]
fn use_export_ir_emits_extracted_document() {
    let v = extract_json(&["--format", "json", "--use-export-ir"]);
    assert_eq!(v["schema_version"], 1);
    assert!(v.get("metadata").is_some(), "IR has metadata");
    assert!(v.get("pages").is_some());
    let text = v["pages"][0]["text"].as_str().unwrap_or("");
    assert!(
        text.contains("SIMPLE_TOKEN_ALPHA"),
        "missing token in {text}"
    );
    assert!(v["pages"][0].get("elements").is_some());
    // Product table/object envelope must not appear on this path.
    assert!(v.get("tables_enabled").is_none());
    assert!(v.get("table_count").is_none());
    assert!(v.get("library").is_none());
}

#[test]
fn default_json_schema_unchanged_without_flag() {
    let v = extract_json(&["--format", "json"]);
    assert_eq!(v["tables_enabled"], false);
    assert!(v.get("library").is_some());
    assert!(v.get("images").is_some());
    assert!(v.get("metadata").is_none());
}

#[test]
fn tables_json_ignores_use_export_ir() {
    let v = extract_json(&["--format", "json", "--tables", "--use-export-ir"]);
    assert_eq!(v["tables_enabled"], true);
    assert!(v.get("tables").is_some());
    assert!(v.get("table_count").is_some());
    assert!(v.get("library").is_some());
    assert!(v.get("metadata").is_none());
}
