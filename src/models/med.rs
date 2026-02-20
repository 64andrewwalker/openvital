use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Route
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    Oral,
    Topical,
    Ophthalmic,
    Injection,
    Inhaled,
    Sublingual,
    Transdermal,
    Other(String),
}

impl FromStr for Route {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "oral" => Self::Oral,
            "topical" => Self::Topical,
            "ophthalmic" => Self::Ophthalmic,
            "injection" => Self::Injection,
            "inhaled" => Self::Inhaled,
            "sublingual" => Self::Sublingual,
            "transdermal" => Self::Transdermal,
            other => Self::Other(other.to_string()),
        })
    }
}

impl fmt::Display for Route {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oral => write!(f, "oral"),
            Self::Topical => write!(f, "topical"),
            Self::Ophthalmic => write!(f, "ophthalmic"),
            Self::Injection => write!(f, "injection"),
            Self::Inhaled => write!(f, "inhaled"),
            Self::Sublingual => write!(f, "sublingual"),
            Self::Transdermal => write!(f, "transdermal"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Frequency
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frequency {
    Daily,
    TwiceDaily,
    ThreeTimesDaily,
    Weekly,
    AsNeeded,
}

impl Frequency {
    /// How many doses are required per day, if the schedule is fixed.
    /// Returns `None` for `Weekly` and `AsNeeded`.
    pub fn required_per_day(&self) -> Option<u32> {
        match self {
            Self::Daily => Some(1),
            Self::TwiceDaily => Some(2),
            Self::ThreeTimesDaily => Some(3),
            Self::Weekly | Self::AsNeeded => None,
        }
    }
}

impl FromStr for Frequency {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "daily" => Ok(Self::Daily),
            "2x_daily" => Ok(Self::TwiceDaily),
            "3x_daily" => Ok(Self::ThreeTimesDaily),
            "weekly" => Ok(Self::Weekly),
            "as_needed" => Ok(Self::AsNeeded),
            other => Err(anyhow::anyhow!("unknown frequency: {other}")),
        }
    }
}

impl fmt::Display for Frequency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Daily => write!(f, "daily"),
            Self::TwiceDaily => write!(f, "2x_daily"),
            Self::ThreeTimesDaily => write!(f, "3x_daily"),
            Self::Weekly => write!(f, "weekly"),
            Self::AsNeeded => write!(f, "as_needed"),
        }
    }
}

// ---------------------------------------------------------------------------
// ParsedDose + parse_dose
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedDose {
    pub raw: String,
    pub value: Option<f64>,
    pub unit: String,
}

/// Parse a dose string into a structured `ParsedDose`.
///
/// Handles decimal (`"400mg"`), fraction (`"1/2 tablet"`), unicode fraction
/// (`"\u{00bd} tablet"`), space-separated (`"2 drops"`), and bare text (`"thin layer"`).
/// Returns a default dose of `1.0 dose` when input is `None` or empty.
pub fn parse_dose(input: Option<&str>) -> ParsedDose {
    let raw = match input {
        Some(s) if !s.is_empty() => s,
        _ => {
            return ParsedDose {
                raw: String::new(),
                value: Some(1.0),
                unit: "dose".to_string(),
            };
        }
    };

    let trimmed = raw.trim();

    // Try unicode fraction prefix (e.g. "½ tablet")
    if let Some(parsed) = try_unicode_fraction(trimmed) {
        return parsed;
    }

    // Try fraction like "1/2 tablet"
    if let Some(parsed) = try_fraction(trimmed) {
        return parsed;
    }

    // Try decimal number (possibly glued to unit): "400mg", ".5mg", "2 drops"
    if let Some(parsed) = try_decimal(trimmed) {
        return parsed;
    }

    // No numeric component recognised
    ParsedDose {
        raw: raw.to_string(),
        value: None,
        unit: "application".to_string(),
    }
}

fn try_unicode_fraction(s: &str) -> Option<ParsedDose> {
    let fractions: &[(char, f64)] = &[
        ('\u{00bd}', 0.5), // ½
        ('\u{2153}', 1.0 / 3.0),
        ('\u{2154}', 2.0 / 3.0),
        ('\u{00bc}', 0.25), // ¼
        ('\u{00be}', 0.75), // ¾
    ];

    let first = s.chars().next()?;
    for &(ch, val) in fractions {
        if first == ch {
            let rest = s[ch.len_utf8()..].trim();
            let unit = if rest.is_empty() {
                "dose".to_string()
            } else {
                rest.to_string()
            };
            return Some(ParsedDose {
                raw: s.to_string(),
                value: Some(val),
                unit,
            });
        }
    }
    None
}

fn try_fraction(s: &str) -> Option<ParsedDose> {
    let re = Regex::new(r"^(\d+)\s*/\s*(\d+)\s*(.*)$").ok()?;
    let caps = re.captures(s)?;
    let num: f64 = caps[1].parse().ok()?;
    let den: f64 = caps[2].parse().ok()?;
    if den == 0.0 || num == 0.0 {
        return None;
    }
    let unit_str = caps[3].trim();
    let unit = if unit_str.is_empty() {
        "dose".to_string()
    } else {
        unit_str.to_string()
    };
    Some(ParsedDose {
        raw: s.to_string(),
        value: Some(num / den),
        unit,
    })
}

fn try_decimal(s: &str) -> Option<ParsedDose> {
    // Must start with a digit or a dot followed by a digit
    let re = Regex::new(r"^(\d+\.?\d*|\.\d+)\s*(.*)$").ok()?;
    let caps = re.captures(s)?;
    let val: f64 = caps[1].parse().ok()?;
    if val <= 0.0 {
        return None;
    }
    let unit_str = caps[2].trim();
    let unit = if unit_str.is_empty() {
        "dose".to_string()
    } else {
        unit_str.to_string()
    };
    Some(ParsedDose {
        raw: s.to_string(),
        value: Some(val),
        unit,
    })
}

// ---------------------------------------------------------------------------
// Medication
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Medication {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dose: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dose_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dose_unit: Option<String>,
    pub route: Route,
    pub frequency: Frequency,
    pub active: bool,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Medication {
    /// Create a new active medication with sensible defaults.
    pub fn new(name: impl Into<String>, frequency: Frequency) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            dose: None,
            dose_value: None,
            dose_unit: None,
            route: Route::Oral,
            frequency,
            active: true,
            started_at: now,
            stopped_at: None,
            stop_reason: None,
            note: None,
            created_at: now,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Route ---------------------------------------------------------------

    #[test]
    fn route_from_str_known() {
        assert_eq!("oral".parse::<Route>().unwrap(), Route::Oral);
        assert_eq!("TOPICAL".parse::<Route>().unwrap(), Route::Topical);
        assert_eq!("Ophthalmic".parse::<Route>().unwrap(), Route::Ophthalmic);
        assert_eq!("injection".parse::<Route>().unwrap(), Route::Injection);
        assert_eq!("inhaled".parse::<Route>().unwrap(), Route::Inhaled);
        assert_eq!("sublingual".parse::<Route>().unwrap(), Route::Sublingual);
        assert_eq!("transdermal".parse::<Route>().unwrap(), Route::Transdermal);
    }

    #[test]
    fn route_from_str_unknown() {
        assert_eq!(
            "rectal".parse::<Route>().unwrap(),
            Route::Other("rectal".to_string())
        );
    }

    #[test]
    fn route_display_roundtrip() {
        let routes = [
            Route::Oral,
            Route::Topical,
            Route::Ophthalmic,
            Route::Injection,
            Route::Inhaled,
            Route::Sublingual,
            Route::Transdermal,
        ];
        for r in &routes {
            let s = r.to_string();
            let parsed: Route = s.parse().unwrap();
            assert_eq!(&parsed, r);
        }
    }

    // -- Frequency -----------------------------------------------------------

    #[test]
    fn frequency_from_str_valid() {
        assert_eq!("daily".parse::<Frequency>().unwrap(), Frequency::Daily);
        assert_eq!(
            "2x_daily".parse::<Frequency>().unwrap(),
            Frequency::TwiceDaily
        );
        assert_eq!(
            "3x_daily".parse::<Frequency>().unwrap(),
            Frequency::ThreeTimesDaily
        );
        assert_eq!("weekly".parse::<Frequency>().unwrap(), Frequency::Weekly);
        assert_eq!(
            "as_needed".parse::<Frequency>().unwrap(),
            Frequency::AsNeeded
        );
    }

    #[test]
    fn frequency_from_str_invalid() {
        assert!("biweekly".parse::<Frequency>().is_err());
    }

    #[test]
    fn frequency_display_roundtrip() {
        let freqs = [
            Frequency::Daily,
            Frequency::TwiceDaily,
            Frequency::ThreeTimesDaily,
            Frequency::Weekly,
            Frequency::AsNeeded,
        ];
        for f in &freqs {
            let s = f.to_string();
            let parsed: Frequency = s.parse().unwrap();
            assert_eq!(&parsed, f);
        }
    }

    #[test]
    fn frequency_required_per_day() {
        assert_eq!(Frequency::Daily.required_per_day(), Some(1));
        assert_eq!(Frequency::TwiceDaily.required_per_day(), Some(2));
        assert_eq!(Frequency::ThreeTimesDaily.required_per_day(), Some(3));
        assert_eq!(Frequency::Weekly.required_per_day(), None);
        assert_eq!(Frequency::AsNeeded.required_per_day(), None);
    }

    // -- parse_dose ----------------------------------------------------------

    #[test]
    fn parse_dose_none() {
        let d = parse_dose(None);
        assert_eq!(d.value, Some(1.0));
        assert_eq!(d.unit, "dose");
    }

    #[test]
    fn parse_dose_empty() {
        let d = parse_dose(Some(""));
        assert_eq!(d.value, Some(1.0));
        assert_eq!(d.unit, "dose");
    }

    #[test]
    fn parse_dose_decimal_glued() {
        let d = parse_dose(Some("400mg"));
        assert_eq!(d.value, Some(400.0));
        assert_eq!(d.unit, "mg");
    }

    #[test]
    fn parse_dose_decimal_dot_prefix() {
        let d = parse_dose(Some(".5mg"));
        assert_eq!(d.value, Some(0.5));
        assert_eq!(d.unit, "mg");
    }

    #[test]
    fn parse_dose_with_space() {
        let d = parse_dose(Some("2 drops"));
        assert_eq!(d.value, Some(2.0));
        assert_eq!(d.unit, "drops");
    }

    #[test]
    fn parse_dose_fraction() {
        let d = parse_dose(Some("1/2 tablet"));
        assert_eq!(d.value, Some(0.5));
        assert_eq!(d.unit, "tablet");
    }

    #[test]
    fn parse_dose_unicode_fraction() {
        let d = parse_dose(Some("\u{00bd} tablet"));
        assert_eq!(d.value, Some(0.5));
        assert_eq!(d.unit, "tablet");
    }

    #[test]
    fn parse_dose_no_numeric() {
        let d = parse_dose(Some("thin layer"));
        assert_eq!(d.value, None);
        assert_eq!(d.unit, "application");
    }

    #[test]
    fn parse_dose_unit_before_number() {
        let d = parse_dose(Some("mg400"));
        assert_eq!(d.value, None);
        assert_eq!(d.unit, "application");
    }

    #[test]
    fn parse_dose_zero_denominator() {
        let d = parse_dose(Some("0/0 tablet"));
        assert_eq!(d.value, None);
        assert_eq!(d.unit, "application");
    }

    #[test]
    fn parse_dose_negative() {
        let d = parse_dose(Some("-5mg"));
        assert_eq!(d.value, None);
        assert_eq!(d.unit, "application");
    }

    // -- Medication ----------------------------------------------------------

    #[test]
    fn medication_new_defaults() {
        let med = Medication::new("ibuprofen", Frequency::Daily);
        assert_eq!(med.name, "ibuprofen");
        assert_eq!(med.frequency, Frequency::Daily);
        assert_eq!(med.route, Route::Oral);
        assert!(med.active);
        assert!(!med.id.is_empty());
        assert!(med.stopped_at.is_none());
        assert!(med.dose.is_none());
    }

    // -- Serde roundtrip -----------------------------------------------------

    #[test]
    fn route_serde_roundtrip() {
        let route = Route::Sublingual;
        let json = serde_json::to_string(&route).unwrap();
        let back: Route = serde_json::from_str(&json).unwrap();
        assert_eq!(back, route);
    }

    #[test]
    fn frequency_serde_roundtrip() {
        let freq = Frequency::ThreeTimesDaily;
        let json = serde_json::to_string(&freq).unwrap();
        let back: Frequency = serde_json::from_str(&json).unwrap();
        assert_eq!(back, freq);
    }
}
