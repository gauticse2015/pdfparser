//! pdfparser CLI — text + Phase U tables.
use clap::{Parser, Subcommand, ValueEnum};
use pdfparser::{Document, TableOptions, TablePreset, TextOptions};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "pdfparser", version, about = "Native PDF parser CLI")]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Extract text (and optional tables) from a PDF
    Extract {
        /// Input PDF path
        path: PathBuf,
        /// Output format
        #[arg(long, default_value = "text")]
        format: OutFormat,
        /// Disable reading-order sort
        #[arg(long)]
        paint_order: bool,
        /// Do not apply page /Rotate
        #[arg(long)]
        no_rotate: bool,
        /// Enable table detection (Phase U lattice)
        #[arg(long)]
        tables: bool,
        /// Page range 1-based inclusive, e.g. 1-3 or 2
        #[arg(long)]
        pages: Option<String>,
    },
    /// Show document info
    Info { path: PathBuf },
}

#[derive(Clone, Debug, ValueEnum)]
enum OutFormat {
    Text,
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.cmd {
        Commands::Extract {
            path,
            format,
            paint_order,
            no_rotate,
            tables,
            pages,
        } => match run_extract(path, format, paint_order, no_rotate, tables, pages) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(2)
            }
        },
        Commands::Info { path } => match run_info(path) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(2)
            }
        },
    }
}

fn run_extract(
    path: PathBuf,
    format: OutFormat,
    paint_order: bool,
    no_rotate: bool,
    tables: bool,
    pages: Option<String>,
) -> Result<(), String> {
    let t0 = Instant::now();
    let doc = Document::open(&path).map_err(|e| e.to_string())?;
    let text_opts = TextOptions {
        sort_reading_order: !paint_order,
        insert_spaces: true,
        apply_page_rotate: !no_rotate,
        include_invisible: true,
    };
    let table_opts = if tables {
        TableOptions::from_preset(TablePreset::LatticeOnly)
    } else {
        TableOptions::default()
    };
    let range = parse_pages(pages.as_deref(), doc.page_count())?;

    match format {
        OutFormat::Text => {
            let mut out = String::new();
            for i in range {
                let page = doc.page(i).map_err(|e| e.to_string())?;
                let t = page.text(&text_opts).map_err(|e| e.to_string())?;
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&t);
                if tables {
                    let tabs = page
                        .tables(&text_opts, &table_opts)
                        .map_err(|e| e.to_string())?;
                    for (ti, tab) in tabs.iter().enumerate() {
                        out.push_str(&format!(
                            "\n[table {ti} {}x{} conf={:.2}]\n",
                            tab.rows, tab.cols, tab.confidence
                        ));
                        out.push_str(&render_table_tsv(tab));
                    }
                }
            }
            print!("{out}");
            if !out.ends_with('\n') {
                println!();
            }
        }
        OutFormat::Json => {
            let mut pages_out = Vec::new();
            for i in range {
                let page = doc.page(i).map_err(|e| e.to_string())?;
                let text = page.text(&text_opts).map_err(|e| e.to_string())?;
                let mut page_json = serde_json::json!({
                    "index": i,
                    "rotate": page.rotate(),
                    "text": text,
                });
                if tables {
                    let tabs = page
                        .tables(&text_opts, &table_opts)
                        .map_err(|e| e.to_string())?;
                    page_json["tables"] = serde_json::to_value(&tabs).map_err(|e| e.to_string())?;
                }
                pages_out.push(page_json);
            }
            let v = serde_json::json!({
                "schema_version": pdfparser::SCHEMA_VERSION,
                "page_count": doc.page_count(),
                "version": doc.version(),
                "tables_enabled": tables,
                "pages": pages_out,
                "elapsed_ms": t0.elapsed().as_secs_f64() * 1000.0,
                "library": "pdfparser",
                "library_version": pdfparser::VERSION,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?
            );
        }
    }
    Ok(())
}

fn render_table_tsv(tab: &pdfparser::Table) -> String {
    let mut grid: Vec<Vec<String>> =
        vec![vec![String::new(); tab.cols as usize]; tab.rows as usize];
    for c in &tab.cells {
        let r = c.row as usize;
        let col = c.col as usize;
        if r < grid.len() && col < grid[r].len() {
            grid[r][col] = c.text.replace(['\n', '\t'], " ");
        }
    }
    let mut out = String::new();
    for row in grid {
        out.push_str(&row.join("\t"));
        out.push('\n');
    }
    out
}

fn run_info(path: PathBuf) -> Result<(), String> {
    let doc = Document::open(&path).map_err(|e| e.to_string())?;
    println!("pages: {}", doc.page_count());
    println!("version: {}", doc.version());
    if let Some(t) = doc.info("Title") {
        println!("title: {t}");
    }
    for i in 0..doc.page_count().min(5) {
        let p = doc.page(i).map_err(|e| e.to_string())?;
        println!("page[{i}].rotate = {}", p.rotate());
    }
    Ok(())
}

fn parse_pages(spec: Option<&str>, n: u32) -> Result<Vec<u32>, String> {
    let Some(spec) = spec else {
        return Ok((0..n).collect());
    };
    let mut out = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            let a: u32 = a.parse().map_err(|_| format!("bad page {a}"))?;
            let b: u32 = b.parse().map_err(|_| format!("bad page {b}"))?;
            for p in a..=b {
                if p == 0 || p > n {
                    return Err(format!("page {p} out of range 1..{n}"));
                }
                out.push(p - 1);
            }
        } else {
            let p: u32 = part.parse().map_err(|_| format!("bad page {part}"))?;
            if p == 0 || p > n {
                return Err(format!("page {p} out of range 1..{n}"));
            }
            out.push(p - 1);
        }
    }
    Ok(out)
}
