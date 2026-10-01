# Herdr Input Source Router

A macOS plugin that changes the input source when focus moves between Herdr panes.

The routing policy is intentionally small:

- The first focused pane keeps its current input source.
- New panes start with the `ABC` input source.
- Each pane remembers the last input source used before focus moved away.
- Returning to a pane restores its remembered input source.

## Requirements

- macOS
- Herdr 0.7.4 or newer
- [`macism`](https://github.com/laishulu/macism)
- A Rust toolchain when installing from source

## Install

```sh
brew tap laishulu/homebrew
brew install macism
herdr plugin install asonas/herdr-input-source-router --ref v0.2.0 --yes
```

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
