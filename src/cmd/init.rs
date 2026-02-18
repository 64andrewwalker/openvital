use anyhow::Result;
use std::io::{self, Write};

use openvital::db::Database;
use openvital::models::config::{Config, Units};
use openvital::models::metric::Metric;

pub fn run(skip: bool, units_arg: Option<&str>) -> Result<()> {
    let mut config = Config::load().unwrap_or_default();

    if config.aliases.is_empty() {
        config.aliases = Config::default_aliases();
    }

    // Apply --units flag if provided
    if let Some(system) = units_arg {
        match system {
            "imperial" => config.units = Units::imperial(),
            "metric" => config.units = Units::default(),
            other => anyhow::bail!(
                "Unknown unit system '{}'. Use 'metric' or 'imperial'.",
                other
            ),
        }
    }

    if !skip {
        let is_imperial = config.units.is_imperial();
        let height_label = if is_imperial {
            "Height (ft, e.g. 5.75)"
        } else {
            "Height (cm)"
        };
        let weight_label = if is_imperial {
            "Current weight (lbs)"
        } else {
            "Current weight (kg)"
        };

        println!("OpenVital — Initial Setup\n");

        let height_input = prompt_f64(height_label)?;
        let height_cm = if is_imperial {
            openvital::core::units::from_input(height_input, "height", &config.units)
        } else {
            height_input
        };
        config.profile.height_cm = Some(height_cm);

        let weight_input = prompt_f64(weight_label)?;
        let weight_kg = if is_imperial {
            openvital::core::units::from_input(weight_input, "weight", &config.units)
        } else {
            weight_input
        };

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

        // Log initial weight (always stored in kg)
        let db = Database::open(&Config::db_path())?;
        let m = Metric::new("weight".into(), weight_kg);
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
