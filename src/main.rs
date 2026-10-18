mod db;
use baradir::Config;
use baradir::run;
use clap::Parser;
use db::get_db_conn;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Public listen address, e.g. 0.0.0.0:8080
    #[arg(short, long, default_value_t = String::from("127.0.0.1:8080"))]
    pub public_addr: String,

    /// Private listen address, unset to disable
    #[arg(long)]
    pub private_addr: Option<String>,

    /// Root folder of environments
    #[arg(short, long, default_value_t = "./workspaces".to_string())]
    pub env_folder: String,

    /// Base domain
    #[arg(short, long, default_value_t = "baradir.localhost".to_string())]
    pub base_domain: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = Config::new(
        args.public_addr,
        args.private_addr,
        args.base_domain,
        args.env_folder,
    )?;
    let conn = get_db_conn()?;
    run(config, conn).await
}
