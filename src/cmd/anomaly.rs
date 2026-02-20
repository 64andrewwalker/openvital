use anyhow::Result;
use std::str::FromStr;

use openvital::core::anomaly;
use openvital::db::Database;
use openvital::models::anomaly::Threshold;
use openvital::models::config::Config;
use openvital::output;
use openvital::output::human;

pub fn run(metric_type: Option<&str>, days: u32, threshold: &str, human_flag: bool) -> Result<()> {
    let _config = Config::load()?;
    let db = Database::open(&Config::db_path())?;
    let threshold = Threshold::from_str(threshold)?;

    let result = anomaly::detect(&db, metric_type, days, threshold)?;

    if human_flag {
        println!("{}", human::format_anomaly(&result));
    } else {
        let out = output::success("anomaly", serde_json::to_value(&result)?);
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
