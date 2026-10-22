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
fontcore add Brand.otf --app ai.storytold.photocraft --tag brand
fontcore list                                  # table; add --json for machine output
fontcore list --app ai.storytold.wordcraft     # what wordcraft will load
fontcore info 1f3a                             # any unique id prefix works
fontcore scope 1f3a ai.storytold.photocraft ai.storytold.pdfcraft   # no apps = all
fontcore disable 1f3a                          # hide from apps, keep in vault
fontcore disable 8be0 --face 2                 # hide one face of a collection
fontcore enable 8be0 --face 2
fontcore remove 1f3a
fontcore verify                                # missing files, hash mismatches, orphans
```

Supported formats are TrueType (`.ttf`), OpenType/CFF (`.otf`), and collections (`.ttc`, `.otc`). Variable fonts in any of these formats are supported. FontCore checks a file's signature, not its extension. WOFF and WOFF2 files are rejected; convert them to TTF or OTF first (see [v1 design decisions](#v1-design-decisions)).

`--license` takes an SPDX license expression such as `OFL-1.1` or `MIT OR Apache-2.0`. A single id in the wrong case (`ofl-1.1`) is corrected. Any other text is stored as a custom license, with a warning. FontCore also warns when an SPDX license is neither OSI-approved nor FSF free/libre (for example `CC-BY-NC-4.0`). Warnings never block the import.

## Using FontCore from a suite app

Add the library to the app's dependencies:

```toml
[dependencies]
fontcore = { git = "https://github.com/calebnanji/fontcore" }
```

Then load the fonts at startup:

```rust
fn register_fonts() -> fontcore::Result<()> {
    let set = fontcore::load_for_app("ai.storytold.photocraft")?;
    for font in &set.fonts {
        let bytes = font.read()?;          // or pass font.path to the text engine
        for face in font.faces() {         // enabled faces only
            my_text_engine::register(&bytes, face.index);
        }
    }
    for missing in &set.unavailable {
        log::warn!("font {} listed in fonts.json but unavailable", missing.id);
    }
    Ok(())
}
```

`load_for_app` only reads; it never writes to the vault. If no vault exists yet, it returns an empty set, so an app can call it on every launch without checking first. The app id must be a lowercase reverse-DNS id (see below); anything else returns `Error::InvalidAppId`. For collections, register only the faces `font.faces()` returns: the user may have disabled some.

Management tools can use `fontcore::Vault` directly (`init`, `add`, `remove`, `set_enabled`, `set_face_enabled`, `set_apps`, `verify`).

## `fonts.json` schema (version 1, stable)

The layout is published as a JSON Schema (draft 2020-12) at [`schema/fonts.v1.json`](schema/fonts.v1.json). Tools in any language can validate a manifest against it. A test in `crates/fontcore/tests/schema.rs` checks the schema against the Rust types, so the two cannot drift apart.

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
          "monospaced": false,
          "enabled": true
        }
      ],
      "enabled": true,
      "apps": ["ai.storytold.photocraft"],
      "tags": ["brand"],
      "license": "OFL-1.1",
      "source": "Inter-Bold.ttf",
      "added_at": "2026-10-22T02:30:00Z"
    }
  ]
}
```

| Field | Meaning |
| --- | --- |
| `file` | Path relative to the vault root: `/`-separated, non-empty segments, with no `.`, `..`, `\` or `:`. Anything else is treated as unsafe and not loaded. |
| `format` | `ttf`, `otf`, or `ttc` (covers `.otc` too) |
| `faces` | One entry per face, with its `index` in the file. Collections list several. |
| `faces[].enabled` | `false` hides that face from apps; the other faces of a collection stay available. Defaults to `true`. |
| `enabled` | `false` hides the whole file, whatever its faces say. A file whose faces are all disabled is hidden too. Defaults to `true`. |
| `apps` | App ids that may load the font. Omitted or empty means all apps. |
| `license` | An SPDX license expression as a string (`"OFL-1.1"`). A license without an SPDX identifier is wrapped in an object: `{"custom": "Licensed to Acme for internal use"}`. |
| `tags`, `source` | Optional; left out when empty. |
| `added_at` | Import time, as an RFC 3339 date-time |

### Rules

- **App ids** are lowercase reverse-DNS with at least three labels, such as `ai.storytold.photocraft`, `ai.storytold.wordcraft` or `ai.storytold.pdfcraft`. Each label is 1–63 characters of `a-z`, `0-9` and `-`, and doesn't start or end with `-`. Ids are compared exactly. A manifest containing an invalid id fails to load.
- **Timestamps** are strict RFC 3339. `T` and `Z` must be uppercase, and a `Z` or `±HH:MM` offset is required. Fractional seconds are allowed. FontCore writes whole seconds in UTC, like `2026-10-22T02:30:00Z`, and keeps timestamps written by other tools exactly as they were.
- **Licenses** in string form are meant to be SPDX expressions. A manifest still loads if a string isn't one this build recognizes, such as an identifier added to SPDX later. `License::is_recognized` and `License::is_open_source` let tools check.
- **Unknown fields are preserved.** Every object (the manifest, each font, each face, and a custom license) may carry fields this schema doesn't list. Readers ignore them, and FontCore writes them back unchanged when it rewrites the file. Name your fields after your app id, e.g. `"ai.storytold.photocraft:pinned": true`, so they cannot collide with fields added to v1 later.
- **Versioning:** adding optional fields keeps `schema_version: 1`. A change that existing readers would misread needs a new version. Readers refuse versions newer than they know rather than guess.

Writes are atomic: FontCore writes a temporary file and renames it into place, so apps never read a half-written manifest. A lock file serializes writers; a lock left behind by a crashed process is treated as stale after 30 seconds.

### v1 design decisions

Schema version 1 is **stable** as of 2026-10-22. From now on it changes only by adding optional fields, and anything else needs `schema_version: 2`. These are the decisions behind it, settled in [issue #2](https://github.com/calebnanji/FontCore/issues/2).

| Topic | Decision |
| --- | --- |
| Forward compatibility | Every object keeps fields it doesn't recognize and writes them back unchanged. Additive changes keep version 1. Readers refuse versions newer than they know. |
| App ids | Strict lowercase reverse-DNS, such as `ai.storytold.photocraft`, compared exactly. |
| Collections | Each face has its own `enabled` flag. The font-level `enabled` switches off the whole file. |
| Timestamps | `added_at` is a strict RFC 3339 string. |
| Licenses | An SPDX expression string, or `{"custom": "..."}` for licenses without an SPDX id. The CLI warns at import when a license isn't recognized or isn't open source. |
| Variable fonts | Supported; see below. |
| Web fonts | WOFF/WOFF2 are rejected at import; see below. |
| Grouping | `tags` only; see below. |
| Migration | A dedicated migration tool, if v2 is ever needed; see below. |

**Variable fonts.** OpenType variable fonts (`.ttf`, `.otf`, and collections containing them) are stored like any other font. The file is copied byte for byte, so its axes and named instances reach apps intact. `fonts.json` deliberately doesn't record axes or named instances: each file gets one face entry describing its default instance, which keeps the manifest small. Apps that need axes or instances read them at runtime from the font data the loader returns (`ResolvedFont::read` or `ResolvedFont::path`), for example from the `fvar` table with their text engine or `ttf-parser`. If listing axes without parsing ever becomes necessary, an optional field can add it within v1.

**Web fonts.** WOFF and WOFF2 are rejected when you try to add them to the vault. `fonts.json` never contains them, and `format` stays `ttf`, `otf` or `ttc`. Tools that start from web fonts must convert them to TTF or OTF before importing.

**Grouping.** v1 organizes fonts only with the free-form `tags` array. Named sets such as "Brand Kit" or "Display" are left to each app. An app that wants them stores them in its own extension fields, named after its app id, which FontCore preserves. For example, a top-level `"ai.storytold.photocraft:sets": {"Brand Kit": ["1f3a9c0d2b7e4a51"]}`. FontCore itself gives these fields no meaning.

**Migration to a future version 2.** If `schema_version: 2` is ever needed, the `fontcore` crate will ship a dedicated migration tool. It will rewrite `fonts.json` from v1 to v2 separately from the apps (taking the vault lock and writing atomically, like every other write) before any suite app built for v2 reads the vault. Apps never migrate the manifest themselves. Because readers already refuse versions newer than they know, an app built for v1 stops with a clear error on a migrated vault instead of misreading it.

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
