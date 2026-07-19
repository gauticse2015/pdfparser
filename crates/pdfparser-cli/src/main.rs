//! pdfparser CLI — text + Phase V tables.
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
        /// Enable table detection (Phase V full pipeline)
        #[arg(long)]
        tables: bool,
        /// Request full-page gray render for line sensing (pdftoppm/mutool/gs; fail-soft).
        /// Implies `--tables`. Uses HighQuality preset (Engine V2 + full-page render request).
        #[arg(long)]
        tables_hq: bool,
        /// Table preset when tables are enabled (overrides --tables-hq when set).
        #[arg(long, value_enum)]
        table_preset: Option<CliTablePreset>,
        /// Disable multipage table stitch (required for real_structure eval — K27).
        #[arg(long)]
        no_stitch: bool,
        /// Prefer page-local table fragments in root `tables` JSON (eval-friendly).
        /// Default: document logical tables when stitch is on.
        #[arg(long)]
        page_tables: bool,
        /// Page range 1-based inclusive, e.g. 1-3 or 2
        #[arg(long)]
        pages: Option<String>,
        /// Dump table engine diagnostics (engine_path, method_mix, rule counts)
        /// to stderr as JSON when tables are enabled (PR9).
        #[arg(long)]
        dump_evidence: bool,
        /// Force legacy soup NMS router (rollback / A/B). Overrides Auto Engine V2 path.
        #[arg(long)]
        legacy_router: bool,
        /// Override table geometry/densify tuning (`key=value`, repeatable).
        ///
        /// Example: `--table-setting densify_y_skip_numeric_frac=0.10`
        /// See `TableTuning` / `TABLE_TUNING_KEYS` for the full settings dict.
        #[arg(long = "table-setting", value_name = "KEY=VALUE")]
        table_settings: Vec<String>,
    },
    /// Show document info
    Info { path: PathBuf },
}

#[derive(Clone, Debug, ValueEnum)]
enum OutFormat {
    Text,
    Json,
}

/// CLI-facing table presets (subset of [`TablePreset`]).
#[derive(Clone, Debug, ValueEnum)]
enum CliTablePreset {
    Auto,
    EngineV2,
    #[value(name = "high-quality")]
    HighQuality,
    /// Latency path: Engine V2, never full-page render.
    Fast,
    Full,
    #[value(name = "lattice-only")]
    LatticeOnly,
}

impl CliTablePreset {
    fn to_preset(&self) -> TablePreset {
        match self {
            CliTablePreset::Auto => TablePreset::Auto,
            CliTablePreset::EngineV2 => TablePreset::EngineV2,
            CliTablePreset::HighQuality => TablePreset::HighQuality,
            CliTablePreset::Fast => TablePreset::Fast,
            CliTablePreset::Full => TablePreset::Full,
            CliTablePreset::LatticeOnly => TablePreset::LatticeOnly,
        }
    }
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
            tables_hq,
            table_preset,
            no_stitch,
            page_tables,
            pages,
            dump_evidence,
            legacy_router,
            table_settings,
        } => {
            let want_tables = tables
                || tables_hq
                || table_preset.is_some()
                || dump_evidence
                || !table_settings.is_empty();
            match run_extract(
                path,
                format,
                paint_order,
                no_rotate,
                want_tables,
                tables_hq,
                table_preset,
                no_stitch,
                page_tables,
                pages,
                dump_evidence,
                legacy_router,
                table_settings,
            ) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(2)
                }
            }
        }
        Commands::Info { path } => match run_info(path) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(2)
            }
        },
    }
}

#[allow(clippy::too_many_arguments)] // CLI flag surface maps 1:1 to extract options
fn run_extract(
    path: PathBuf,
    format: OutFormat,
    paint_order: bool,
    no_rotate: bool,
    tables: bool,
    tables_hq: bool,
    table_preset: Option<CliTablePreset>,
    no_stitch: bool,
    page_tables: bool,
    pages: Option<String>,
    dump_evidence: bool,
    legacy_router: bool,
    table_settings: Vec<String>,
) -> Result<(), String> {
    let t0 = Instant::now();
    let doc = Document::open(&path).map_err(|e| e.to_string())?;
    let text_opts = TextOptions {
        sort_reading_order: !paint_order,
        insert_spaces: true,
        apply_page_rotate: !no_rotate,
        include_invisible: true,
    };
    let mut table_opts = if let Some(ref p) = table_preset {
        TableOptions::from_preset(p.to_preset())
    } else if tables_hq {
        TableOptions::from_preset(TablePreset::HighQuality)
    } else if tables {
        TableOptions::from_preset(TablePreset::Auto)
    } else {
        TableOptions::default()
    };
    if no_stitch {
        table_opts.stitch_multipage = false;
    }
    if dump_evidence {
        table_opts.shadow_diagnostics = true;
    }
    if legacy_router {
        table_opts.legacy_router = true;
    }
    for s in &table_settings {
        table_opts
            .apply_tuning_kv_string(s)
            .map_err(|e| format!("--table-setting: {e}"))?;
    }
    let preset_name = match table_preset.as_ref() {
        Some(p) => format!("{p:?}"),
        None if tables_hq => "HighQuality".into(),
        None if tables => "Auto".into(),
        None => "Off".into(),
    };
    let range = parse_pages(pages.as_deref(), doc.page_count())?;

    let (page_frags, logical) = if tables {
        doc.tables(&text_opts, &table_opts)
            .map_err(|e| e.to_string())?
    } else {
        (Vec::new(), Vec::new())
    };

    if dump_evidence && tables {
        let mut pages_ev = Vec::new();
        for i in &range {
            // Lightweight evidence: table count/method on this page from extract.
            let tabs = page_frags.get(*i as usize).cloned().unwrap_or_default();
            pages_ev.push(serde_json::json!({
                "page": i,
                "n_tables": tabs.len(),
                "methods": tabs.iter().map(|t| format!("{:?} {}x{}", t.method, t.rows, t.cols)).collect::<Vec<_>>(),
                "notes": tabs.iter().map(|t| t.notes.clone()).collect::<Vec<_>>(),
                "engine_hint": if table_opts.use_engine_v2 && !table_opts.legacy_router { "engine_v2" } else { "legacy" },
            }));
        }
        let ev = serde_json::json!({
            "path": path.display().to_string(),
            "preset": preset_name,
            "use_engine_v2": table_opts.use_engine_v2,
            "legacy_router": table_opts.legacy_router,
            "allow_auto_render": table_opts.allow_auto_render,
            "enable_full_page_render": table_opts.enable_full_page_render,
            "pages": pages_ev,
        });
        eprintln!("{}", serde_json::to_string_pretty(&ev).unwrap_or_default());
    }

    // Eval path: page-local fragments (structure gold is per-page).
    // When `--pages` limits the range, only tables on those pages appear in root.
    let range_set: std::collections::HashSet<u32> = range.iter().copied().collect();
    let root_tables: Vec<_> = if page_tables || no_stitch {
        page_frags
            .iter()
            .flatten()
            .filter(|t| range_set.contains(&t.page))
            .cloned()
            .collect()
    } else {
        logical
            .iter()
            .filter(|t| range_set.contains(&t.page))
            .cloned()
            .collect()
    };

    match format {
        OutFormat::Text => {
            let mut out = String::new();
            for i in &range {
                let page = doc.page(*i).map_err(|e| e.to_string())?;
                let t = page.text(&text_opts).map_err(|e| e.to_string())?;
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&t);
                if tables {
                    let tabs = page_frags.get(*i as usize).cloned().unwrap_or_default();
                    for (ti, tab) in tabs.iter().enumerate() {
                        out.push_str(&format!(
                            "\n[table {ti} {}x{} conf={:.2} method={:?}]\n",
                            tab.rows, tab.cols, tab.confidence, tab.method
                        ));
                        out.push_str(&render_table_tsv(tab));
                    }
                }
            }
            if tables
                && !no_stitch
                && logical.len() != page_frags.iter().map(|p| p.len()).sum::<usize>()
            {
                out.push_str("\n# logical (stitched) tables\n");
                for (ti, tab) in logical.iter().enumerate() {
                    out.push_str(&format!(
                        "\n[logical {ti} {}x{} pages_chain conf={:.2}]\n",
                        tab.rows, tab.cols, tab.confidence
                    ));
                    out.push_str(&render_table_tsv(tab));
                }
            }
            print!("{out}");
            if !out.ends_with('\n') {
                println!();
            }
        }
        OutFormat::Json => {
            let mut pages_out = Vec::new();
            for i in &range {
                let page = doc.page(*i).map_err(|e| e.to_string())?;
                let text = page.text(&text_opts).map_err(|e| e.to_string())?;
                let mut page_json = serde_json::json!({
                    "index": i,
                    "rotate": page.rotate(),
                    "text": text,
                });
                if tables {
                    let tabs = page_frags.get(*i as usize).cloned().unwrap_or_default();
                    page_json["tables"] = serde_json::to_value(&tabs).map_err(|e| e.to_string())?;
                }
                pages_out.push(page_json);
            }
            let objects = doc.objects().map_err(|e| e.to_string())?;
            let image_count = objects.image_count();
            let links: Vec<String> = objects.link_uris();
            let form_fields: Vec<String> = objects.form_field_labels();
            let outline: Vec<String> = objects.outline_titles.clone();
            let images_meta: Vec<serde_json::Value> = objects
                .images
                .iter()
                .map(|im| {
                    serde_json::json!({
                        "name": im.name,
                        "width": im.width,
                        "height": im.height,
                        "page": im.page,
                    })
                })
                .collect();

            let mut v = serde_json::json!({
                "schema_version": pdfparser::SCHEMA_VERSION,
                "page_count": doc.page_count(),
                "version": doc.version(),
                "tables_enabled": tables,
                "table_preset": preset_name,
                "stitch_multipage": table_opts.stitch_multipage,
                "pages": pages_out,
                "elapsed_ms": t0.elapsed().as_secs_f64() * 1000.0,
                "library": "pdfparser",
                "library_version": pdfparser::VERSION,
                "image_count": image_count,
                "images": images_meta,
                "links": links,
                "form_fields": form_fields,
                "outline": outline,
            });
            if tables {
                v["tables"] = serde_json::to_value(&root_tables).map_err(|e| e.to_string())?;
                v["table_count"] = serde_json::json!(root_tables.len());
                v["logical_tables"] = serde_json::to_value(&logical).map_err(|e| e.to_string())?;
                v["page_table_count"] =
                    serde_json::json!(page_frags.iter().map(|p| p.len()).sum::<usize>());
            }
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
            let start: u32 = a
                .trim()
                .parse()
                .map_err(|_| format!("bad page start: {a}"))?;
            let end: u32 = b.trim().parse().map_err(|_| format!("bad page end: {b}"))?;
            if start < 1 || end < start {
                return Err(format!("bad page range: {part}"));
            }
            for p in start..=end {
                if p > n {
                    return Err(format!("page {p} out of range (n={n})"));
                }
                out.push(p - 1);
            }
        } else {
            let p: u32 = part.parse().map_err(|_| format!("bad page: {part}"))?;
            if p < 1 || p > n {
                return Err(format!("page {p} out of range (n={n})"));
            }
            out.push(p - 1);
        }
    }
    if out.is_empty() {
        return Err("empty page range".into());
    }
    Ok(out)
}
