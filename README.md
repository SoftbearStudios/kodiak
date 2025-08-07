# Kodiak

[![Build](https://github.com/SoftbearStudios/kodiak/actions/workflows/build.yml/badge.svg)](https://github.com/SoftbearStudios/kodiak/actions/workflows/build.yml) 

Kodiak is a custom game engine; we use it in all of our games.

## Key features

- Client framework (`yew`)
- Server framework (`axum` + `actix` actors)
- WebGL 1.0 or 2.0 support
- 2D and/or 3D support
- WebSockets (`tokio-websockets`)
- WebTransport (`wtransport`)
- Multiple state synchronization models
- Metrics

## Key libraries

* `client` - Library for WebAssembly game client
* `common` - Library for both game client and game server
* `macros` - Procedural macros
* `plasma_protocol` - Library for game server and backend microservice
* `server` - Library for game server

## Developer tools

(The following are not required when building the game client or game server.)

* `manifest` - Manifest re-building utility
* `sprite_sheet_util` - Sprite sheet re-building utility
* `uploader` - New phrase uploader utility (for new translations)

Note: we're still preparing dev tools to be open-sourced.

## Notice

Certain features, like chat, are tied to our backend microservice. We might add
polyfills in the future.
