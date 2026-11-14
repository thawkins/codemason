//! Word-wrapping of styled lines into display rows.
//!
//! A port of the terminal renderer's `WordWrapper` (trim = false) over
//! frontend-neutral [`StyledLine`]s, so the app's pre-wrapped line caches
//! hold exactly the rows the terminal paints (cached row counts and scroll
//! geometry stay in one coordinate system).
//!
//! Depends on `unicode-segmentation` / `unicode-width` (grapheme widths, the
//! same versions the renderer uses) and `codemason-app-core` (styled text).

use codemason_app_core::styled::{StyledLine, StyledSpan, TextStyle};

/// Wrap one rendered [`StyledLine`] into styled display rows that match the
/// terminal renderer's `Paragraph` word-wrapping (`Wrap { trim: false }`).
///
/// Rows are broken exactly where the terminal renderer's `WordWrapper`
/// (`reflow.rs`, `WordWrapper::process_input`) breaks them, and each row keeps
/// the original span styles.  Because no row ever exceeds the target width,
/// re-wrapping a row inside `Paragraph` is a no-op: the cache geometry and the
/// painted output stay in one coordinate system.
pub fn wrap_line_styled(line: &StyledLine, width: usize) -> Vec<StyledLine> {
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;

    if width == 0 {
        return vec![line.clone()];
    }

    let max = width;
    // Flat stream of (grapheme symbol, patched style) mirroring
    // `Paragraph::styled_graphemes` (line style patched with span style).
    // the renderer (0.30) filters every grapheme that contains a control character
    // (`filter(|g| !g.contains(char::is_control))`), not just `"\n"` as 0.29
    // did, so tabs must be dropped here too or the port over-counts rows.
    let graphemes: Vec<(String, TextStyle)> = line
        .spans
        .iter()
        .flat_map(|span| {
            let span_style = span.style;
            span.text
                .graphemes(true)
                .filter(|g| !g.contains(char::is_control))
                .map(move |g| (g.to_string(), span_style))
        })
        .collect();

    // Port of `WordWrapper::process_input` (trim = false).  State:
    // `row` is the display row being built, `word` the current word buffer,
    // `ws` the pending whitespace run.
    let mut rows: Vec<Vec<(String, TextStyle)>> = Vec::new();
    let mut row: Vec<(String, TextStyle)> = Vec::new();
    let mut row_width: usize = 0;
    let mut word: Vec<(String, TextStyle)> = Vec::new();
    let mut word_width: usize = 0;
    let mut ws: Vec<(String, TextStyle)> = Vec::new();
    let mut ws_width: usize = 0;
    let mut non_ws_previous = false;

    for (g, style) in graphemes {
        // The renderer treats NBSP as a regular (non-whitespace) grapheme
        // and ZWSP (zero-width space) as whitespace, so the port must agree
        // with `StyledGrapheme::is_whitespace` or the wrapped geometry
        // drifts from the painted output.
        let is_ws = (g.chars().all(char::is_whitespace) && g != "\u{00a0}") || g == "\u{200b}";
        // Symbols wider than the limit are ignored (as in WordWrapper).
        if g.width() > max {
            continue;
        }
        let gw = g.width();

        let word_found = non_ws_previous && is_ws;
        // The completed word (with its preceding whitespace run) would
        // overflow even on an otherwise empty row.
        let untrimmed_overflow = row.is_empty() && word_width + ws_width + gw > max;

        // Flush the completed word to the row (trim = false keeps the
        // leading whitespace run).
        if word_found || untrimmed_overflow {
            row.append(&mut ws);
            row_width += ws_width;
            row.append(&mut word);
            row_width += word_width;
            ws_width = 0;
            word_width = 0;
        }

        let row_full = row_width >= max;
        let pending_word_overflow = gw > 0 && row_width + ws_width + word_width >= max;

        if row_full || pending_word_overflow {
            let remaining = max.saturating_sub(row_width);
            rows.push(std::mem::take(&mut row));
            row_width = 0;

            // Drop whitespace up to the end of the emitted row.
            while let Some((front, _)) = ws.first() {
                let fw = front.width();
                if fw > remaining {
                    break;
                }
                ws_width -= fw;
                ws.remove(0);
            }

            // The wrapping whitespace symbol is consumed by the break.
            // (WordWrapper skips the counter update on this path too.)
            if is_ws && ws.is_empty() {
                continue;
            }
        }

        if is_ws {
            ws.push((g, style));
            ws_width += gw;
        } else {
            word.push((g, style));
            word_width += gw;
        }
        non_ws_previous = !is_ws;
    }

    // Tail: emit whatever is still buffered for this input line.  With
    // `trim = false` the pending whitespace run stays on the final row;
    // The renderer only emits a separate blank row for a whitespace-only input
    // when `trim` is set (`&& self.trim` in `WordWrapper::process_input`),
    // so the port must not add one here or the cache gains a phantom row.
    row.append(&mut ws);
    row.append(&mut word);
    if !row.is_empty() {
        rows.push(row);
    } else if rows.is_empty() {
        // Whitespace-independent blank input line: WordWrapper still emits a
        // single (blank) row so blank separator lines keep their height.
        rows.push(vec![]);
    }

    rows.into_iter().map(cells_to_line).collect()
}

/// Convert a wrapped row's grapheme/style cells back into one styled line,
/// collapsing adjacent same-style graphemes into single spans.
fn cells_to_line(cells: Vec<(String, TextStyle)>) -> StyledLine {
    let mut spans: Vec<StyledSpan> = Vec::with_capacity(cells.len());
    let mut text = String::new();
    let mut cur_style: Option<TextStyle> = None;
    for (g, st) in cells {
        if let Some(style) = cur_style.filter(|s| *s != st) {
            if !text.is_empty() {
                spans.push(StyledSpan::styled(std::mem::take(&mut text), style));
            }
        }
        cur_style = Some(st);
        text.push_str(&g);
    }
    if !text.is_empty() {
        spans.push(StyledSpan::styled(text, cur_style.unwrap_or_default()));
    }
    StyledLine::from(spans)
}

/// Plain-text projection of pre-wrapped [`StyledLine`]s for text-selection copy.
pub fn wrapped_lines_to_strings(wrapped: &[StyledLine]) -> Vec<String> {
    wrapped
        .iter()
        .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect::<String>())
        .collect()
}
