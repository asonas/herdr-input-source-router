# Herdr Input Source Router

A macOS plugin that changes the input source when focus moves between Herdr panes.

The initial routing policy is intentionally small:

- New panes start with the `ABC` input source.
- Moving from a Codex pane to a plain shell selects `ABC`.
- Moving between Codex panes leaves the current input source unchanged.
- Other focus transitions leave the current input source unchanged.

## Requirements

- macOS
- Herdr 0.7.4 or newer
- [`macism`](https://github.com/laishulu/macism)
- A Rust toolchain when installing from source

## Local development

```sh
mise exec -- cargo build
command cp target/debug/herdr-input-source-router bin/herdr-input-source-router
mise exec -- cargo test
herdr plugin link .
```

Inspect the current routing state with:

```sh
herdr plugin action invoke status --plugin asonas.input-source-router
```
