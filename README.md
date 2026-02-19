# Template Rust

A forkable template for Rust projects. Start a new project with linting, formatting, and testing already configured—just rename and customize.

## Quick Start

```bash
# Fork on GitHub, then clone your fork
git clone git@github.com:YOUR_USERNAME/YOUR_PROJECT.git
cd YOUR_PROJECT

# Set up environment
just bootstrap
just test
```

## After Forking

Rename the package to match your project:

1. Update `name` in `Cargo.toml`
2. Update `pname`, `description`, and `mainProgram` in `flake.nix`

Then start building:

- Add modules to `src/`
- Add dependencies with `cargo add`

## Development

```bash
just              # Show available recipes
just bootstrap    # Build the project
just check        # Lint and format check
just fmt          # Auto-format code
just test         # Run tests
just run          # Run the project
just build        # Build release binary
```

## Tech Stack

[Rust](https://www.rust-lang.org/) • [Clippy](https://github.com/rust-lang/rust-clippy) • [dprint](https://dprint.dev/) • [Nix](https://nixos.org/)
