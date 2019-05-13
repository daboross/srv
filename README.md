srv: screeps room view
======================
[![Linux Build Status][travis-image]][travis-builds]
[![Windows Build Status][appveyor-image]][appveyor-builds]

A TUI application allowing viewing [Screeps] servers/rooms.

Screeps is a true programming MMO where users uploading JavaScript code to power their online empires.

Uses [rust-screeps-api] under the hood.

## Building

```
cd srv-cli
cargo build --release
cargo run --release -- --help
```

[travis-image]: https://travis-ci.org/daboross/srv-cli.svg?branch=master
[travis-builds]: https://travis-ci.org/daboross/srv-cli
[appveyor-image]: https://ci.appveyor.com/api/projects/status/github/daboross/srv-cli?branch=master&svg=true
[appveyor-builds]: https://ci.appveyor.com/project/daboross/srv-cli
[screeps]: https://screeps.com
[rust-screeps-api]: https://github.com/daboross/rust-screeps-api
