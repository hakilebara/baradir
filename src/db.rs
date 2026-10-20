use rusqlite::{Connection, Error, params};

use crate::runtime::{App, NewApp};
pub struct Db {
    // TODO: make this field private once all db access have been moved here
    conn: Connection,
}

pub struct AppRow {
    pub id: i64,
    pub name: String,
    pub filepath: String,
}

impl Db {
    pub fn open(path: &str) -> Result<Self, Error> {
        let conn = rusqlite::Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        Ok(Db { conn })
    }

    pub fn init_schema(&mut self) -> Result<(), Error> {
        let sql = "
        CREATE TABLE IF NOT EXISTS environment (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            slug TEXT NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS app (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            filepath TEXT NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS environment_app (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            environment_id INTEGER NOT NULL,
            app_id INTEGER NOT NULL,
            FOREIGN KEY (environment_id) REFERENCES environment(id) ON DELETE CASCADE,
            FOREIGN KEY (app_id) REFERENCES app(id) ON DELETE CASCADE,
            UNIQUE (environment_id, app_id)
        );";
        self.conn.execute_batch(sql)?;
        Ok(())
    }

    pub fn seed_db(&mut self) -> Result<(), Error> {
        self.conn.execute_batch(
            "
        INSERT OR IGNORE INTO app (name, filepath)
        VALUES ('hello', 'target/wasm32-wasip2/release/hello.wasm');

        INSERT OR IGNORE INTO app (name, filepath)
        VALUES ('manager', 'target/wasm32-wasip2/release/manager.wasm');
            ",
        )?;
        Ok(())
    }

    pub fn get_apps(&self) -> Result<Vec<AppRow>, Error> {
        let mut stmt = self.conn.prepare("SELECT id, name, filepath FROM app")?;
        stmt.query_map([], |row| {
            Ok(AppRow {
                id: row.get(0)?,
                name: row.get(1)?,
                filepath: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
    }

    pub fn get_app_names(&self) -> Result<Vec<String>, Error> {
        let mut names = vec![];
        let mut stmt = self.conn.prepare(
            "SELECT name FROM app
                ORDER BY name",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            names.push(row.get(0)?);
        }
        Ok(names)
    }

    pub fn add_app_to_environment(&mut self, app: NewApp, env: &str) -> Result<App, Error> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "
            INSERT INTO app (name, filepath)
            VALUES (?1, ?2)",
            params![app.name, app.filepath],
        )?;
        let app_id = tx.last_insert_rowid();
        let env_id: i64 = tx.query_row(
            "SELECT id FROM environment WHERE slug = (?1)",
            params![env],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO environment_app (environment_id, app_id) VALUES (?1, ?2)",
            params![env_id, app_id],
        )?;

        tx.commit()?;

        Ok(App {
            id: app_id,
            name: app.name,
            filepath: app.filepath,
            pre: None,
        })
    }

    pub fn app_exists(&self, env_slug: &str, app_name: &str) -> bool {
        self.conn
            .query_row(
                "SELECT * FROM environment_app ea
                JOIN environment e ON e.id = ea.environment_id
                JOIN app a ON a.id = ea.app_id
                WHERE e.slug = ?1 AND a.name = ?2",
                params![env_slug, app_name],
                |_| Ok(()),
            )
            .is_ok()
    }

    pub fn env_exists(&self, env_slug: &str) -> bool {
        self.conn
            .query_row(
                "SELECT * FROM environment
                WHERE slug = ?1",
                params![env_slug],
                |_| Ok(()),
            )
            .is_ok()
    }

    pub fn create_environment(&mut self, domain_slug: &str) -> Result<(), Error> {
        let tx = self.conn.transaction()?;

        tx.execute(
            "INSERT INTO environment (slug)
            VALUES (?1)",
            params![domain_slug],
        )?;
        let env_id = tx.last_insert_rowid();

        let manager_id: i64 = tx.query_row(
            "SELECT id FROM app
            WHERE name = ?1",
            ["manager"],
            |row| row.get(0),
        )?;

        let hello_id: i64 = tx.query_row(
            "SELECT id FROM app
            WHERE name = ?1",
            ["hello"],
            |row| row.get(0),
        )?;

        tx.execute(
            "INSERT INTO environment_app (environment_id, app_id)
            VALUES (?1, ?2)",
            params![env_id, manager_id],
        )?;

        tx.execute(
            "INSERT INTO environment_app (environment_id, app_id)
            VALUES (?1, ?2)",
            params![env_id, hello_id],
        )?;

        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn count_env_table_entries(db: &Db) -> Result<i64, Error> {
        let sql = "SELECT COUNT(*) FROM environment_app";
        db.conn.query_row(sql, [], |row| row.get(0))
    }

    #[test]
    fn can_create_new_env() -> anyhow::Result<()> {
        let mut db = Db::open(":memory:")?;
        db.init_schema()?;
        db.seed_db()?;

        assert_eq!(count_env_table_entries(&db)?, 0);

        db.create_environment(&"fake_slug")?;

        assert_eq!(count_env_table_entries(&db)?, 2);

        Ok(())
    }

    #[test]
    fn return_list_of_app_names() -> anyhow::Result<()> {
        let mut db = Db::open(":memory:")?;
        db.init_schema()?;
        db.seed_db()?;
        assert_eq!(db.get_app_names()?, vec!["hello", "manager"]);
        Ok(())
    }

    // TODO: Complete test once add_app_to_env exists
    // fn can_tell_if_app_exists() -> () {
    //     let db = Db::open(":memory:")?;
    //     db.init_schema()?;
    //     todo!();
    // }
}
