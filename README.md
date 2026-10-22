# FontCore

FontCore is an open-source desktop font manager written in Rust. It manages fonts for every app in the storytold (ArtCraft) suite: photocraft, wordcraft, pdfcraft, and the rest.

FontCore never changes the operating system's font registry. It keeps its own **vault**, a directory holding copies of your fonts and a single `fonts.json` manifest. Suite apps read that manifest on launch and load the fonts it lists. If you add a font once, every app can use it, and nothing is installed system-wide.

## Workspace

| Crate | What it is |
| --- | --- |
| [`fontcore`](crates/fontcore) | Library: the vault, the `fonts.json` schema, font parsing, and the read-only loader that suite apps use |
| [`fontcore-cli`](crates/fontcore-cli) | The `fontcore` command-line tool |

## Vault layout

```
<vault>/
├── fonts.json          # the manifest; the only file apps need to read
├── fonts/
│   ├── 1f3a9c0d2b7e4a51.ttf
│   └── 8be0c4f7a2d91e36.otc
└── .fontcore.lock      # present only while a write is in progress
```

FontCore picks the vault location in this order:

1. `--vault <dir>` (CLI only)
2. `$FONTCORE_HOME`
3. The platform data directory: `~/.local/share/fontcore` on Linux, `~/Library/Application Support/fontcore` on macOS, or `%APPDATA%\storytold\fontcore\data` on Windows

Each font file is stored under an id: the first 16 hex digits of its SHA-256 hash. Importing the same file twice therefore does nothing the second time.

## CLI

```sh
cargo install --path crates/fontcore-cli

fontcore init                                  # create the vault
fontcore add ~/Downloads/Inter --license OFL-1.1
fontcore add Brand.otf --app photocraft --tag brand
fontcore list                                  # table; add --json for machine output
fontcore list --app wordcraft                  # what wordcraft will load
fontcore info 1f3a                             # any unique id prefix works
fontcore scope 1f3a photocraft pdfcraft        # restrict to apps (no apps = all)
fontcore disable 1f3a                          # hide from apps, keep in vault
fontcore enable 1f3a
fontcore remove 1f3a
fontcore verify                                # missing files, hash mismatches, orphans
```

Supported formats are TrueType (`.ttf`), OpenType/CFF (`.otf`), and collections (`.ttc`, `.otc`). FontCore checks a file's signature, not its extension. WOFF and WOFF2 files are rejected; convert them to TTF or OTF first.

## Using FontCore from a suite app

Add the library to the app's dependencies:

```toml
[dependencies]
fontcore = { git = "https://github.com/calebnanji/fontcore" }
```

Then load the fonts at startup:

```rust
fn register_fonts() -> fontcore::Result<()> {
    let set = fontcore::load_for_app("photocraft")?;
    for font in &set.fonts {
        let bytes = font.read()?;          // or pass font.path to the text engine
        my_text_engine::register(&bytes, &font.entry.faces);
    }
    for missing in &set.unavailable {
        log::warn!("font {} listed in fonts.json but unavailable", missing.id);
    }
    Ok(())
}
```

`load_for_app` only reads; it never writes to the vault. If no vault exists yet, it returns an empty set, so an app can call it on every launch without checking first. App names are compared case-insensitively.

Management tools can use `fontcore::Vault` directly (`init`, `add`, `remove`, `set_enabled`, `set_apps`, `verify`).

## `fonts.json` schema (version 1)

```json
{
  "schema_version": 1,
  "fonts": [
    {
      "id": "1f3a9c0d2b7e4a51",
      "file": "fonts/1f3a9c0d2b7e4a51.ttf",
      "format": "ttf",
      "sha256": "1f3a9c0d2b7e4a51…",
      "size": 84492,
      "faces": [
        {
          "index": 0,
          "family": "Inter",
          "subfamily": "Bold",
          "full_name": "Inter Bold",
          "postscript_name": "Inter-Bold",
          "version": "Version 4.000",
          "weight": 700,
          "italic": false,
          "monospaced": false
        }
      ],
      "enabled": true,
      "apps": ["photocraft"],
      "tags": ["brand"],
      "license": "OFL-1.1",
      "source": "Inter-Bold.ttf",
      "added_at": 1792646718
    }
  ]
}
```

| Field | Meaning |
| --- | --- |
| `file` | Path relative to the vault root, using `/`. Readers reject absolute paths and `..`. |
| `format` | `ttf`, `otf`, or `ttc` (covers `.otc` too) |
| `faces` | One entry per face. Collections list several, each with its `index`. |
| `enabled` | `false` hides the font from apps but keeps it in the vault. |
| `apps` | Apps that may load the font. Omitted or empty means all apps. |
| `tags`, `license`, `source` | Optional; left out when empty. |
| `added_at` | Import time, in Unix seconds |

Writes are atomic: FontCore writes a temporary file and renames it into place, so apps never read a half-written manifest. A lock file serializes writers; a lock left behind by a crashed process is treated as stale after 30 seconds. A build refuses to load a manifest whose `schema_version` is newer than it supports, rather than misreading it.

## Development

```sh
cargo test --all
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```

Tests generate minimal TrueType files in code (`crates/fontcore/tests/common`), so the repository contains no binary font fixtures.

## License

FontCore is dual-licensed under either of:

- the Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- the MIT License ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

You may use it under whichever license you prefer. This is the usual arrangement in the Rust ecosystem. MIT is short and permissive. Apache-2.0 adds an explicit patent grant and patent-retaliation terms. Offering both lets downstream projects choose the one that fits their own licensing. Both crates in this workspace declare `license = "MIT OR Apache-2.0"`.

This license covers FontCore's own code. Fonts you import into a vault keep their own licenses (for example `OFL-1.1`), which you can record in each entry's `license` field.

### Contribution

Unless you explicitly state otherwise, any contribution you intentionally submit for inclusion in this work, as defined in the Apache-2.0 license, is dual-licensed as above, without any additional terms or conditions.
