# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Build the WASM plugin (debug)
cargo build --target wasm32-unknown-unknown

# Build the WASM plugin (release)
cargo build --target wasm32-unknown-unknown --release

# Run tests
cargo test

# Check code without building
cargo check
```

## Architecture

This is an Extism WebAssembly plugin that integrates with Jackett (a torrent indexer API aggregator). It implements the `rs-plugin-common-interfaces` plugin system.

### Plugin Capabilities

The plugin exposes three main functions via `#[plugin_fn]`:

- **`infos()`** - Returns plugin metadata (name: "jackett_lookup", capabilities: Lookup + Request)
- **`lookup()`** - Searches Jackett for episodes or movies, returns torrent/magnet results
- **`process()`** / **`request_permanent()`** - Converts Jackett torrent links to magnet URIs

### Key Patterns

- Uses custom MIME type `jackett/torrent` to identify requests that need processing
- API tokens are replaced with `#token#` placeholder in stored URLs for security, then restored during processing
- Episode queries use format `{serie} s{season:02}e{episode:02}` with `unidecode` for ASCII normalization
- Torrent files are converted to magnet URIs using `rs_torrent_magnet`

### Data Flow

1. `lookup()` queries Jackett API → returns `RsLookupSourceResult::Requests` with tokenized URLs
2. `process()` receives requests with `jackett/torrent` MIME → fetches torrent → converts to magnet URI
