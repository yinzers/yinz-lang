//! Markdown input: rewrites fenced code blocks tagged `ynz` or `yinz` that parse.

use crate::yinz_source::{apply_renames, line_of_offset, rewrite_yinz_source, ParseDiagnostics};

#[derive(Debug, Default)]
pub struct MarkdownRewrite {
    pub new_text: String,
    pub renames: usize,
    pub blocks_rewritten: usize,
    pub blocks_unchanged: usize,
    /// (line of the opening fence, reason) for each tagged block that did not parse.
    pub blocks_unparseable: Vec<(usize, String)>,
    pub ambiguous: Vec<(usize, String)>,
}

struct OpenFence {
    fence_character: u8,
    fence_length: usize,
    is_yinz_block: bool,
    content_start: usize,
    fence_line: usize,
}

/// O(n) time and memory in the file length (plus the parse cost of each tagged block).
pub fn rewrite_markdown(file_label: &str, source: &str) -> MarkdownRewrite {
    let mut result = MarkdownRewrite::default();
    let mut output = String::with_capacity(source.len());
    let mut copied_up_to = 0;
    let mut open_fence: Option<OpenFence> = None;
    let mut line_start = 0;

    while line_start < source.len() {
        let line_end = source[line_start..]
            .find('\n')
            .map_or(source.len(), |newline| line_start + newline + 1);
        let line = &source[line_start..line_end];
        let trimmed = line.trim_start_matches([' ', '\t']);
        let fence_run = fence_run_of(trimmed);

        match (&open_fence, fence_run) {
            (None, Some((fence_character, fence_length))) => {
                let info_string = trimmed[fence_length..].trim();
                let language = info_string
                    .split(|character: char| character.is_whitespace() || character == '{')
                    .next()
                    .unwrap_or("");
                open_fence = Some(OpenFence {
                    fence_character,
                    fence_length,
                    is_yinz_block: language == "ynz" || language == "yinz",
                    content_start: line_end,
                    fence_line: line_of_offset(source, line_start),
                });
            }
            (Some(fence), Some((fence_character, fence_length)))
                if fence_character == fence.fence_character
                    && fence_length >= fence.fence_length
                    && trimmed[fence_length..].trim().is_empty() =>
            {
                if fence.is_yinz_block {
                    rewrite_block(
                        file_label,
                        source,
                        fence,
                        line_start,
                        &mut copied_up_to,
                        &mut output,
                        &mut result,
                    );
                }
                open_fence = None;
            }
            _ => {}
        }
        line_start = line_end;
    }

    output.push_str(&source[copied_up_to..]);
    result.new_text = output;
    result
}

fn rewrite_block(
    file_label: &str,
    source: &str,
    fence: &OpenFence,
    content_end: usize,
    copied_up_to: &mut usize,
    output: &mut String,
    result: &mut MarkdownRewrite,
) {
    let block_text = &source[fence.content_start..content_end];
    match rewrite_yinz_source(file_label, block_text, ParseDiagnostics::Reject) {
        Err(unparseable) => result
            .blocks_unparseable
            .push((fence.fence_line, unparseable.reason)),
        Ok(rewrite) => {
            for (line_in_block, name) in rewrite.ambiguous_constant_receivers {
                result
                    .ambiguous
                    .push((fence.fence_line + line_in_block, name));
            }
            if rewrite.renames.is_empty() {
                result.blocks_unchanged += 1;
            } else {
                output.push_str(&source[*copied_up_to..fence.content_start]);
                output.push_str(&apply_renames(block_text, &rewrite.renames));
                *copied_up_to = content_end;
                result.renames += rewrite.renames.len();
                result.blocks_rewritten += 1;
            }
        }
    }
}

/// Returns the fence character and run length when the line starts with 3+ backticks or tildes.
fn fence_run_of(trimmed_line: &str) -> Option<(u8, usize)> {
    let first = *trimmed_line.as_bytes().first()?;
    if first != b'`' && first != b'~' {
        return None;
    }
    let run = trimmed_line.bytes().take_while(|b| *b == first).count();
    (run >= 3).then_some((first, run))
}
