use anyhow::Result;
use std::io::{self, Write};

use crate::db::Database;
use crate::models::config::Config;
use crate::models::metric::Metric;

pub fn run(skip: bool) -> Result<()> {
    let mut config = Config::load().unwrap_or_default();

    if config.aliases.is_empty() {
        config.aliases = Config::default_aliases();
    }

    if !skip {
        println!("OpenVital — Initial Setup\n");

        config.profile.height_cm = Some(prompt_f64("Height (cm)")?);
        let weight = prompt_f64("Current weight (kg)")?;
        config.profile.birth_year = Some(prompt_u16("Birth year")?);
        config.profile.gender = Some(prompt_string("Gender (male/female/other)")?);

        let conditions = prompt_string("Known conditions (comma separated, or empty)")?;
        if !conditions.is_empty() {
            config.profile.conditions = conditions
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
        }

        config.profile.primary_exercise = Some(prompt_string("Primary exercise type")?);

        config.save()?;

        // Log initial weight
        let db = Database::open(&Config::db_path())?;
        let m = Metric::new("weight".into(), weight);
        db.insert_metric(&m)?;

        println!("\nSetup complete. Data stored in {:?}", Config::data_dir());
    } else {
        config.save()?;
        println!("Config initialized with defaults at {:?}", Config::path());
    }

    Ok(())
}

fn prompt_string(label: &str) -> Result<String> {
    print!("{}: ", label);
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().to_string())
}

fn prompt_f64(label: &str) -> Result<f64> {
    loop {
        let s = prompt_string(label)?;
        match s.parse::<f64>() {
            Ok(v) => return Ok(v),
            Err(_) => println!("Please enter a number."),
        }
    }
}

fn prompt_u16(label: &str) -> Result<u16> {
    loop {
        let s = prompt_string(label)?;
        match s.parse::<u16>() {
            Ok(v) => return Ok(v),
            Err(_) => println!("Please enter a number."),
        }
    }
}
