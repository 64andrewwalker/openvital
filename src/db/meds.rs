use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use std::str::FromStr;

use crate::models::med::{Frequency, Medication, Route};

use super::Database;

struct MedicationRow {
    id: String,
    name: String,
    dose: Option<String>,
    dose_value: Option<f64>,
    dose_unit: Option<String>,
    route: String,
    frequency: String,
    active: bool,
    started_at: String,
    stopped_at: Option<String>,
    stop_reason: Option<String>,
    note: Option<String>,
    created_at: String,
}

fn row_to_medication(r: MedicationRow) -> Result<Medication> {
    let route = Route::from_str(&r.route).unwrap_or(Route::Oral);
    let frequency: Frequency = r.frequency.parse()?;
    let started_at: DateTime<Utc> =
        DateTime::parse_from_rfc3339(&r.started_at)?.with_timezone(&Utc);
    let stopped_at: Option<DateTime<Utc>> = match r.stopped_at {
        Some(ref s) => Some(DateTime::parse_from_rfc3339(s)?.with_timezone(&Utc)),
        None => None,
    };
    let created_at: DateTime<Utc> =
        DateTime::parse_from_rfc3339(&r.created_at)?.with_timezone(&Utc);

    Ok(Medication {
        id: r.id,
        name: r.name,
        dose: r.dose,
        dose_value: r.dose_value,
        dose_unit: r.dose_unit,
        route,
        frequency,
        active: r.active,
        started_at,
        stopped_at,
        stop_reason: r.stop_reason,
        note: r.note,
        created_at,
    })
}

const SELECT_COLS: &str = "id, name, dose, dose_value, dose_unit, route, frequency, active, started_at, stopped_at, stop_reason, note, created_at";

macro_rules! map_row {
    ($row:expr) => {
        Ok(MedicationRow {
            id: $row.get(0)?,
            name: $row.get(1)?,
            dose: $row.get(2)?,
            dose_value: $row.get(3)?,
            dose_unit: $row.get(4)?,
            route: $row.get(5)?,
            frequency: $row.get(6)?,
            active: $row.get(7)?,
            started_at: $row.get(8)?,
            stopped_at: $row.get(9)?,
            stop_reason: $row.get(10)?,
            note: $row.get(11)?,
            created_at: $row.get(12)?,
        })
    };
}

impl Database {
    pub fn insert_medication(&self, med: &Medication) -> Result<()> {
        self.conn.execute(
            "INSERT INTO medications (id, name, dose, dose_value, dose_unit, route, frequency, active, started_at, stopped_at, stop_reason, note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                med.id,
                med.name,
                med.dose,
                med.dose_value,
                med.dose_unit,
                med.route.to_string(),
                med.frequency.to_string(),
                med.active as i32,
                med.started_at.to_rfc3339(),
                med.stopped_at.map(|t| t.to_rfc3339()),
                med.stop_reason,
                med.note,
                med.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_medication_by_name(&self, name: &str) -> Result<Option<Medication>> {
        let sql = format!("SELECT {SELECT_COLS} FROM medications WHERE name = ?1 AND active = 1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map(params![name], |row| map_row!(row))?;
        match rows.next() {
            Some(row) => Ok(Some(row_to_medication(row?)?)),
            None => Ok(None),
        }
    }

    pub fn get_medication_by_name_any(&self, name: &str) -> Result<Option<Medication>> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM medications WHERE name = ?1 ORDER BY active DESC LIMIT 1"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map(params![name], |row| map_row!(row))?;
        match rows.next() {
            Some(row) => Ok(Some(row_to_medication(row?)?)),
            None => Ok(None),
        }
    }

    pub fn list_medications(&self, include_stopped: bool) -> Result<Vec<Medication>> {
        let sql = if include_stopped {
            format!("SELECT {SELECT_COLS} FROM medications ORDER BY name ASC")
        } else {
            format!("SELECT {SELECT_COLS} FROM medications WHERE active = 1 ORDER BY name ASC")
        };
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| map_row!(row))?;

        let mut meds = Vec::new();
        for row in rows {
            meds.push(row_to_medication(row?)?);
        }
        Ok(meds)
    }

    pub fn stop_medication(
        &self,
        name: &str,
        stopped_at: DateTime<Utc>,
        reason: Option<&str>,
    ) -> Result<bool> {
        let count = self.conn.execute(
            "UPDATE medications SET active = 0, stopped_at = ?1, stop_reason = ?2
             WHERE name = ?3 AND active = 1",
            params![stopped_at.to_rfc3339(), reason, name],
        )?;
        Ok(count > 0)
    }

    pub fn remove_medication(&self, name: &str) -> Result<bool> {
        let count = self
            .conn
            .execute("DELETE FROM medications WHERE name = ?1", params![name])?;
        Ok(count > 0)
    }
}
