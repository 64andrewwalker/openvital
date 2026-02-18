mod common;

use openvital::models::goal::{Direction, Goal, Timeframe};

#[test]
fn test_create_and_list_goals() {
    let (_dir, db) = common::setup_db();

    let goal = Goal::new("weight".into(), 75.0, Direction::Below, Timeframe::Monthly);
    db.insert_goal(&goal).unwrap();

    let goals = db.list_goals(true).unwrap();
    assert_eq!(goals.len(), 1);
    assert_eq!(goals[0].metric_type, "weight");
    assert_eq!(goals[0].target_value, 75.0);
    assert!(goals[0].active);
}

#[test]
fn test_get_goal_by_type() {
    let (_dir, db) = common::setup_db();

    let goal = Goal::new("cardio".into(), 150.0, Direction::Above, Timeframe::Weekly);
    db.insert_goal(&goal).unwrap();

    let found = db.get_goal_by_type("cardio").unwrap();
    assert!(found.is_some());
    let g = found.unwrap();
    assert_eq!(g.target_value, 150.0);

    let not_found = db.get_goal_by_type("sleep_hours").unwrap();
    assert!(not_found.is_none());
}

#[test]
fn test_remove_goal() {
    let (_dir, db) = common::setup_db();

    let goal = Goal::new("water".into(), 2000.0, Direction::Above, Timeframe::Daily);
    let id = goal.id.clone();
    db.insert_goal(&goal).unwrap();

    assert!(db.remove_goal(&id).unwrap());
    let goals = db.list_goals(true).unwrap();
    assert!(goals.is_empty());
    let all = db.list_goals(false).unwrap();
    assert_eq!(all.len(), 1);
    assert!(!all[0].active);
}

#[test]
fn test_goal_is_met() {
    let g = Goal::new("weight".into(), 75.0, Direction::Below, Timeframe::Monthly);
    assert!(g.is_met(74.0));
    assert!(g.is_met(75.0));
    assert!(!g.is_met(76.0));

    let g2 = Goal::new("cardio".into(), 150.0, Direction::Above, Timeframe::Weekly);
    assert!(g2.is_met(150.0));
    assert!(g2.is_met(200.0));
    assert!(!g2.is_met(100.0));
}
