use anyhow::Result;

use crate::db::Database;
use crate::models::config::Config;
use crate::output;
use crate::output::human;

pub fn run(human_flag: bool) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;
    let status = crate::core::status::compute(&db, &config)?;

    if human_flag {
        println!("{}", human::format_status(&status));
    } else {
        let out = output::success("status", serde_json::to_value(&status)?);
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
