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

#[test]
fn test_get_goal_by_id_found() {
    let (_dir, db) = common::setup_db();

    let goal = Goal::new("steps".into(), 10000.0, Direction::Above, Timeframe::Daily);
    let id = goal.id.clone();
    db.insert_goal(&goal).unwrap();

    let found = db.get_goal(&id).unwrap();
    assert!(found.is_some());
    let g = found.unwrap();
    assert_eq!(g.id, id);
    assert_eq!(g.metric_type, "steps");
    assert_eq!(g.target_value, 10000.0);
    assert_eq!(g.direction, Direction::Above);
    assert_eq!(g.timeframe, Timeframe::Daily);
    assert!(g.active);
}

#[test]
fn test_get_goal_by_id_not_found() {
    let (_dir, db) = common::setup_db();

    let result = db.get_goal("00000000-0000-0000-0000-000000000000").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_list_goals_active_only_false_includes_inactive() {
    let (_dir, db) = common::setup_db();

    let goal1 = Goal::new(
        "sleep_hours".into(),
        8.0,
        Direction::Above,
        Timeframe::Daily,
    );
    let goal2 = Goal::new(
        "calories".into(),
        2000.0,
        Direction::Below,
        Timeframe::Daily,
    );
    let id1 = goal1.id.clone();
    db.insert_goal(&goal1).unwrap();
    db.insert_goal(&goal2).unwrap();

    // Deactivate goal1
    db.remove_goal(&id1).unwrap();

    // active_only=true should return only goal2
    let active = db.list_goals(true).unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].metric_type, "calories");

    // active_only=false should return both goals
    let all = db.list_goals(false).unwrap();
    assert_eq!(all.len(), 2);
    let inactive: Vec<_> = all.iter().filter(|g| !g.active).collect();
    assert_eq!(inactive.len(), 1);
    assert_eq!(inactive[0].metric_type, "sleep_hours");
}

#[test]
fn test_remove_goal_nonexistent_returns_false() {
    let (_dir, db) = common::setup_db();

    let removed = db
        .remove_goal("nonexistent-id-that-does-not-exist")
        .unwrap();
    assert!(!removed);
}

#[test]
fn test_remove_goal_already_inactive_returns_false() {
    let (_dir, db) = common::setup_db();

    let goal = Goal::new("vo2max".into(), 50.0, Direction::Above, Timeframe::Monthly);
    let id = goal.id.clone();
    db.insert_goal(&goal).unwrap();

    // First removal succeeds
    assert!(db.remove_goal(&id).unwrap());
    // Second removal returns false (already inactive)
    assert!(!db.remove_goal(&id).unwrap());
}
