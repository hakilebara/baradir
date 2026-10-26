mod db;
mod executor;
mod routes;
mod runtime;
mod server;
mod slug;
mod wit;

use std::path::PathBuf;

pub use db::Db;
pub use server::run;

#[derive(Clone, Copy, Debug)]
pub enum Access {
    Public,
    Private,
}

pub struct Config {
    public_addr: String,
    private_addr: Option<String>,
    tcp_port: u16,
    base_domain: String,
    data_dir: PathBuf,
}

impl Config {
    pub fn new(
        public_addr: String,
        private_addr: Option<String>,
        base_domain: String,
        data_dir: PathBuf,
    ) -> anyhow::Result<Config> {
        let tcp_port = public_addr
            .rsplit(':')
            .next()
            .and_then(|port| port.parse().ok())
            .ok_or_else(|| {
                anyhow::anyhow!("invalid public address `{public_addr}`: expected host:port")
            })?;

        if let Some(addr) = &private_addr {
            addr.rsplit(':')
                .next()
                .and_then(|port| port.parse::<u16>().ok())
                .ok_or_else(|| {
                    anyhow::anyhow!("invalid private address `{addr}`: expected host:port")
                })?;
        }

        Ok(Config {
            public_addr,
            private_addr,
            tcp_port,
            base_domain,
            data_dir,
        })
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("db.sqlite")
    }

    pub fn data_dir(&self) -> PathBuf {
        self.data_dir.clone()
    }

    pub fn environments_dir(&self) -> PathBuf {
        self.data_dir.join("environments")
    }

    pub fn registry_dir(&self) -> PathBuf {
        self.data_dir.join("registry")
    }
}
