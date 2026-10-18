# Baradir


> **Warning**: Baradir is experimental and not ready for production yet.

Baradir is an application platform for [WebAssembly Components](https://component-model.bytecodealliance.org/).
It lets you host multiple wasm components as apps, organize them into isolated
environments, and serve each app over HTTP under its own subdomain — with the
apps themselves implemented as plain `wasi:http/proxy` components.


## Technical Overview

Baradir is a Rust host built on [Wasmtime](https://wasmtime.dev/) and
[Hyper](https://hyper.rs/). It serves WASI 0.2 (`wasi:http/proxy`) components
behind subdomain routing.

- Requests to `{app}.{slug}.{base-domain}:{port}` are served by the component
  named `app` for the environment identified by `slug`.
- Visiting the bare domain creates a new environment with a random slug,
  seeds it with the `manager` and `hello` apps, and redirects to
  `manager.{slug}.{base-domain}:{port}`.
- Components call back into the host through a WIT interface to list apps,
  get their domain, and install new apps from a wasm binary.
- State lives in SQLite (`./data/db.sqlite`) and installed components in
  `./workspaces/{slug}/{app}/`.

## Prerequisites

- Stable Rust
- The `wasm32-wasip2` target:

  ```
  rustup target add wasm32-wasip2
  ```

## Build & run

```
cargo build -p manager -p hello --release --target wasm32-wasip2
cargo run
```

CLI flags:

| flag | default | description |
| --- | --- | --- |
| `--public-addr` | `127.0.0.1:8080` | public address:port to listen on |
| `--private-addr` | `None` | private address:port to listen on |
| `--base-domain` | `baradir.localhost` | base domain used for routing |
| `--env-folder` | `./workspaces` | root folder of environments |

## Try it

```
curl -i http://baradir.localhost:8080/
```

The server responds with a 302 to `http://manager.{slug}.baradir.localhost:8080`,
the dashboard for the newly created environment. From there you can:

- open each installed app from the card list
- upload a new `.wasm` component, which is compiled and served immediately at
  `http://{app}.{slug}.baradir.localhost:8080`

`*.localhost` resolves to loopback instantly (RFC 6761), so no `/etc/hosts`
edits are needed.
