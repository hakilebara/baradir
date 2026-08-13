mod db;
use baradir::Config;
use baradir::run;
use clap::Parser;
use wasmtime::Result;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// TCP port to listen to
    #[arg(short, long, default_value_t = 8080)]
    pub port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config = Config::new(args.port);
    run(config).await
}
