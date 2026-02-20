use anyhow::Result;

use openvital::core::context;
use openvital::db::Database;
use openvital::models::config::Config;
use openvital::output;
use openvital::output::human;

pub fn run(days: u32, types: Option<&str>, human_flag: bool) -> Result<()> {
    let config = Config::load()?;
    let db = Database::open(&Config::db_path())?;

    let type_filter: Option<Vec<&str>> = types.map(|t| t.split(',').collect());
    let type_refs: Option<&[&str]> = type_filter.as_deref();

    let result = context::compute(&db, &config, days, type_refs)?;

    if human_flag {
        println!("{}", human::format_context(&result));
    } else {
        let out = output::success("context", serde_json::to_value(&result)?);
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
