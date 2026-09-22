mod db;
use baradir::Config;
use baradir::run;
use clap::Parser;
use db::get_db_conn;
use wasmtime::Result;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// TCP port to listen to
    #[arg(short, long, default_value_t = 8080)]
    pub port: u16,
    /// Root folder of environments
    #[arg(short, long, default_value_t = "./workspaces".to_string())]
    pub env_folder: String,
    /// Base domain
    #[arg(short, long, default_value_t = "baradir.localhost".to_string())]
    pub base_domain: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config = Config::new(args.port, args.base_domain, args.env_folder);
    let conn = get_db_conn()?;
    run(config, conn).await
}
