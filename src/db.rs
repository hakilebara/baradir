use std::path::Path;

use rusqlite::{params, Connection, Error};

use crate::runtime::NewApp;
pub struct Db {
    conn: Connection,
}

pub struct AppRow {
    pub name: String,
    pub digest: String,
}

impl Db {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
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
            digest TEXT NOT NULL
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

    pub fn get_apps(&self) -> Result<Vec<AppRow>, Error> {
        let mut stmt = self.conn.prepare("SELECT name, digest FROM app")?;
        stmt.query_map([], |row| {
            Ok(AppRow {
                name: row.get(0)?,
                digest: row.get(1)?,
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

    pub fn get_env_slugs(&self) -> Result<Vec<String>, Error> {
        let mut stmt = self.conn.prepare("SELECT slug FROM environment")?;
        stmt.query_map([], |row| Ok(row.get(0)?))?.collect()
    }

    pub fn add_app_to_environment(&mut self, app: NewApp, env: &str) -> Result<AppRow, Error> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "
            INSERT INTO app (name, digest)
            VALUES (?1, ?2)",
            params![app.name, app.digest],
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

        Ok(AppRow {
            name: app.name,
            digest: app.digest,
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
        self.conn.execute(
            "INSERT INTO environment (slug)
            VALUES (?1)",
            params![domain_slug],
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn count_env_app_links(db: &Db) -> Result<i64, Error> {
        db.conn
            .query_row("SELECT COUNT(*) FROM environment_app", [], |row| row.get(0))
    }

    #[test]
    fn can_create_new_env() -> anyhow::Result<()> {
        let mut db = Db::open(":memory:")?;
        db.init_schema()?;

        db.create_environment("fake_slug")?;

        assert!(db.env_exists("fake_slug"));
        // Manager is implicit: creating an environment links no app rows.
        assert_eq!(count_env_app_links(&db)?, 0);

        Ok(())
    }

    #[test]
    fn return_list_of_app_names() -> anyhow::Result<()> {
        let mut db = Db::open(":memory:")?;
        db.init_schema()?;
        db.create_environment("test-env")?;

        db.add_app_to_environment(
            NewApp {
                name: "bookshelf".into(),
                digest: "d1".into(),
            },
            "test-env",
        )?;
        db.add_app_to_environment(
            NewApp {
                name: "analytics".into(),
                digest: "d2".into(),
            },
            "test-env",
        )?;

        assert_eq!(db.get_app_names()?, vec!["analytics", "bookshelf"]);
        Ok(())
    }

    #[test]
    fn add_app_stores_digest_and_links_environment() -> anyhow::Result<()> {
        let mut db = Db::open(":memory:")?;
        db.init_schema()?;
        db.create_environment("test-env")?;

        let app = db.add_app_to_environment(
            NewApp {
                name: "bookshelf".into(),
                digest: "d1".into(),
            },
            "test-env",
        )?;

        assert_eq!(app.digest, "d1");
        assert!(db.app_exists("test-env", "bookshelf"));
        assert_eq!(db.get_apps()?[0].digest, "d1");
        Ok(())
    }
}
