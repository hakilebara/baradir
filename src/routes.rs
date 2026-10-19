use crate::Access;
use crate::db::Db;

#[derive(Debug)]
pub enum Route {
    Component { app: String, env: String },
    Admin,
    BaseDomain,
    NotFound,
    Forbidden,
}

const PRIVATE: [&str; 1] = ["manager"];

pub fn route(access: Access, host: Option<&str>, base_domain: &str, db: &Db) -> Route {
    if let Some(host) = host {
        let host = host.split_once(':').map_or(host, |(h, _port)| h); // remove port info if any

        let prefix = host
            .strip_suffix(base_domain)
            .map(|p| p.trim_end_matches('.'));

        let Some(prefix) = prefix else {
            return Route::NotFound;
        };

        if prefix.is_empty() {
            return Route::BaseDomain;
        }

        let sub_domains: Vec<&str> = prefix.split('.').collect();

        match sub_domains.as_slice() {
            [app_name, env_slug] => match access {
                Access::Private => Route::Admin,
                Access::Public => {
                    if !db.app_exists(env_slug, app_name) {
                        return Route::NotFound;
                    }
                    if PRIVATE.contains(app_name) {
                        return Route::Forbidden;
                    }
                    return Route::Component {
                        app: app_name.to_string(),
                        env: env_slug.to_string(),
                    };
                }
            },
            [_env_slug] => {
                return Route::NotFound;
            }
            _ => {
                return Route::NotFound;
            }
        }
    } else {
        Route::NotFound
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn test_db() -> Db {
        let mut db = Db::open(":memory:").unwrap();
        db.init_schema().unwrap();
        db.seed_db().unwrap();
        db.create_environment("test-env").unwrap();
        db
    }

    #[test]
    fn public_app_route_is_component() {
        let db = test_db();

        let route = route(
            Access::Public,
            Some("hello.test-env.baradir.localhost"),
            "baradir.localhost",
            &db,
        );

        match route {
            Route::Component { app, env } => {
                assert_eq!(app, "hello");
                assert_eq!(env, "test-env");
            }
            _ => panic!("expected Route::Component"),
        }
    }

    #[test]
    fn public_private_app_is_forbidden() {
        let db = test_db();

        let route = route(
            Access::Public,
            Some("manager.test-env.baradir.localhost"),
            "baradir.localhost",
            &db,
        );

        assert!(matches!(route, Route::Forbidden));
    }

    #[test]
    fn bare_domain_routes_to_base_domain() {
        let db = test_db();

        let route = route(
            Access::Public,
            Some("baradir.localhost"),
            "baradir.localhost",
            &db,
        );

        assert!(matches!(route, Route::BaseDomain));
    }
}
