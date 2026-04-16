# CCE

CCE is a Rust router for opening code in the right `tmux` session and `nvim` instance on macOS.

It does four things:

- routes file opens into the tmux session and worktree of a matching live Codex thread
- otherwise prefers an existing tmux session before falling back to plain Neovim
- provides a terminal dashboard so you can inspect the route before executing it
- installs a tiny macOS shim app so Finder and LaunchServices can forward file opens into the CLI

## Why this rewrite

The original implementation lived in dotfiles and used Python for routing. This repo splits the core routing model from the CLI and adds a dashboard UI so the behavior is easier to reason about, test, and publish.

## Commands

- `cce open [--wait] [--line N] [--column N] <path...>`
- `cce dashboard [<path...>]`
- `cce <session-id-or-thread-name> [--cwd <path>]`
- `cce install-macos`
- `cce shell-init zsh|bash`

## Install

```sh
cargo install --path crates/cce-cli
```

Optional local setup:

```sh
mkdir -p ~/.local/bin
ln -sf "$HOME/src/cce/bin/cce" ~/.local/bin/cce
ln -sf "$HOME/src/cce/bin/dev-editor" ~/.local/bin/dev-editor
ln -sf "$HOME/src/cce/bin/dev-editor-open" ~/.local/bin/dev-editor-open
ln -sf "$HOME/src/cce/bin/zed" ~/.local/bin/zed
```

To install the macOS app shims:

```sh
cce install-macos
```

To keep live Codex worktrees discoverable from your shell:

```sh
eval "$(cce shell-init zsh)"
```

## Dashboard

Run `cce` with no arguments, or run `cce dashboard <path>`.

Keys:

- `Enter` or `o`: execute the proposed route
- `r`: refresh the route snapshot
- `q`: quit

## Development

```sh
cargo fmt
cargo test
cargo run -p cce -- dashboard .
```

## License

MIT
