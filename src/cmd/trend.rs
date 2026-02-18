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
                println!(
                    "  {} | avg: {:.1}  min: {:.1}  max: {:.1}  (n={})",
                    d.label, d.avg, d.min, d.max, d.count
                );
            }
            println!();
            println!(
                "  Direction: {} ({:+.1} {})",
                result.trend.direction, result.trend.rate, result.trend.rate_unit
            );
            if let Some(p) = result.trend.projected_30d {
                println!("  30-day projection: {:.1}", p);
            }
        }
    } else {
        let out = output::success("trend", serde_json::to_value(&result)?);
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
