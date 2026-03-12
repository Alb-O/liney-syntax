//! Generic language-runtime helpers for tree-sitter grammars and queries.
//!
//! This crate provides the pieces needed to:
//! - find and load compiled tree-sitter grammars
//! - fetch grammar sources from git and build shared libraries
//! - read query files with `; inherits` resolution
//! - pin and fetch Helix runtime queries for build-time embedding

pub mod build;
pub mod bundle;
pub mod grammar;
pub mod helix;
pub mod query;
pub mod registry;
pub mod runtime_paths;

pub use {
	build::{
		BuildStatus, FetchStatus, GrammarBuildError, GrammarConfig, GrammarSource, build_grammar, fetch_grammar,
		get_grammar_src_dir, grammar_lib_dir, grammar_sources_dir, library_extension,
	},
	bundle::{QueryBundle, load_query_bundle},
	grammar::{
		GrammarError, GrammarSource as LoadedGrammarSource, load_grammar, load_grammar_from_path,
		load_grammar_from_paths, load_or_build_grammar, load_or_build_grammar_from_paths, locate_grammar_library,
	},
	helix::{HelixQueryError, HelixRuntimeLock, ensure_helix_queries_checkout, merge_language_queries},
	query::{read_query, read_query_from_paths},
	registry::{GrammarLocator, LanguageId, LanguageRegistry, LanguageSpec, QueryLocator},
	runtime_paths::{cache_dir, grammar_search_paths, query_search_paths, runtime_dir},
};
