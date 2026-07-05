# Surge

Surge is a terminal radio app.

It plays internet radio streams from the Radio Browser API.

## Start

Install Rust, then run:

```sh
cargo run --release
```

You need a working audio output device and network access.

## Controls

- `h` opens help
- `f` searches stations
- `Up` / `Down` moves through search results
- `Enter` plays the selected station
- `+` / `-` changes volume
- `m` mutes audio
- `q` quits

## Planned Features

- Support fuzzy search, search by country, etc.
- Support streams with channel counts other than the one accepted by the audio device.
- Support audio device output sample formats other than f32.
