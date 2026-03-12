use {
	crate::{
		DocumentSnapshot, Error, Language, LanguageLoader, Syntax,
		change::{ChangeSet, Revision, SnapshotId, TextEdit, UpdateResult},
		text::TextStorage,
		tree_sitter::{InputEdit, Point},
	},
	ropey::Rope,
	std::time::Duration,
};

const DEFAULT_PARSE_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineConfig {
	pub parse_timeout: Duration,
}

impl Default for EngineConfig {
	fn default() -> Self {
		Self {
			parse_timeout: DEFAULT_PARSE_TIMEOUT,
		}
	}
}

#[derive(Debug, Clone)]
pub struct DocumentSession {
	language: Language,
	config: EngineConfig,
	revision: Revision,
	next_snapshot_id: u64,
	generation: u64,
	text: Rope,
	syntax: Syntax,
}

impl DocumentSession {
	pub fn new(
		language: Language, text: &impl TextStorage, loader: &impl LanguageLoader, config: EngineConfig,
	) -> Result<Self, Error> {
		let rope = text.to_rope();
		let syntax = Syntax::new(rope.slice(..), language, config.parse_timeout, loader)?;
		Ok(Self {
			language,
			config,
			revision: Revision(0),
			next_snapshot_id: 1,
			generation: 0,
			text: rope,
			syntax,
		})
	}

	pub fn language(&self) -> Language {
		self.language
	}

	pub fn revision(&self) -> Revision {
		self.revision
	}

	pub fn config(&self) -> EngineConfig {
		self.config
	}

	pub fn text(&self) -> &Rope {
		&self.text
	}

	pub fn snapshot(&self) -> DocumentSnapshot {
		DocumentSnapshot::new(
			SnapshotId(self.next_snapshot_id),
			self.revision,
			self.generation,
			self.text.clone(),
			self.syntax.clone(),
		)
	}

	pub fn apply_edits(&mut self, edits: &ChangeSet, loader: &impl LanguageLoader) -> Result<UpdateResult, Error> {
		if edits.is_empty() {
			return Ok(UpdateResult {
				revision: self.revision,
				snapshot_id: SnapshotId(self.next_snapshot_id),
				changed_ranges: Vec::new(),
				timed_out: false,
				snapshot_changed: false,
				affected_layers: self.syntax.layer_count(),
			});
		}

		let mut text = self.text.clone();
		let mut syntax = self.syntax.clone();
		let mut changed_ranges = Vec::new();

		for edit in edits.iter() {
			validate_edit(&text, edit)?;
			let input_edit = build_input_edit(&text, edit);
			apply_edit(&mut text, edit)?;
			syntax.update(text.slice(..), self.config.parse_timeout, &[input_edit], loader)?;
			changed_ranges.push(edit.range.start..(edit.range.start + edit.replacement.len() as u32));
		}

		self.text = text;
		self.syntax = syntax;
		self.revision = Revision(self.revision.0.wrapping_add(1));
		self.next_snapshot_id = self.next_snapshot_id.wrapping_add(1);
		self.generation = self.generation.wrapping_add(1);

		Ok(UpdateResult {
			revision: self.revision,
			snapshot_id: SnapshotId(self.next_snapshot_id),
			changed_ranges,
			timed_out: false,
			snapshot_changed: true,
			affected_layers: self.syntax.layer_count(),
		})
	}
}

fn apply_edit(text: &mut Rope, edit: &TextEdit) -> Result<(), Error> {
	validate_edit(text, edit)?;
	let start_char = text
		.try_byte_to_char(edit.range.start as usize)
		.map_err(|_| Error::InvalidRanges)?;
	let end_char = text
		.try_byte_to_char(edit.range.end as usize)
		.map_err(|_| Error::InvalidRanges)?;
	text.remove(start_char..end_char);
	text.insert(start_char, &edit.replacement);
	Ok(())
}

fn validate_edit(text: &Rope, edit: &TextEdit) -> Result<(), Error> {
	if edit.range.start > edit.range.end || edit.range.end > text.len_bytes() as u32 {
		return Err(Error::InvalidRanges);
	}

	text.try_byte_to_char(edit.range.start as usize)
		.map_err(|_| Error::InvalidRanges)?;
	text.try_byte_to_char(edit.range.end as usize)
		.map_err(|_| Error::InvalidRanges)?;
	Ok(())
}

fn build_input_edit(text: &Rope, edit: &TextEdit) -> InputEdit {
	let start_byte = edit.range.start;
	let old_end_byte = edit.range.end;
	let new_end_byte = start_byte + edit.replacement.len() as u32;

	InputEdit {
		start_byte,
		old_end_byte,
		new_end_byte,
		start_point: point_for_byte(text, start_byte),
		old_end_point: point_for_byte(text, old_end_byte),
		new_end_point: point_after_insert(point_for_byte(text, start_byte), &edit.replacement),
	}
}

fn point_for_byte(text: &Rope, byte: u32) -> Point {
	let line = text.byte_to_line(byte as usize);
	let line_start = text.line_to_byte(line);
	Point {
		row: line as u32,
		col: byte - line_start as u32,
	}
}

fn point_after_insert(start: Point, inserted: &str) -> Point {
	let mut row = start.row;
	let mut col = start.col;
	let mut last_line_start = 0usize;
	let mut newline_count = 0u32;

	for (idx, ch) in inserted.char_indices() {
		if ch == '\n' {
			newline_count += 1;
			last_line_start = idx + ch.len_utf8();
		}
	}

	if newline_count == 0 {
		col += inserted.len() as u32;
	} else {
		row += newline_count;
		col = inserted[last_line_start..].len() as u32;
	}

	Point { row, col }
}

#[cfg(test)]
mod tests {
	use {
		super::*,
		crate::{SingleLanguageLoader, StringText, tree_sitter::Grammar},
	};

	fn rust_session(src: &str) -> DocumentSession {
		let grammar = Grammar::try_from(tree_sitter_rust::LANGUAGE).expect("rust grammar should load");
		let loader = SingleLanguageLoader::from_queries(crate::Language::new(0), grammar, "", "", "")
			.expect("loader should build");
		DocumentSession::new(
			loader.language(),
			&StringText::new(src),
			&loader,
			EngineConfig::default(),
		)
		.expect("session should parse")
	}

	#[test]
	fn apply_edits_updates_snapshot_text_and_revision() {
		let grammar = Grammar::try_from(tree_sitter_rust::LANGUAGE).expect("rust grammar should load");
		let loader = SingleLanguageLoader::from_queries(crate::Language::new(0), grammar, "", "", "")
			.expect("loader should build");
		let mut session = rust_session("fn alpha() {}\n");

		let result = session
			.apply_edits(&ChangeSet::single(3..8, "beta"), &loader)
			.expect("edit should apply");

		assert_eq!(result.revision, Revision(1));
		assert_eq!(session.snapshot().byte_text(0..14), "fn beta() {}\n");
		assert!(session.snapshot().named_node_at(3, 7).is_some());
	}

	#[test]
	fn oversized_edits_leave_session_unchanged() {
		let grammar = Grammar::try_from(tree_sitter_rust::LANGUAGE).expect("rust grammar should load");
		let loader = SingleLanguageLoader::from_queries(crate::Language::new(0), grammar, "", "", "")
			.expect("loader should build");
		let mut session = rust_session("fn alpha() {}\n");

		let error = session
			.apply_edits(&ChangeSet::single(0..99, ""), &loader)
			.expect_err("edit should fail");
		assert_eq!(error, Error::InvalidRanges);
		assert_eq!(session.snapshot().byte_text(0..14), "fn alpha() {}\n");
	}
}
