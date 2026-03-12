# liney-syntax

Reusable Rust crates for tree-sitter engines, runtime loading, editor adapters,
and structural query helpers.

## Crate roles

- `crates/tree-house`
  Generic engine layer. Owns document sessions, immutable snapshots, incremental
  parsing, injections, locals, and highlight/query execution.
- `crates/language`
  Runtime and registry layer. Owns explicit grammar/query lookup, query bundles,
  language registry types, and optional Helix/build helpers.
- `crates/syntax`
  Editor adapter layer. Owns document IDs, viewport/full-tree selection policy,
  viewport caches, and tiled highlight caching.
- `crates/tree-sitter-queries`
  Query-product layer. Owns reusable text-object, tag, indentation, and rainbow
  helpers built on top of engine snapshots.

## Examples

- `crates/tree-house/examples/engine_basic.rs`
  Parse and highlight through `DocumentSession` and `DocumentSnapshot`.
- `crates/tree-house/examples/engine_edits.rs`
  Apply edits through the engine session API and inspect revision metadata.
- `crates/language/examples/runtime_registry.rs`
  Load queries from explicit roots through the runtime registry layer.
- `crates/syntax/examples/editor_viewport_manager.rs`
  Use the optional editor adapter for viewport selection and tile caching.
- `crates/tree-sitter-queries/examples/queries_tags.rs`
  Run reusable tag queries against an engine snapshot.
