use openvital::core::units;
use openvital::models::config::Units;

#[test]
fn test_to_display_weight_metric() {
    let u = Units::default();
    let (val, unit) = units::to_display(72.5, "weight", &u);
    assert!((val - 72.5).abs() < 0.01);
    assert_eq!(unit, "kg");
}

#[test]
fn test_to_display_weight_imperial() {
    let u = Units::imperial();
    let (val, unit) = units::to_display(72.5, "weight", &u);
    assert!((val - 159.8).abs() < 0.2);
    assert_eq!(unit, "lbs");
}

#[test]
fn test_to_display_waist_imperial() {
    let u = Units::imperial();
    let (val, unit) = units::to_display(80.0, "waist", &u);
    assert!((val - 31.5).abs() < 0.1);
    assert_eq!(unit, "in");
}

#[test]
fn test_to_display_height_imperial() {
    let u = Units::imperial();
    let (val, unit) = units::to_display(178.0, "height", &u);
    assert!((val - 5.8).abs() < 0.1);
    assert_eq!(unit, "ft");
}

#[test]
fn test_to_display_water_imperial() {
    let u = Units::imperial();
    let (val, unit) = units::to_display(2000.0, "water", &u);
    assert!((val - 67.6).abs() < 0.2);
    assert_eq!(unit, "fl oz");
}

#[test]
fn test_to_display_temperature_imperial() {
    let u = Units::imperial();
    let (val, unit) = units::to_display(37.0, "temperature", &u);
    assert!((val - 98.6).abs() < 0.1);
    assert_eq!(unit, "\u{00b0}F");
}

#[test]
fn test_to_display_unaffected_metric_types() {
    let u = Units::imperial();
    let (val, unit) = units::to_display(8.0, "sleep", &u);
    assert!((val - 8.0).abs() < 0.01);
    assert_eq!(unit, "hours");

    let (val, unit) = units::to_display(68.0, "heart_rate", &u);
    assert!((val - 68.0).abs() < 0.01);
    assert_eq!(unit, "bpm");
}

#[test]
fn test_from_input_weight_metric() {
    let u = Units::default();
    let val = units::from_input(72.5, "weight", &u);
    assert!((val - 72.5).abs() < 0.01);
}

#[test]
fn test_from_input_weight_imperial() {
    let u = Units::imperial();
    let val = units::from_input(160.0, "weight", &u);
    assert!((val - 72.57).abs() < 0.1);
}

#[test]
fn test_from_input_water_imperial() {
    let u = Units::imperial();
    let val = units::from_input(67.6, "water", &u);
    assert!((val - 2000.0).abs() < 5.0);
}

#[test]
fn test_from_input_temperature_imperial() {
    let u = Units::imperial();
    let val = units::from_input(98.6, "temperature", &u);
    assert!((val - 37.0).abs() < 0.1);
}

#[test]
fn test_from_input_height_imperial() {
    let u = Units::imperial();
    let val = units::from_input(5.83, "height", &u);
    assert!((val - 177.7).abs() < 0.5);
}

#[test]
fn test_from_input_sleep_unaffected() {
    let u = Units::imperial();
    let val = units::from_input(8.0, "sleep", &u);
    assert!((val - 8.0).abs() < 0.01);
}

#[test]
fn test_round_trip_weight() {
    let u = Units::imperial();
    let stored = units::from_input(160.0, "weight", &u);
    let (displayed, _) = units::to_display(stored, "weight", &u);
    assert!((displayed - 160.0).abs() < 0.1);
}

#[test]
fn test_round_trip_temperature() {
    let u = Units::imperial();
    let stored = units::from_input(98.6, "temperature", &u);
    let (displayed, _) = units::to_display(stored, "temperature", &u);
    assert!((displayed - 98.6).abs() < 0.1);
}

#[test]
fn test_round_trip_height() {
    let u = Units::imperial();
    let stored = units::from_input(5.83, "height", &u);
    let (displayed, _) = units::to_display(stored, "height", &u);
    assert!((displayed - 5.8).abs() < 0.1);
}
