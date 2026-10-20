use crate::Access;
use crate::routes::{Route, route};
use crate::runtime::Runtime;
use crate::slug::generate_slug;
use http_body_util::BodyExt;
use std::sync::Arc;
use wasmtime::Result;
use wasmtime_wasi_http::p2::body::HyperOutgoingBody;

impl Runtime {
    pub async fn handle_request(
        self: Arc<Self>,
        req: hyper::Request<hyper::body::Incoming>,
        access: Access,
    ) -> Result<hyper::Response<HyperOutgoingBody>> {
        let route = route(
            access,
            req.headers().get("host").map(|h| h.to_str()).transpose()?,
            &self.config.base_domain,
            &self.db.lock().unwrap(),
        );

        return match route {
            Route::Component { app, env } => self.execute(req, app, env).await,
            Route::Admin => {
                todo!("implement admin");
            }
            Route::WebHost => {
                todo!("implement web hosting");
            }
            Route::NotFound => {
                // TODO: return a 404
                panic!("404 APP not found");
            }
            Route::Forbidden => {
                // TODO: return a 403
                panic!("403 Access Forbidden");
            }
        };
    }

    fn _generate_environment(self: Arc<Self>) -> Result<hyper::Response<HyperOutgoingBody>> {
        let mut db = self.db.lock().unwrap();

        let domain_slug = generate_slug();
        db.create_environment(&domain_slug)?;

        // redirect to the subdomain of the newly created environment
        println!(
            "redirecting to mgr.{}.{}:{}",
            &domain_slug, &self.config.base_domain, &self.config.tcp_port
        );
        Ok(hyper::Response::builder()
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
            )?)
    }
}
