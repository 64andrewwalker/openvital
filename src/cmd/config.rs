use anyhow::Result;
use serde_json::json;

use openvital::models::config::Config;
use openvital::output;

pub fn run_show(human: bool) -> Result<()> {
    let config = Config::load()?;
    if human {
        let toml_str = toml::to_string_pretty(&config)?;
        println!("{}", toml_str);
    } else {
        let out = output::success("config", json!({ "config": config }));
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}

pub fn run_set(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load()?;

    match key {
        "height" => config.profile.height_cm = Some(value.parse()?),
        "birth_year" => config.profile.birth_year = Some(value.parse()?),
        "gender" => config.profile.gender = Some(value.to_string()),
        "conditions" => {
            config.profile.conditions = value.split(',').map(|s| s.trim().to_string()).collect();
        }
        "primary_exercise" => config.profile.primary_exercise = Some(value.to_string()),
        "units.system" => match value {
            "metric" => config.units = openvital::models::config::Units::default(),
            "imperial" => config.units = openvital::models::config::Units::imperial(),
            _ => anyhow::bail!("units.system must be 'metric' or 'imperial'"),
        },
        k if k.starts_with("alias.") => {
            let alias = k.strip_prefix("alias.").unwrap();
            config.aliases.insert(alias.to_string(), value.to_string());
        }
        _ => anyhow::bail!("unknown config key: {}", key),
    }

    config.save()?;
    let out = output::success("config", json!({ "key": key, "value": value }));
    println!("{}", serde_json::to_string(&out)?);
    Ok(())
}
