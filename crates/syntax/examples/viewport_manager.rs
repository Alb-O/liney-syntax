use {
	liney_syntax::{
		DocumentId, Highlight, HighlightSpanQuery, HighlightTiles, Language, LanguageConfig, LanguageLoader,
		SealedSource, Syntax, SyntaxManager, SyntaxOptions, ViewportKey, tree_sitter::Grammar,
	},
	liney_tree_house::InjectionLanguageMarker,
	ropey::Rope,
	std::{error::Error, sync::Arc},
};

const H_IDENTIFIER: u32 = 1;
const H_TYPE: u32 = 2;
const H_BUILTIN_TYPE: u32 = 3;
const H_STRING: u32 = 4;
const H_NUMBER: u32 = 5;
const H_BRACKET: u32 = 6;

const SOURCE: &str = r#"const BEFORE: u32 = 1;

fn middle(value: i32) -> i32 {
    let label = "mid";
    value + 1
}

const AFTER: u32 = 2;
"#;

const HIGHLIGHT_QUERY: &str = r#"
(identifier) @identifier
(type_identifier) @type
(primitive_type) @type.builtin
(string_literal) @string
(integer_literal) @number
[
  "{"
  "}"
  "("
  ")"
] @punctuation.bracket
"#;

struct SingleLanguageLoader {
	language: Language,
	config: LanguageConfig,
}

impl SingleLanguageLoader {
	fn rust() -> Result<Self, Box<dyn Error>> {
		let grammar = Grammar::try_from(tree_sitter_rust::LANGUAGE)?;
		let config = LanguageConfig::new(grammar, HIGHLIGHT_QUERY, "", "")?;
		config.configure(|name| {
			Some(match name {
				"identifier" => Highlight::new(H_IDENTIFIER),
				"type" => Highlight::new(H_TYPE),
				"type.builtin" => Highlight::new(H_BUILTIN_TYPE),
				"string" => Highlight::new(H_STRING),
				"number" => Highlight::new(H_NUMBER),
				"punctuation.bracket" => Highlight::new(H_BRACKET),
				_ => return None,
			})
		});

		Ok(Self {
			language: Language::new(0),
			config,
		})
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

fn snippet(range: std::ops::Range<u32>) -> &'static str {
	&SOURCE[range.start as usize..range.end as usize]
}

fn main() -> Result<(), Box<dyn Error>> {
	let loader = SingleLanguageLoader::rust()?;
	let rope = Rope::from_str(SOURCE);
	let opts = SyntaxOptions::default();
	let full = Syntax::new(rope.slice(..), loader.language, &loader, opts)?;
	let full_spans = full.highlighter(rope.slice(..), &loader, ..).collect_spans();
	assert!(
		full_spans
			.iter()
			.any(|span| span.highlight.get() == H_STRING && snippet(span.range()) == "\"mid\"")
	);

	let viewport_start = SOURCE.find("fn middle").expect("viewport start should exist") as u32;
	let viewport_end = SOURCE.find("\n\nconst AFTER").expect("viewport end should exist") as u32;
	let sealed = Arc::new(SealedSource::from_window(
		rope.byte_slice(viewport_start as usize..viewport_end as usize),
		"",
	));
	let viewport = Syntax::new_viewport(sealed, loader.language, &loader, opts, viewport_start)?;
	let viewport_spans = viewport
		.highlighter(rope.slice(..), &loader, viewport_start..viewport_end)
		.collect_spans();
	assert!(
		viewport_spans
			.iter()
			.all(|span| span.start >= viewport_start && span.end <= viewport_end)
	);
	assert!(
		viewport_spans
			.iter()
			.any(|span| span.highlight.get() == H_STRING && snippet(span.range()) == "\"mid\"")
	);

	let doc_id = DocumentId(7);
	let viewport_key = ViewportKey(1);
	let mut manager = SyntaxManager::new();
	manager.install_full(doc_id, full.clone(), 1);
	manager.install_viewport_stage_b(doc_id, viewport_key, viewport.clone(), viewport_start..viewport_end, 2);

	let viewport_selection = manager
		.syntax_for_viewport(doc_id, 2, viewport_start..viewport_end)
		.expect("viewport syntax should be selected");
	assert_eq!(viewport_selection.coverage, Some(viewport_start..viewport_end));
	assert_eq!(viewport_selection.tree_doc_version, 2);

	let mut tiles = HighlightTiles::new();
	let start_line = rope.byte_to_line(viewport_start as usize);
	let end_line = rope.byte_to_line(viewport_end as usize) + 1;
	let tile_spans = tiles.get_spans(HighlightSpanQuery {
		doc_id,
		syntax_version: manager.syntax_version(doc_id),
		rope: &rope,
		syntax: viewport_selection.syntax,
		loader: &loader,
		style_resolver: |highlight| highlight.get(),
		start_line,
		end_line,
	});
	assert!(
		tile_spans
			.iter()
			.any(|(span, style)| *style == H_STRING && snippet(span.range()) == "\"mid\"")
	);

	manager.install_full(doc_id, full, 2);
	let full_selection = manager
		.syntax_for_viewport(doc_id, 2, viewport_start..viewport_end)
		.expect("full syntax should be selected");
	assert!(full_selection.coverage.is_none());
	assert_eq!(full_selection.tree_doc_version, 2);

	println!(
		"full spans: {}, viewport spans: {}, cached spans: {}",
		full_spans.len(),
		viewport_spans.len(),
		tile_spans.len()
	);

	Ok(())
}
