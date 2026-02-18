use anyhow::Result;
use chrono::{Local, NaiveDate};

use crate::db::Database;
use crate::models::config::Config;
use crate::models::metric::Metric;

pub enum ShowResult {
    ByType {
        metric_type: String,
        entries: Vec<Metric>,
    },
    ByDate {
        date: NaiveDate,
        entries: Vec<Metric>,
    },
}

/// Query metrics by type or date.
pub fn show(
    db: &Database,
    config: &Config,
    metric_type: Option<&str>,
    last: Option<u32>,
    date: Option<NaiveDate>,
) -> Result<ShowResult> {
    // `show today` or `show` with no args → today's entries
    if metric_type == Some("today") || (metric_type.is_none() && date.is_none()) {
        let d = date.unwrap_or_else(|| Local::now().date_naive());
        let entries = db.query_by_date(d)?;
        return Ok(ShowResult::ByDate { date: d, entries });
    }

    if let Some(d) = date {
        let entries = db.query_by_date(d)?;
        return Ok(ShowResult::ByDate { date: d, entries });
    }

    let metric_type = metric_type.unwrap();
    let resolved = config.resolve_alias(metric_type);
    let entries = db.query_by_type(&resolved, Some(last.unwrap_or(1)))?;
    Ok(ShowResult::ByType {
        metric_type: resolved,
        entries,
    })
}
