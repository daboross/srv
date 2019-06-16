pub fn setup_logging(verbosity: u64) {
    let log_level = match verbosity {
        0 => log::LevelFilter::Info,
        1 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };
    fern::Dispatch::new()
        .level(log_level)
        .level_for("rustls", log::LevelFilter::Warn)
        .level_for("hyper", log::LevelFilter::Warn)
        .chain(
            fern::Dispatch::new()
                .format(|out, message, record| {
                    let now = chrono::Local::now();

                    out.finish(format_args!(
                        "[{}][{}] {}: {}",
                        now.format("%H:%M:%S"),
                        record.level(),
                        record.target(),
                        message
                    ));
                })
                .chain(fern::log_file("srv.log").unwrap()),
        )
        .chain(Box::new(cursive::logger::get_logger()) as Box<dyn log::Log>)
        .apply()
        // ignore errors
        .unwrap_or(());

    // log panics
    log_panics::init();
}
