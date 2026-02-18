use anyhow::Result;
use chrono::NaiveDate;

use openvital::core::export;
use openvital::db::Database;
use openvital::models::config::Config;
use openvital::output;

pub fn run_export(
    format: &str,
    output_path: Option<&str>,
    metric_type: Option<&str>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    human: bool,
) -> Result<()> {
    let db = Database::open(&Config::db_path())?;

    let content = match format {
        "csv" => export::to_csv(&db, metric_type, from, to)?,
        "json" => export::to_json(&db, metric_type, from, to)?,
        other => anyhow::bail!("unsupported format: {} (expected csv/json)", other),
    };

    if let Some(path) = output_path {
        std::fs::write(path, &content)?;
        if human {
            println!("Exported to {}", path);
        } else {
            let out = output::success(
                "export",
                serde_json::json!({"path": path, "format": format}),
            );
            println!("{}", serde_json::to_string(&out)?);
        }
    } else {
        print!("{}", content);
    }
    Ok(())
}

pub fn run_import(source: &str, file_path: &str, human: bool) -> Result<()> {
    let db = Database::open(&Config::db_path())?;
    let content = std::fs::read_to_string(file_path)?;

    let count = match source {
        "json" => export::import_json(&db, &content)?,
        "csv" => export::import_csv(&db, &content)?,
        other => anyhow::bail!("unsupported import source: {} (expected csv/json)", other),
    };

    if human {
        println!("Imported {} entries from {}", count, file_path);
    } else {
        let out = output::success(
            "import",
            serde_json::json!({"count": count, "source": source, "file": file_path}),
        );
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
