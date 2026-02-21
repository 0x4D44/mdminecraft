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
    Pass,    // Within acceptable range or improved
    Warning, // 5-10% degradation
    Failure, // >10% degradation
}

impl DiffStatus {
    fn emoji(self) -> &'static str {
        match self {
            DiffStatus::Pass => "✅",
            DiffStatus::Warning => "⚠️",
            DiffStatus::Failure => "❌",
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            DiffStatus::Pass => "PASS",
            DiffStatus::Warning => "WARN",
            DiffStatus::Failure => "FAIL",
        }
    }
}

fn parse_args() -> Result<Config, String> {
    parse_args_from_iter(std::env::args())
}

fn parse_args_from_iter<I>(args: I) -> Result<Config, String>
where
    I: IntoIterator<Item = String>,
{
    let args: Vec<String> = args.into_iter().collect();
    let program = args
        .get(0)
        .map(|s| s.as_str())
        .unwrap_or("metrics-diff");

    if args.len() < 3 {
        return Err(format!(
            "Usage: {} <baseline.json> <current.json> [options]\n\
             Options:\n\
               --format <text|json>           Output format (default: text)\n\
               --threshold-warning <percent>  Warning threshold (default: 0.05)\n\
               --threshold-failure <percent>  Failure threshold (default: 0.10)",
            program
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

    serde_json::from_str(&content).map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
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

    println!(
        "┌────────────────────────────────────┬──────────────┬──────────────┬──────────┬────────┐"
    );
    println!(
        "│ Metric                             │ Baseline     │ Current      │ Change   │ Status │"
    );
    println!(
        "├────────────────────────────────────┼──────────────┼──────────────┼──────────┼────────┤"
    );

    for diff in diffs {
        let status_str = format!("{} {}", diff.status.emoji(), diff.status.as_str());
        println!(
            "│ {:<34} │ {:>12.3} │ {:>12.3} │ {:>7.2}% │ {:<6} │",
            truncate(&diff.name, 34),
            diff.baseline,
            diff.current,
            diff.change_percent,
            status_str
        );
    }

    println!(
        "└────────────────────────────────────┴──────────────┴──────────────┴──────────┴────────┘"
    );
    println!();

    // Summary
    let passed = diffs
        .iter()
        .filter(|d| d.status == DiffStatus::Pass)
        .count();
    let warned = diffs
        .iter()
        .filter(|d| d.status == DiffStatus::Warning)
        .count();
    let failed = diffs
        .iter()
        .filter(|d| d.status == DiffStatus::Failure)
        .count();

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
        println!(
            "❌ FAILURE: Performance regressions detected (>{}%)",
            config.threshold_failure * 100.0
        );
    } else if warned > 0 {
        println!(
            "⚠️  WARNING: Performance degradation detected ({}%-{}%)",
            config.threshold_warning * 100.0,
            config.threshold_failure * 100.0
        );
    } else {
        println!("✅ SUCCESS: All metrics within acceptable ranges");
    }
}

fn print_json_report(diffs: &[MetricDiff]) {
    let report = build_json_report(diffs);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

fn build_json_report(diffs: &[MetricDiff]) -> serde_json::Value {
    let mut report = HashMap::new();

    let metrics: Vec<HashMap<&str, serde_json::Value>> = diffs
        .iter()
        .map(|d| {
            let mut m = HashMap::new();
            m.insert("name", serde_json::json!(d.name));
            m.insert("baseline", serde_json::json!(d.baseline));
            m.insert("current", serde_json::json!(d.current));
            m.insert("change_percent", serde_json::json!(d.change_percent));
            m.insert("status", serde_json::json!(d.status.as_str()));
            m
        })
        .collect();

    report.insert("metrics", serde_json::json!(metrics));

    let passed = diffs
        .iter()
        .filter(|d| d.status == DiffStatus::Pass)
        .count();
    let warned = diffs
        .iter()
        .filter(|d| d.status == DiffStatus::Warning)
        .count();
    let failed = diffs
        .iter()
        .filter(|d| d.status == DiffStatus::Failure)
        .count();

    report.insert(
        "summary",
        serde_json::json!({
            "total": diffs.len(),
            "passed": passed,
            "warnings": warned,
            "failures": failed,
        }),
    );

    serde_json::json!(report)
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdminecraft_testkit::{MetricsReportBuilder, TestExecutionMetrics, TestResult};

    fn default_config() -> Config {
        Config {
            baseline_path: PathBuf::from("baseline.json"),
            current_path: PathBuf::from("current.json"),
            threshold_warning: 0.05,
            threshold_failure: 0.10,
            format: OutputFormat::Text,
        }
    }

    fn report_with_duration(name: &str, duration: f64) -> MetricsReport {
        MetricsReportBuilder::new(name)
            .result(TestResult::Pass)
            .execution(TestExecutionMetrics {
                duration_seconds: duration,
                peak_memory_mb: None,
                assertions_checked: None,
                validations_passed: None,
            })
            .build()
    }

    #[test]
    fn parse_args_defaults() {
        let config = parse_args_from_iter(vec![
            "metrics-diff".to_string(),
            "base.json".to_string(),
            "curr.json".to_string(),
        ])
        .expect("config");
        assert_eq!(config.baseline_path, PathBuf::from("base.json"));
        assert_eq!(config.current_path, PathBuf::from("curr.json"));
        assert_eq!(config.threshold_warning, 0.05);
        assert_eq!(config.threshold_failure, 0.10);
        assert!(matches!(config.format, OutputFormat::Text));
    }

    #[test]
    fn parse_args_format_json() {
        let config = parse_args_from_iter(vec![
            "metrics-diff".to_string(),
            "base.json".to_string(),
            "curr.json".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ])
        .expect("config");
        assert!(matches!(config.format, OutputFormat::Json));
    }

    #[test]
    fn compare_metric_thresholds_lower_is_better() {
        let config = default_config();
        let fail = compare_metric("t", 100.0, 120.0, &config, true);
        assert_eq!(fail.status, DiffStatus::Failure);
        let warn = compare_metric("t", 100.0, 107.0, &config, true);
        assert_eq!(warn.status, DiffStatus::Warning);
        let pass = compare_metric("t", 100.0, 101.0, &config, true);
        assert_eq!(pass.status, DiffStatus::Pass);
    }

    #[test]
    fn compare_metric_thresholds_higher_is_better() {
        let config = default_config();
        let fail = compare_metric("t", 100.0, 80.0, &config, false);
        assert_eq!(fail.status, DiffStatus::Failure);
        let warn = compare_metric("t", 100.0, 94.0, &config, false);
        assert_eq!(warn.status, DiffStatus::Warning);
        let pass = compare_metric("t", 100.0, 101.0, &config, false);
        assert_eq!(pass.status, DiffStatus::Pass);
    }

    #[test]
    fn compare_metrics_includes_execution() {
        let config = default_config();
        let baseline = report_with_duration("baseline", 1.0);
        let current = report_with_duration("current", 1.2);
        let diffs = compare_metrics(&baseline, &current, &config);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].name, "execution.duration_seconds");
        assert_eq!(diffs[0].status, DiffStatus::Failure);
    }

    #[test]
    fn build_json_report_counts_summary() {
        let config = default_config();
        let diff_pass = compare_metric("a", 1.0, 1.0, &config, true);
        let diff_warn = compare_metric("b", 1.0, 1.06, &config, true);
        let diff_fail = compare_metric("c", 1.0, 1.2, &config, true);
        let report = build_json_report(&[diff_pass, diff_warn, diff_fail]);
        let summary = &report["summary"];
        assert_eq!(summary["total"].as_u64(), Some(3));
        assert_eq!(summary["passed"].as_u64(), Some(1));
        assert_eq!(summary["warnings"].as_u64(), Some(1));
        assert_eq!(summary["failures"].as_u64(), Some(1));
    }

    #[test]
    fn truncate_short_and_long() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("longer-than", 8), "longe...");
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
