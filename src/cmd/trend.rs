use anyhow::Result;

use openvital::core::trend::{self, TrendPeriod};
use openvital::db::Database;
use openvital::models::config::Config;
use openvital::output;

pub fn run(metric_type: &str, period: Option<&str>, last: Option<u32>, human: bool) -> Result<()> {
    let config = Config::load()?;
    let resolved = config.resolve_alias(metric_type);
    let db = Database::open(&Config::db_path())?;
    let period: TrendPeriod = period.unwrap_or("weekly").parse()?;
    let result = trend::compute(&db, &resolved, period, last)?;

    if human {
        if result.data.is_empty() {
            println!("No data for '{}'", resolved);
        } else {
            println!("Trend: {} ({})\n", resolved, result.period);
            for d in &result.data {
                let (avg, _) = openvital::core::units::to_display(d.avg, &resolved, &config.units);
                let (min, _) = openvital::core::units::to_display(d.min, &resolved, &config.units);
                let (max, unit) =
                    openvital::core::units::to_display(d.max, &resolved, &config.units);
                println!(
                    "  {} | avg: {:.1}  min: {:.1}  max: {:.1}  (n={}) [{}]",
                    d.label, avg, min, max, d.count, unit
                );
            }
            println!();
            println!(
                "  Direction: {} ({:+.1} {})",
                result.trend.direction, result.trend.rate, result.trend.rate_unit
            );
            if let Some(p) = result.trend.projected_30d {
                let (pv, pu) = openvital::core::units::to_display(p, &resolved, &config.units);
                println!("  30-day projection: {:.1} {}", pv, pu);
            }
        }
    } else {
        let out = output::success("trend", serde_json::to_value(&result)?);
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

pub fn run_correlate(metrics: &str, last: Option<u32>, human: bool) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;

    let parts: Vec<&str> = metrics.split(',').collect();
    if parts.len() != 2 {
        anyhow::bail!("--correlate requires exactly two metric types separated by comma");
    }
    let a = config.resolve_alias(parts[0].trim());
    let b = config.resolve_alias(parts[1].trim());

    let result = trend::correlate(&db, &a, &b, last)?;

    if human {
        println!("Correlation: {} vs {}\n", result.metric_a, result.metric_b);
        println!("  Coefficient: {:.2}", result.coefficient);
        println!("  Data points: {}", result.data_points);
        println!("  Strength: {}", result.interpretation);
    } else {
        let out = output::success("correlate", serde_json::to_value(&result)?);
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
