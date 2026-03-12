use {
	crate::build::{BuildStatus, FetchStatus, GrammarBuildError, GrammarConfig, build_grammar, fetch_grammar},
	liney_tree_house::tree_sitter::Grammar,
	std::path::{Path, PathBuf},
	thiserror::Error,
	tracing::{info, warn},
};

/// Loaded grammar source.
#[derive(Debug, Clone)]
pub enum GrammarSource {
	Library(PathBuf),
	Builtin(&'static str),
}

/// Errors that can occur when loading a grammar.
#[derive(Debug, Error)]
pub enum GrammarError {
	#[error("grammar not found: {0}")]
	NotFound(String),
	#[error("failed to load grammar library: {0}")]
	LoadError(String),
	#[error("failed to build grammar: {0}")]
	BuildFailed(String),
	#[error("jit grammar build disabled: {0}")]
	JitDisabled(String),
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),
}

impl From<GrammarBuildError> for GrammarError {
	fn from(value: GrammarBuildError) -> Self {
		Self::BuildFailed(value.to_string())
	}
}

pub fn runtime_dir() -> PathBuf {
	if let Ok(runtime) = std::env::var("LINEY_RUNTIME") {
		return PathBuf::from(runtime);
	}

	data_local_dir()
		.map(|dir| dir.join("liney"))
		.unwrap_or_else(|| PathBuf::from("."))
}

pub fn cache_dir() -> Option<PathBuf> {
	#[cfg(unix)]
	{
		std::env::var_os("XDG_CACHE_HOME")
			.map(PathBuf::from)
			.or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
			.map(|path| path.join("liney"))
	}
	#[cfg(windows)]
	{
		std::env::var_os("LOCALAPPDATA").map(|path| PathBuf::from(path).join("liney").join("cache"))
	}
	#[cfg(not(any(unix, windows)))]
	{
		None
	}
}

pub fn grammar_search_paths() -> Vec<PathBuf> {
	let mut dirs = Vec::new();

	if let Ok(runtime) = std::env::var("LINEY_RUNTIME") {
		dirs.push(PathBuf::from(runtime).join("grammars"));
	}

	if let Ok(exe) = std::env::current_exe()
		&& let Some(bin_dir) = exe.parent()
	{
		dirs.push(bin_dir.join("..").join("share").join("liney").join("grammars"));
	}

	if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR")
		&& let Some(workspace) = PathBuf::from(manifest).ancestors().nth(2)
	{
		dirs.push(workspace.join("target").join("grammars"));
	}

	if let Some(cache) = cache_dir() {
		dirs.push(cache.join("grammars"));
	}

	if let Some(data) = data_local_dir() {
		dirs.push(data.join("liney").join("grammars"));
	}

	for helix_dir in helix_runtime_dirs() {
		dirs.push(helix_dir.join("grammars"));
	}

	dirs
}

pub fn query_search_paths() -> Vec<PathBuf> {
	let mut dirs = Vec::new();

	if let Ok(runtime) = std::env::var("LINEY_RUNTIME") {
		dirs.push(PathBuf::from(runtime).join("queries"));
	}

	if let Some(data) = data_local_dir() {
		dirs.push(data.join("liney").join("queries"));
	}

	for helix_dir in helix_runtime_dirs() {
		dirs.push(helix_dir.join("queries"));
	}

	dirs
}

pub fn load_grammar(name: &str) -> Result<Grammar, GrammarError> {
	let lib_name = grammar_library_name(name);

	for path in grammar_search_paths() {
		let lib_path = path.join(&lib_name);
		if lib_path.exists() {
			return load_grammar_from_path(&lib_path, name);
		}
	}

	Err(GrammarError::NotFound(name.to_string()))
}

pub fn load_or_build_grammar(config: &GrammarConfig) -> Result<Grammar, GrammarError> {
	match load_grammar(&config.grammar_id) {
		Ok(grammar) => return Ok(grammar),
		Err(GrammarError::NotFound(_)) => {
			info!(grammar = %config.grammar_id, "Grammar not found in bundle/cache; JIT build is fallback");
		}
		Err(error) => return Err(error),
	}

	if std::env::var_os("LINEY_DISABLE_JIT_GRAMMARS").is_some() {
		return Err(GrammarError::JitDisabled(config.grammar_id.clone()));
	}

	let fetch_status = fetch_grammar(config).map_err(|e| {
		warn!(grammar = %config.grammar_id, error = %e, "Failed to fetch grammar source");
		GrammarError::BuildFailed(format!("fetch failed: {e}"))
	})?;
	match fetch_status {
		FetchStatus::Updated => info!(grammar = %config.grammar_id, "Fetched grammar source"),
		FetchStatus::UpToDate | FetchStatus::Local => {}
	}

	let build_status = build_grammar(config).map_err(|e| {
		warn!(grammar = %config.grammar_id, error = %e, "Failed to build grammar");
		GrammarError::BuildFailed(format!("build failed: {e}"))
	})?;
	match build_status {
		BuildStatus::Built => info!(grammar = %config.grammar_id, "Compiled grammar"),
		BuildStatus::AlreadyBuilt => {}
	}

	load_grammar(&config.grammar_id)
}

fn load_grammar_from_path(path: &Path, name: &str) -> Result<Grammar, GrammarError> {
	unsafe { Grammar::new(name, path).map_err(|e| GrammarError::LoadError(format!("{}: {}", path.display(), e))) }
}

fn grammar_library_name(name: &str) -> String {
	let safe_name = name.replace('-', "_");
	#[cfg(target_os = "macos")]
	{
		format!("lib{safe_name}.dylib")
	}
	#[cfg(target_os = "windows")]
	{
		format!("{safe_name}.dll")
	}
	#[cfg(not(any(target_os = "macos", target_os = "windows")))]
	{
		format!("lib{safe_name}.so")
	}
}

fn data_local_dir() -> Option<PathBuf> {
	#[cfg(unix)]
	{
		std::env::var_os("XDG_DATA_HOME")
			.map(PathBuf::from)
			.or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share")))
	}
	#[cfg(windows)]
	{
		std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
	}
	#[cfg(not(any(unix, windows)))]
	{
		None
	}
}

fn helix_runtime_dirs() -> Vec<PathBuf> {
	let mut dirs = Vec::new();

	if let Ok(runtime) = std::env::var("HELIX_RUNTIME") {
		dirs.push(PathBuf::from(runtime));
	}

	#[cfg(unix)]
	if let Some(config) = std::env::var_os("XDG_CONFIG_HOME")
		.map(PathBuf::from)
		.or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
	{
		let helix_runtime = config.join("helix").join("runtime");
		if helix_runtime.exists() {
			dirs.push(helix_runtime);
		}
	}

	if let Some(data) = data_local_dir() {
		let helix_runtime = data.join("helix").join("runtime");
		if helix_runtime.exists() {
			dirs.push(helix_runtime);
		}
	}

	dirs
}
