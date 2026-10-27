use std::path::PathBuf;

use baradir::Config;
use baradir::Db;
use baradir::run;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Public listen address, e.g. 0.0.0.0:8080
    #[arg(short, long, default_value_t = String::from("0.0.0.0:8080"))]
    pub public_addr: String,

    /// Private listen address, unset to disable
    #[arg(long)]
    pub private_addr: Option<String>,

    /// Base domain
    #[arg(short, long, default_value_t = "baradir.localhost".to_string())]
    pub base_domain: String,

    /// Path to data folder
    #[arg(short, long, default_value = "./data")]
    pub data_dir: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = Config::new(
        args.public_addr,
        args.private_addr,
        args.base_domain,
        args.data_dir,
    )?;

    std::fs::create_dir_all(config.data_dir())?;
    let mut db = Db::open(config.db_path())?;
    db.init_schema()?;

    run(config, db).await
}
