//! Markdown table construction for text codemason prints into a transcript.
//!
//! A report laid out as space-padded columns only lines up in a monospace
//! view. The GUI renders the transcript as Markdown, so a `|`-table is the
//! one form that survives both frontends: the Markdown viewer lays the cells
//! out itself, and the plain-text renderer's html2text pass draws it as an
//! ASCII grid.
//!
//! Every cell written through [`cell`] is escaped, so a value carrying a `|`
//! (a shell pipe in a command, an `a|b` in a description) or an embedded line
//! break cannot split its row into extra columns.
//!
//! # Example
//!
//! ```
//! use codemason_types::mdtable::{cell, table};
//!
//! let rows = vec![vec![cell("bash"), cell("Bash tool")]];
//! let rendered = table(&["Name", "Description"], rows);
//! assert_eq!(
//!     rendered,
//!     "| Name | Description |\n| --- | --- |\n| bash | Bash tool |\n"
//! );
//! ```

/// Escape `value` for use as one Markdown table cell.
///
/// A `|` becomes `\|` (otherwise it would start a new column), and every line
/// break or tab becomes a space: a table row is a single line, so a value that
/// arrived with a newline in it would otherwise end its row early.
///
/// # Arguments
///
/// * `value` - the cell text, escaped or not.
///
/// # Returns
///
/// The cell text with its row-breaking characters neutralised.
#[must_use]
pub fn cell(value: &str) -> String {
    value
        .replace('|', "\\|")
        .replace(['\n', '\r'], " ")
        .replace('\t', " ")
}

/// Render one table row, escaping each cell.
///
/// # Arguments
///
/// * `cells` - the row's cell values, in column order.
///
/// # Returns
///
/// `| a | b |` followed by a newline.
#[must_use]
pub fn row<S: AsRef<str>>(cells: &[S]) -> String {
    let mut out = String::from("|");
    for value in cells {
        out.push(' ');
        out.push_str(&cell(value.as_ref()));
        out.push_str(" |");
    }
    out.push('\n');
    out
}

/// Render a header row, its delimiter row, and every body row.
///
/// A table with no body row is still a table (the header alone is valid
/// Markdown), so a report with an empty collection renders its headers rather
/// than nothing.
///
/// # Arguments
///
/// * `headers` - the column titles, in column order.
/// * `rows` - the body rows; each is a list of cell values in column order.
///
/// # Returns
///
/// The complete table, one row per line, ending with a newline.
#[must_use]
pub fn table<S: AsRef<str>, R: IntoIterator<Item = Vec<String>>>(headers: &[S], rows: R) -> String {
    let mut out = row(headers);
    out.push('|');
    for _ in headers {
        out.push_str(" --- |");
    }
    out.push('\n');
    for cells in rows {
        out.push_str(&row(&cells));
    }
    out
}
