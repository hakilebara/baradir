use crate::Access;
use crate::runtime::Runtime;
use crate::slug::generate_slug;
use http_body_util::BodyExt;
use rusqlite::params;
use std::sync::Arc;
use wasmtime::Result;
use wasmtime_wasi_http::p2::body::HyperOutgoingBody;

impl Runtime {
    pub async fn handle_request(
        self: Arc<Self>,
        req: hyper::Request<hyper::body::Incoming>,
        _access: Access,
    ) -> Result<hyper::Response<HyperOutgoingBody>> {
        let raw_host = req.headers().get("host").unwrap().to_str().unwrap();
        let v: Vec<_> = raw_host.split('.').take(2).collect();

        let host = raw_host.split(":").next().unwrap();

        let is_root = host == self.config.base_domain;

        let app_name = v[0].to_string();
        let env_slug = v[1].to_string();

        /*
         * If a request targets the naked domain of baradir
         * we create a new environment and redirect the user to it
         */
        if is_root {
            // Create new environment
            let domain_slug = generate_slug();
            let mut conn = self.conn.lock().unwrap();
            let tx = conn.transaction()?;

            tx.execute(
                "INSERT INTO environment (slug) VALUES (?1)",
                params![domain_slug],
            )?;
            let env_id = tx.last_insert_rowid();

            let manager_id: i64 =
                tx.query_row("SELECT id FROM app WHERE name = 'manager'", [], |row| {
                    row.get(0)
                })?;

            let hello_id: i64 =
                tx.query_row("SELECT id FROM app WHERE name = 'hello'", [], |row| {
                    row.get(0)
                })?;

            tx.execute(
                "INSERT INTO environment_app (environment_id, app_id) VALUES (?1, ?2)",
                params![env_id, manager_id],
            )?;

            tx.execute(
                "INSERT INTO environment_app (environment_id, app_id) VALUES (?1, ?2)",
                params![env_id, hello_id],
            )?;

            let _ = tx.commit();

            // redirect to the subdomain of the newly created environment
            println!(
                "redirect to mgr.{}.{}:{}",
                domain_slug, self.config.base_domain, self.config.tcp_port
            );
            return Ok(hyper::Response::builder()
                // TODO: replace "location" and the return code with
                // the constants provided by the hyper library
                .header(
                    "Location",
                    format!(
                        "http://manager.{}.{}:{}",
                        domain_slug, self.config.base_domain, self.config.tcp_port
                    ),
                )
                .status(302)
                .body(
                    http_body_util::Empty::<bytes::Bytes>::new()
                        .map_err(|_| unreachable!("infaillable"))
                        .boxed_unsync(),
                )?);
        }

        // I have a env name and an app name from the HTTP host
        // I want to check that I can find this env/app relation in the environment_app table
        let exists: bool = {
            let conn = self.conn.lock().unwrap();
            conn.query_row(
                "SELECT * FROM environment_app ea
                JOIN environment e ON e.id = ea.environment_id
                JOIN app a ON a.id = ea.app_id
                WHERE e.slug = ?1 AND a.name = ?2",
                params![env_slug, app_name],
                |_| Ok(()),
            )
            .is_ok()
        };

        // TODO: return a 404
        if !exists {
            panic!("APP not found")
        }

        self.execute(req, app_name, env_slug).await
    }
}
