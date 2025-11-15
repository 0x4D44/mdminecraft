//! Metrics Diff Tool
//!
//! Compares two metrics.json files and reports differences.
//! Used for CI regression detection.
//!
//! Exit codes:
//! - 0: All metrics within acceptable ranges
//! - 1: Warnings (5-10% degradation)
//! - 2: Failures (>10% degradation)
//!
//! Usage:
//!   metrics-diff baseline.json current.json
//!   metrics-diff baseline.json current.json --format json
//!   metrics-diff baseline.json current.json --threshold-warning 0.05 --threshold-failure 0.10

use mdminecraft_testkit::MetricsReport;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process;

#[derive(Debug)]
struct Config {
    baseline_path: PathBuf,
    current_path: PathBuf,
    threshold_warning: f64,
    threshold_failure: f64,
    format: OutputFormat,
}

#[derive(Debug, Clone, Copy)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug)]
struct MetricDiff {
    name: String,
    baseline: f64,
    current: f64,
    change_percent: f64,
    status: DiffStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffStatus {
    Pass,       // Within acceptable range or improved
    Warning,    // 5-10% degradation
    Failure,    // >10% degradation
}

impl DiffStatus {
    fn to_emoji(&self) -> &'static str {
        match self {
            DiffStatus::Pass => "✅",
            DiffStatus::Warning => "⚠️",
            DiffStatus::Failure => "❌",
        }
    }

    fn to_string(&self) -> &'static str {
        match self {
            DiffStatus::Pass => "PASS",
            DiffStatus::Warning => "WARN",
            DiffStatus::Failure => "FAIL",
        }
    }
}

fn parse_args() -> Result<Config, String> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        return Err(format!(
            "Usage: {} <baseline.json> <current.json> [options]\n\
             Options:\n\
               --format <text|json>           Output format (default: text)\n\
               --threshold-warning <percent>  Warning threshold (default: 0.05)\n\
               --threshold-failure <percent>  Failure threshold (default: 0.10)",
            args[0]
        ));
    }

    let baseline_path = PathBuf::from(&args[1]);
    let current_path = PathBuf::from(&args[2]);

    let mut threshold_warning = 0.05; // 5%
    let mut threshold_failure = 0.10; // 10%
    let mut format = OutputFormat::Text;

    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--format" => {
                if i + 1 >= args.len() {
                    return Err("--format requires an argument".to_string());
                }
                format = match args[i + 1].as_str() {
                    "text" => OutputFormat::Text,
                    "json" => OutputFormat::Json,
                    _ => return Err(format!("Invalid format: {}", args[i + 1])),
                };
                i += 2;
            }
            "--threshold-warning" => {
                if i + 1 >= args.len() {
                    return Err("--threshold-warning requires an argument".to_string());
                }
                threshold_warning = args[i + 1]
                    .parse::<f64>()
                    .map_err(|e| format!("Invalid warning threshold: {}", e))?;
                i += 2;
            }
            "--threshold-failure" => {
                if i + 1 >= args.len() {
                    return Err("--threshold-failure requires an argument".to_string());
                }
                threshold_failure = args[i + 1]
                    .parse::<f64>()
                    .map_err(|e| format!("Invalid failure threshold: {}", e))?;
                i += 2;
            }
            _ => {
                return Err(format!("Unknown option: {}", args[i]));
            }
        }
    }

    Ok(Config {
        baseline_path,
        current_path,
        threshold_warning,
        threshold_failure,
        format,
    })
}

fn load_metrics(path: &PathBuf) -> Result<MetricsReport, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
}

fn compare_metrics(
    baseline: &MetricsReport,
    current: &MetricsReport,
    config: &Config,
) -> Vec<MetricDiff> {
    let mut diffs = Vec::new();

    // Compare terrain metrics
    if let (Some(b_terrain), Some(c_terrain)) = (&baseline.terrain, &current.terrain) {
        diffs.push(compare_metric(
            "terrain.avg_gen_time_us",
            b_terrain.avg_gen_time_us,
            c_terrain.avg_gen_time_us,
            config,
            true, // lower is better
        ));

        diffs.push(compare_metric(
            "terrain.chunks_per_second",
            b_terrain.chunks_per_second,
            c_terrain.chunks_per_second,
            config,
            false, // higher is better
        ));

        diffs.push(compare_metric(
            "terrain.max_gen_time_us",
            b_terrain.max_gen_time_us as f64,
            c_terrain.max_gen_time_us as f64,
            config,
            true, // lower is better
        ));
    }

    // Compare mob metrics
    if let (Some(b_mobs), Some(c_mobs)) = (&baseline.mobs, &current.mobs) {
        diffs.push(compare_metric(
            "mobs.avg_update_time_us",
            b_mobs.avg_update_time_us,
            c_mobs.avg_update_time_us,
            config,
            true, // lower is better
        ));

        diffs.push(compare_metric(
            "mobs.total_spawned",
            b_mobs.total_spawned as f64,
            c_mobs.total_spawned as f64,
            config,
            false, // equality expected for determinism
        ));
    }

    // Compare item metrics
    if let (Some(b_items), Some(c_items)) = (&baseline.items, &current.items) {
        diffs.push(compare_metric(
            "items.avg_update_time_us",
            b_items.avg_update_time_us,
            c_items.avg_update_time_us,
            config,
            true, // lower is better
        ));
    }

    // Compare persistence metrics
    if let (Some(b_persist), Some(c_persist)) = (&baseline.persistence, &current.persistence) {
        diffs.push(compare_metric(
            "persistence.avg_save_time_us",
            b_persist.avg_save_time_us,
            c_persist.avg_save_time_us,
            config,
            true, // lower is better
        ));

        diffs.push(compare_metric(
            "persistence.compression_ratio",
            b_persist.compression_ratio,
            c_persist.compression_ratio,
            config,
            false, // higher is better
        ));
    }

    // Compare execution metrics
    diffs.push(compare_metric(
        "execution.duration_seconds",
        baseline.test_execution.duration_seconds,
        current.test_execution.duration_seconds,
        config,
        true, // lower is better
    ));

    diffs
}

fn compare_metric(
    name: &str,
    baseline: f64,
    current: f64,
    config: &Config,
    lower_is_better: bool,
) -> MetricDiff {
    let change_percent = if baseline == 0.0 {
        if current == 0.0 {
            0.0
        } else {
            100.0 // or handle specially
        }
    } else {
        ((current - baseline) / baseline) * 100.0
    };

    // Determine status based on whether lower or higher is better
    let status = if lower_is_better {
        // For metrics where lower is better (e.g., time)
        if change_percent > config.threshold_failure * 100.0 {
            DiffStatus::Failure
        } else if change_percent > config.threshold_warning * 100.0 {
            DiffStatus::Warning
        } else {
            DiffStatus::Pass
        }
    } else {
        // For metrics where higher is better (e.g., throughput)
        if change_percent < -config.threshold_failure * 100.0 {
            DiffStatus::Failure
        } else if change_percent < -config.threshold_warning * 100.0 {
            DiffStatus::Warning
        } else {
            DiffStatus::Pass
        }
    };

    MetricDiff {
        name: name.to_string(),
        baseline,
        current,
        change_percent,
        status,
    }
}

fn print_text_report(
    diffs: &[MetricDiff],
    baseline: &MetricsReport,
    current: &MetricsReport,
    config: &Config,
) {
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║           Metrics Diff Report                                  ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();

    println!("Baseline:  {}", config.baseline_path.display());
    println!("Current:   {}", config.current_path.display());
    println!();
    println!("Test:      {} → {}", baseline.test_name, current.test_name);
    println!("Result:    {:?} → {:?}", baseline.result, current.result);
    println!();

    println!("Thresholds:");
    println!("  Warning:  {}%", config.threshold_warning * 100.0);
    println!("  Failure:  {}%", config.threshold_failure * 100.0);
    println!();

    println!("┌────────────────────────────────────┬──────────────┬──────────────┬──────────┬────────┐");
    println!("│ Metric                             │ Baseline     │ Current      │ Change   │ Status │");
    println!("├────────────────────────────────────┼──────────────┼──────────────┼──────────┼────────┤");

    for diff in diffs {
        let status_str = format!("{} {}", diff.status.to_emoji(), diff.status.to_string());
        println!(
            "│ {:<34} │ {:>12.3} │ {:>12.3} │ {:>7.2}% │ {:<6} │",
            truncate(&diff.name, 34),
            diff.baseline,
            diff.current,
            diff.change_percent,
            status_str
        );
    }

    println!("└────────────────────────────────────┴──────────────┴──────────────┴──────────┴────────┘");
    println!();

    // Summary
    let passed = diffs.iter().filter(|d| d.status == DiffStatus::Pass).count();
    let warned = diffs.iter().filter(|d| d.status == DiffStatus::Warning).count();
    let failed = diffs.iter().filter(|d| d.status == DiffStatus::Failure).count();

    println!("Summary:");
    println!("  ✅ Passed:  {}/{}", passed, diffs.len());
    if warned > 0 {
        println!("  ⚠️  Warnings: {}/{}", warned, diffs.len());
    }
    if failed > 0 {
        println!("  ❌ Failed:  {}/{}", failed, diffs.len());
    }
    println!();

    if failed > 0 {
        println!("❌ FAILURE: Performance regressions detected (>{}%)", config.threshold_failure * 100.0);
    } else if warned > 0 {
        println!("⚠️  WARNING: Performance degradation detected ({}%-{}%)",
            config.threshold_warning * 100.0, config.threshold_failure * 100.0);
    } else {
        println!("✅ SUCCESS: All metrics within acceptable ranges");
    }
}

fn print_json_report(diffs: &[MetricDiff]) {
    let mut report = HashMap::new();

    let metrics: Vec<HashMap<&str, serde_json::Value>> = diffs
        .iter()
        .map(|d| {
            let mut m = HashMap::new();
            m.insert("name", serde_json::json!(d.name));
            m.insert("baseline", serde_json::json!(d.baseline));
            m.insert("current", serde_json::json!(d.current));
            m.insert("change_percent", serde_json::json!(d.change_percent));
            m.insert("status", serde_json::json!(d.status.to_string()));
            m
        })
        .collect();

    report.insert("metrics", serde_json::json!(metrics));

    let passed = diffs.iter().filter(|d| d.status == DiffStatus::Pass).count();
    let warned = diffs.iter().filter(|d| d.status == DiffStatus::Warning).count();
    let failed = diffs.iter().filter(|d| d.status == DiffStatus::Failure).count();

    report.insert("summary", serde_json::json!({
        "total": diffs.len(),
        "passed": passed,
        "warnings": warned,
        "failures": failed,
    }));

    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

fn main() {
    let config = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(2);
        }
    };

    let baseline = match load_metrics(&config.baseline_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error loading baseline: {}", e);
            process::exit(2);
        }
    };

    let current = match load_metrics(&config.current_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error loading current: {}", e);
            process::exit(2);
        }
    };

    let diffs = compare_metrics(&baseline, &current, &config);

    match config.format {
        OutputFormat::Text => print_text_report(&diffs, &baseline, &current, &config),
        OutputFormat::Json => print_json_report(&diffs),
    }

    // Determine exit code
    let has_failures = diffs.iter().any(|d| d.status == DiffStatus::Failure);
    let has_warnings = diffs.iter().any(|d| d.status == DiffStatus::Warning);

    if has_failures {
        process::exit(2);
    } else if has_warnings {
        process::exit(1);
    } else {
        process::exit(0);
    }
}
