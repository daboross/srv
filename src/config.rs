use bytes::Bytes;
use screeps_api::RoomName;
use structopt::StructOpt;

#[derive(Clone, Debug, StructOpt)]
#[structopt(name = "srv", about = "screeps room view client")]
pub struct Config {
    /// A token to authentication to the server with
    #[structopt(short = "t", long = "token", parse(from_str))]
    pub auth_token: Bytes,
    /// The server to connect to (default is https://screeps.com/api/)
    #[structopt(short = "u", long = "server")]
    pub server: Option<String>,
    /// The shard to watch the room on - must be specified for the default server
    #[structopt(short = "s", long = "shard")]
    pub shard: Option<String>,
    /// The room to watch
    #[structopt(short = "r", long = "room", parse(try_from_str = "RoomName::new"))]
    pub room: RoomName,
    /// Increase log verbosity
    #[structopt(short = "v", parse(from_occurrences))]
    pub verbosity: u64,
    /// Disable UI
    #[structopt(short = "d", long = "dry-run")]
    pub dry_run: bool,
}

fn setup_logging(verbosity: u64) {
    let log_level = match verbosity {
        0 => log::LevelFilter::Info,
        1 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };
    fern::Dispatch::new()
        .level(log_level)
        .level_for("rustls", log::LevelFilter::Warn)
        .level_for("hyper", log::LevelFilter::Warn)
        .format(|out, message, record| {
            let now = chrono::Local::now();

            out.finish(format_args!("[{}][{}] {}: {}",
                                    now.format("%H:%M:%S"),
                                    record.level(),
                                    record.target(),
                                    message));
        })
        .chain(fern::log_file("srv.log").unwrap())
        .apply()
        // ignore errors
        .unwrap_or(());

    // log panics
    log_panics::init();
}

pub fn setup() -> Config {
    let conf = Config::from_args();

    setup_logging(conf.verbosity);

    return conf;
}
