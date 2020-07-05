use bytes::Bytes;
use screeps_api::RoomName;
use structopt::StructOpt;

fn bytes_from_str(v: &str) -> Bytes {
    Bytes::copy_from_slice(v.as_bytes())
}

#[derive(Clone, Debug, StructOpt)]
#[structopt(name = "srv", about = "screeps room view client")]
pub struct Config {
    /// A token to authentication to the server with
    #[structopt(short = "t", long = "token", parse(from_str = bytes_from_str))]
    pub auth_token: Bytes,
    /// The server to connect to (default is https://screeps.com/api/)
    #[structopt(short = "u", long = "server")]
    pub server: Option<String>,
    /// The shard to watch the room on - must be specified for the default server
    #[structopt(short = "s", long = "shard")]
    pub shard: Option<String>,
    /// The room to watch
    #[structopt(short = "r", long = "room", parse(try_from_str = RoomName::new))]
    pub room: Option<RoomName>,
    /// Increase log verbosity
    #[structopt(short = "v", parse(from_occurrences))]
    pub verbosity: u64,
    /// Disable UI
    #[structopt(short = "d", long = "dry-run")]
    pub dry_run: bool,
}

pub fn setup() -> Config {
    let conf = Config::from_args();

    crate::logging::setup_logging(conf.verbosity);

    return conf;
}
