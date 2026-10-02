# Herdr Input Source Router

[English](README.md)

Herdrのペイン間でフォーカスを移動したときに、macOSの入力ソースを切り替えるプラグインです。

切り替え規則は次のとおりです。

- 最初にフォーカスされたペインでは、現在の入力ソースを維持します。
- 新しいペインでは、入力ソースを`ABC`に切り替えます。
- ペインからフォーカスが外れると、その時点の入力ソースをペインごとに記録します。
- 以前使ったペインへ戻ると、記録されている入力ソースを復元します。

## 必要な環境

- macOS
- Herdr 0.7.4以降
- [`macism`](https://github.com/laishulu/macism)
- ソースからインストールする場合はRustツールチェーン

## インストール

```sh
brew tap laishulu/homebrew
brew install macism
herdr plugin install asonas/herdr-input-source-router --ref v0.2.0 --yes
```

## ローカル開発

```sh
mise exec -- cargo build
command cp target/debug/herdr-input-source-router bin/herdr-input-source-router
mise exec -- cargo test
herdr plugin link .
```

現在の記録状態は、次のコマンドで確認できます。

```sh
herdr plugin action invoke status --plugin asonas.input-source-router
```
