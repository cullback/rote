# Display available recipes
default:
    just --list --unsorted

# Install dependencies and set up the development environment
bootstrap:
    cargo build

alias fmt := format

# Format code
format:
    dprint fmt
    cargo fmt
    fd -e nix | xargs -r nixfmt
    rg -l '[^\n]\z' --multiline | xargs -r sed -i -e '$a\\'

# Run linters and static analysis
check:
    #!/usr/bin/env fish
    set status_flag 0
    dprint check; or set status_flag 1
    cargo fmt --check; or set status_flag 1
    cargo clippy -- -D warnings; or set status_flag 1
    fd -e nix | xargs -r nixfmt --check; or set status_flag 1
    ! rg -l '[^\n]\z' --multiline; or set status_flag 1
    exit $status_flag

# Run the test suite
test:
    cargo test

# Build release binary
build:
    cargo build --release

# Run the project
run *args:
    cargo run -- {{ args }}

# Start the web UI (watches for changes and restarts)
serve *args:
    watchexec -r -e rs,html,css,js -- cargo run -- serve {{ args }}
