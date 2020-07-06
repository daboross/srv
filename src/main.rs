use cursive::{Cursive, CursiveExt};

use log::debug;
use srv::{config, net, ui};

fn main() {
    let config = config::setup();

    let mut siv = if config.dry_run {
        Cursive::dummy()
    } else {
        Cursive::default()
    };
    ui::setup(&mut siv);
    net::spawn(config.clone(), siv.cb_sink().clone());

    debug!("running srv ui");
    siv.run();
    debug!("srv ui exited normally");
}
