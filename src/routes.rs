use crate::db::Db;
use crate::Access;

#[derive(Debug, PartialEq)]
pub enum Route {
    Manager { env: String },
    Component { app: String, env: String },
    Admin,
    WebHost,
    NotFound,
    Forbidden,
}

fn is_valid_manager(app_name: &str, env_slug: &str, db: &Db) -> bool {
    app_name == "manager" && db.env_exists(env_slug)
}

pub fn route(access: Access, host: Option<&str>, base_domain: &str, db: &Db) -> Route {
    let Some(host) = host else {
        return Route::NotFound;
    };

    // remove port info if any
    let host = host.split_once(':').map_or(host, |(h, _port)| h);

    // <prefix>.<base-domain>
    let prefix = host.strip_suffix(base_domain).and_then(|p| {
        if p.is_empty() {
            Some(p)
        } else {
            p.strip_suffix('.')
        }
    });

    let Some(prefix) = prefix else {
        return Route::NotFound;
    };

    let sub_domains: Vec<&str> = prefix.split('.').collect();

    match (access, sub_domains.as_slice()) {
        // <manager>.<env_slug>.<base-domain>
        (Access::Private, [app_name, env_slug]) if is_valid_manager(app_name, env_slug, db) => {
            Route::Manager {
                env: env_slug.to_string(),
            }
        }
        (Access::Public, [app_name, env_slug]) if is_valid_manager(app_name, env_slug, db) => {
            Route::Forbidden
        }

        // <app_name>.<env_slug>.<base-domain>
        (_, [app_name, env_slug]) if !db.app_exists(env_slug, app_name) => Route::NotFound,
        (_, [app_name, env_slug]) => Route::Component {
            app: app_name.to_string(),
            env: env_slug.to_string(),
        },

        // <base-domain>
        (Access::Public, [""]) => Route::Forbidden,
        (Access::Private, [""]) => Route::Admin,

        // <env_slug>.<base-domain>
        (_, [env_slug]) if db.env_exists(env_slug) => Route::WebHost,

        // other
        _ => Route::NotFound,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn test_db() -> Db {
        let mut db = Db::open(":memory:").unwrap();
        db.init_schema().unwrap();
        db.create_environment("test-env").unwrap();
        db.add_app_to_environment(
            crate::runtime::NewApp {
                name: "demo".into(),
                digest: "deadbeef".into(),
            },
            "test-env",
        )
        .unwrap();
        db
    }

    fn route_for(access: Access, host: &str) -> Route {
        route(access, Some(host), "baradir.localhost", &test_db())
    }

    #[test]
    fn public_app_route_is_component() {
        assert_eq!(
            route_for(Access::Public, "demo.test-env.baradir.localhost"),
            Route::Component {
                app: "demo".into(),
                env: "test-env".into(),
            }
        );
    }

    #[test]
    fn bare_domain_public_is_forbidden() {
        assert_eq!(
            route_for(Access::Public, "baradir.localhost"),
            Route::Forbidden
        );
    }

    #[test]
    fn bare_domain_private_is_admin() {
        assert_eq!(
            route_for(Access::Private, "baradir.localhost"),
            Route::Admin
        );
    }

    #[test]
    fn suffix_without_dot_boundary_is_not_found() {
        assert_eq!(
            route_for(Access::Public, "evilbaradir.localhost"),
            Route::NotFound
        );
    }
    #[test]
    fn prefix_with_double_dot_is_not_found() {
        assert_eq!(
            route_for(Access::Public, "demo..baradir.localhost"),
            Route::NotFound
        );
    }

    #[test]
    fn private_socket_serves_demo() {
        assert_eq!(
            route_for(Access::Private, "demo.test-env.baradir.localhost"),
            Route::Component {
                app: "demo".into(),
                env: "test-env".into(),
            }
        );
    }

    #[test]
    fn private_socket_serves_manager() {
        assert_eq!(
            route_for(Access::Private, "manager.test-env.baradir.localhost"),
            Route::Manager {
                env: "test-env".into(),
            }
        );
    }

    #[test]
    fn public_manager_is_forbidden() {
        assert_eq!(
            route_for(Access::Public, "manager.test-env.baradir.localhost"),
            Route::Forbidden
        );
    }

    #[test]
    fn private_manager_unknown_env_is_not_found() {
        assert_eq!(
            route_for(Access::Private, "manager.nope.baradir.localhost"),
            Route::NotFound
        );
    }

    #[test]
    fn env_host_routes_to_web_host() {
        assert_eq!(
            route_for(Access::Public, "test-env.baradir.localhost"),
            Route::WebHost
        );
        assert_eq!(
            route_for(Access::Private, "test-env.baradir.localhost"),
            Route::WebHost
        );
    }

    #[test]
    fn host_with_port_routes_to_component() {
        assert_eq!(
            route_for(Access::Public, "demo.test-env.baradir.localhost:8080"),
            Route::Component {
                app: "demo".into(),
                env: "test-env".into(),
            }
        );
    }

    #[test]
    fn unknown_env_is_not_found() {
        assert_eq!(
            route_for(Access::Public, "demo.nope.baradir.localhost"),
            Route::NotFound
        );
    }

    #[test]
    fn unknown_app_is_not_found() {
        assert_eq!(
            route_for(Access::Public, "nope.test-env.baradir.localhost"),
            Route::NotFound
        );
    }

    #[test]
    fn three_label_host_is_not_found() {
        assert_eq!(
            route_for(Access::Public, "a.b.c.baradir.localhost"),
            Route::NotFound
        );
    }

    #[test]
    fn non_matching_base_domain_is_not_found() {
        assert_eq!(
            route_for(Access::Public, "demo.test-env.other.example"),
            Route::NotFound
        );
    }

    #[test]
    fn missing_host_is_not_found() {
        assert_eq!(
            route(Access::Public, None, "baradir.localhost", &test_db()),
            Route::NotFound
        );
        assert_eq!(
            route(Access::Private, None, "baradir.localhost", &test_db()),
            Route::NotFound
        );
    }

    #[test]
    fn unknown_naked_env_is_not_found() {
        assert_eq!(
            route(
                Access::Public,
                Some("nope.baradir.localhost"),
                "baradir.localhost",
                &test_db()
            ),
            Route::NotFound
        );
    }

    #[test]
    fn private_raw_ip_is_not_found() {
        assert_eq!(route_for(Access::Private, "127.0.0.1"), Route::NotFound);
    }
}
