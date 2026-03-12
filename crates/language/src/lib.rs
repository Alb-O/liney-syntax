//! Generic language-runtime helpers extracted from the editor-specific syntax stack.
//!
//! This crate keeps the reusable pieces needed to:
//! - find and load compiled tree-sitter grammars
//! - fetch grammar sources from git and build shared libraries
//! - read query files with `; inherits` resolution
//! - pin and fetch Helix runtime queries for build-time embedding

pub mod build;
pub mod grammar;
pub mod helix;
pub mod query;

pub use {
	build::{
		BuildStatus, FetchStatus, GrammarBuildError, GrammarConfig, GrammarSource, build_grammar, fetch_grammar,
		get_grammar_src_dir, grammar_lib_dir, grammar_sources_dir, library_extension,
	},
	grammar::{
		GrammarError, GrammarSource as LoadedGrammarSource, cache_dir, grammar_search_paths, load_grammar,
		load_or_build_grammar, query_search_paths, runtime_dir,
	},
	helix::{HelixQueryError, HelixRuntimeLock, ensure_helix_queries_checkout, merge_language_queries},
	query::{read_query, read_query_from_paths},
};
