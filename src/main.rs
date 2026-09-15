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
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let conn = get_db_conn()?;
    let config = Config::new(args.port, conn, args.env_folder);
    run(config).await
}
