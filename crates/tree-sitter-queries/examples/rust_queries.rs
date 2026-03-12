use {
	liney_tree_house::{
		InjectionLanguageMarker, Language, LanguageConfig, LanguageLoader, Syntax, TREE_SITTER_MATCH_LIMIT,
		tree_sitter::{Grammar, InactiveQueryCursor, RopeInput},
	},
	liney_tree_sitter_queries::{RainbowQuery, TagQuery, TextObjectQuery},
	ropey::Rope,
	std::{error::Error, time::Duration},
};

const SOURCE: &str = r#"struct User {
    name: String,
}

fn build_user(name: &str) -> User {
    User { name: name.to_owned() }
}

fn greet(user: &User) -> String {
    user.name.clone()
}

fn main() {
    let user = build_user("Ada");
    let greeting = greet(&user);
    println!("{greeting}");
}
"#;

const TEXT_OBJECT_QUERY: &str = r#"
(function_item
  body: (block) @function.inner) @function.outer

(call_expression
  arguments: (arguments) @call.arguments)
"#;

const TAG_QUERY: &str = r#"
(function_item
  name: (identifier) @name) @definition.function
"#;

const RAINBOW_QUERY: &str = r#"
[
  "{"
  "}"
  "("
  ")"
] @rainbow.bracket

[
  (block)
  (parameters)
  (arguments)
] @rainbow.scope
"#;

struct SingleLanguageLoader {
	language: Language,
	config: LanguageConfig,
}

impl SingleLanguageLoader {
	fn rust() -> Result<Self, Box<dyn Error>> {
		let grammar = Grammar::try_from(tree_sitter_rust::LANGUAGE)?;
		let config = LanguageConfig::new(grammar, "", "", "")?;
		Ok(Self {
			language: Language::new(0),
			config,
		})
	}

	fn grammar(&self) -> Grammar {
		self.config.grammar
	}
}

impl LanguageLoader for SingleLanguageLoader {
	fn language_for_marker(&self, _marker: InjectionLanguageMarker) -> Option<Language> {
		Some(self.language)
	}

	fn get_config(&self, language: Language) -> Option<&LanguageConfig> {
		(language == self.language).then_some(&self.config)
	}
}

fn snippet(start: usize, end: usize) -> &'static str {
	&SOURCE[start..end]
}

fn main() -> Result<(), Box<dyn Error>> {
	let loader = SingleLanguageLoader::rust()?;
	let rope = Rope::from_str(SOURCE);
	let syntax = Syntax::new(rope.slice(..), loader.language, Duration::from_millis(500), &loader)?;
	let root = syntax.tree().root_node();

	let text_objects = TextObjectQuery::new(loader.grammar(), TEXT_OBJECT_QUERY)?;
	let functions: Vec<_> = text_objects
		.capture_nodes("function.outer", &root, rope.slice(..))
		.expect("function.outer capture should exist")
		.map(|node| {
			let range = node.byte_range();
			snippet(range.start, range.end)
				.trim()
				.lines()
				.next()
				.unwrap()
				.to_owned()
		})
		.collect();
	assert_eq!(functions.len(), 3);
	assert!(
		functions
			.iter()
			.any(|line| line == "fn build_user(name: &str) -> User {")
	);
	assert!(functions.iter().any(|line| line == "fn greet(user: &User) -> String {"));
	assert!(functions.iter().any(|line| line == "fn main() {"));

	let call_arguments: Vec<_> = text_objects
		.capture_nodes("call.arguments", &root, rope.slice(..))
		.expect("call.arguments capture should exist")
		.map(|node| {
			let range = node.byte_range();
			snippet(range.start, range.end).to_owned()
		})
		.collect();
	assert!(call_arguments.iter().any(|args| args == "(\"Ada\")"));
	assert!(call_arguments.iter().any(|args| args == "(&user)"));

	let tag_query = TagQuery::new(loader.grammar(), TAG_QUERY)?;
	let name_capture = tag_query.query.get_capture("name").expect("name capture should exist");
	let mut tag_cursor = InactiveQueryCursor::new(0..u32::MAX, TREE_SITTER_MATCH_LIMIT).execute_query(
		&tag_query.query,
		&root,
		RopeInput::new(rope.slice(..)),
	);
	let mut tagged_functions = Vec::new();
	while let Some(query_match) = tag_cursor.next_match() {
		tagged_functions.extend(query_match.nodes_for_capture(name_capture).map(|node| {
			let range = node.byte_range();
			snippet(range.start as usize, range.end as usize).to_owned()
		}));
	}
	assert_eq!(
		tagged_functions,
		vec!["build_user".to_owned(), "greet".to_owned(), "main".to_owned()]
	);

	let rainbow_query = RainbowQuery::new(loader.grammar(), RAINBOW_QUERY)?;
	let bracket_capture = rainbow_query
		.bracket_capture
		.expect("rainbow.bracket capture should exist");
	let mut rainbow_cursor = InactiveQueryCursor::new(0..u32::MAX, TREE_SITTER_MATCH_LIMIT).execute_query(
		&rainbow_query.query,
		&root,
		RopeInput::new(rope.slice(..)),
	);
	let mut bracket_count = 0;
	while let Some(query_match) = rainbow_cursor.next_match() {
		bracket_count += query_match.nodes_for_capture(bracket_capture).count();
	}
	assert!(bracket_count >= 10);

	println!(
		"parsed {} functions, tagged {:?}, and matched {bracket_count} rainbow brackets",
		functions.len(),
		tagged_functions
	);

	Ok(())
}
