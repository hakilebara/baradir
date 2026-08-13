// use rusqlite::{params, Connection, Error, Result};
use rusqlite::{Connection, Error};

const SCHEMA: &str = "PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS environment (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS app (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    filepath TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS environment_app (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    environment_id INTEGER NOT NULL,
    app_id INTEGER NOT NULL,
    FOREIGN KEY (environment_id) REFERENCES environment(id) ON DELETE CASCADE,
    FOREIGN KEY (app_id) REFERENCES app(id) ON DELETE CASCADE,
    UNIQUE (environment_id, app_id)
);";

pub fn get_db_conn() -> Result<Connection, Error> {
    let conn = Connection::open("db.sqlite")?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}
