srv: screeps room view
======================
[![Build Status][travis-image]][travis-builds]

A TUI application allowing viewing [Screeps] servers/rooms.

Screeps is a true programming MMO where users uploading JavaScript code to power their online empires.

Uses the [rust-screeps-api] library for networking.

Current features:
- viewing rooms
- defaulting to a user's owned room when starting up
- navigating around room with arrow keys or hjlk
- viewing some information about objects under the current cursor
  - completed: creeps, terrain


TODO:
- implement viewing detailed information about remaining object types
- implement more controls besides just "move around the room"

![screenshot image of srv](./docs/screenshot.png)

## Building

Requires nightly Rust. Tested with `rustc 1.36.0-nightly (372be4f36 2019-05-14)`.

Options:

- Install snapshot of repository into PATH

  ```
  cargo install --git https://github.com/daboross/srvc.git
  ```
- Install from cloned repository
  ```
  cargo install --path .
  ```
- Run directly from repository

 ```
 # debug mode (faster compile, slower runtime)
 cargo run -- --token 'my_auth_token'
 cargo run -- --help

 # release
 cargo run --release -- --help
 # or
 cargo build --release
 ./target/release/srv --help
 ```

[travis-image]: https://travis-ci.org/daboross/srvc.svg?branch=master
[travis-builds]: https://travis-ci.org/daboross/srvc
[screeps]: https://screeps.com
[rust-screeps-api]: https://github.com/daboross/rust-screeps-api
