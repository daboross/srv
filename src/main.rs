use cursive::Cursive;

use srv::{config, net, ui};

fn main() {
    let config = config::setup();

    let mut siv = if config.dry_run {
        Cursive::dummy()
    } else {
        Cursive::termion().unwrap()
    };
    ui::setup(&mut siv);
    net::spawn(config.clone(), siv.cb_sink().clone());

    siv.run();

    loop {
        std::thread::yield_now();
    }
}
