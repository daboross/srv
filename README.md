srv: screeps room view
======================
[![Build Status][travis-image]][travis-builds]

A TUI application allowing viewing [Screeps] servers/rooms.

Screeps is a true programming MMO where users uploading JavaScript code to power their online empires.

Uses [rust-screeps-api] under the hood.

## Screenshot

![screenshot image of srv](./docs/screenshot.png)

## Building

Run debug build
```
cargo run -- --help
```

Compile release build
```
cargo build --release
./target/release/srv --help
```

[travis-image]: https://travis-ci.org/daboross/srv-cli.svg?branch=master
[travis-builds]: https://travis-ci.org/daboross/srv-cli
[appveyor-builds]: https://ci.appveyor.com/project/daboross/srv-cli
[screeps]: https://screeps.com
[rust-screeps-api]: https://github.com/daboross/rust-screeps-api
