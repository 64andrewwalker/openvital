use anyhow::Result;

use openvital::db::Database;
use openvital::models::config::Config;
use openvital::output;
use openvital::output::human;

pub fn run(human_flag: bool) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;
    let status = openvital::core::status::compute(&db, &config)?;

    if human_flag {
        println!("{}", human::format_status(&status, &config.units));
    } else {
        let out = output::success("status", serde_json::to_value(&status)?);
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
