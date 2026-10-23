//! Rule 13 (low confidence): tables. Two or more consecutive lines split into cells by wide gaps,
//! plus wrapped cell lines that start under a later column (K8s Terminology, GoF Table 1.1).

use super::line_kinds::LineKind;
use super::lines::Line;

const MIN_TABLE_ROWS: usize = 2;
/// A wrapped cell line starts within this distance of a column's left edge.
const COLUMN_ALIGN_TOLERANCE: f32 = 3.0;
const SAME_SIZE_TOLERANCE_RATIO: f32 = 0.05;
/// Kinds a table row may have been given on its own.
const ROW_CANDIDATE_KINDS: [LineKind; 3] = [LineKind::Body, LineKind::NoteBox, LineKind::Heading];

/// Re-marks the table rows among one page's content lines as `TableRow`.
pub fn mark_table_rows(lines: &[&Line], kinds: &mut [LineKind]) {
    let mut start = 0;
    while start < lines.len() {
        let end = table_end(lines, kinds, start);
        let rows = lines[start..end].iter().filter(|line| line.has_cells()).count();
        if rows >= MIN_TABLE_ROWS {
            let first_row = if header_row_above(lines, kinds, start, end) { start - 1 } else { start };
            kinds[first_row..end].fill(LineKind::TableRow);
            start = end;
        } else {
            start += 1;
        }
    }
}

/// A header row whose cell gaps are too narrow to count as cells (K8s `Kubernetes verb(s) HTTP method …`)
/// still has a piece on one of the table's later columns.
fn header_row_above(lines: &[&Line], kinds: &[LineKind], start: usize, end: usize) -> bool {
    let Some(above) = start.checked_sub(1) else { return false };
    let line = lines[above];
    let first = lines[start];
    let later_columns: Vec<f32> = lines[start..end].iter().flat_map(|row| row.cell_lefts.iter().skip(1).copied()).collect();
    let same_size = (line.size - first.size).abs() <= first.size * SAME_SIZE_TOLERANCE_RATIO;
    let on_a_later_column = line.pieces.iter().any(|piece| column_of(&later_columns, piece.left).is_some());
    same_size && ROW_CANDIDATE_KINDS.contains(&kinds[above]) && on_a_later_column
}

/// One past the last line of the table that would start at `start` (equal to `start` if none does).
fn table_end(lines: &[&Line], kinds: &[LineKind], start: usize) -> usize {
    let first = lines[start];
    if !first.has_cells() || !ROW_CANDIDATE_KINDS.contains(&kinds[start]) {
        return start;
    }
    let mut columns = first.cell_lefts.clone();
    let mut end = start + 1;
    while end < lines.len() {
        let line = lines[end];
        let same_size = (line.size - first.size).abs() <= first.size * SAME_SIZE_TOLERANCE_RATIO;
        if !same_size || !ROW_CANDIDATE_KINDS.contains(&kinds[end]) {
            break;
        }
        if line.has_cells() {
            columns.extend(line.cell_lefts.iter().skip(1));
        } else if !line.pieces.iter().any(|piece| column_of(&columns, piece.left).is_some_and(|column| column > 0)) {
            break;
        }
        end += 1;
    }
    end
}

/// The distinct column left edges of a table, left to right.
pub fn table_columns(lines: &[Line]) -> Vec<f32> {
    let mut columns: Vec<f32> = lines.iter().flat_map(|line| line.cell_lefts.iter().copied()).collect();
    columns.sort_by(f32::total_cmp);
    columns.dedup_by(|right, left| (*right - *left).abs() <= COLUMN_ALIGN_TOLERANCE);
    columns
}

fn column_of(columns: &[f32], left: f32) -> Option<usize> {
    columns.iter().position(|column| (column - left).abs() <= COLUMN_ALIGN_TOLERANCE)
}

/// A row's shown text: a new cell starts at each piece that sits on a column edge; cells are joined
/// with ` | ` and a wrapped cell line is indented by its column (` | per user. Runs…`).
pub fn row_text(line: &Line, columns: &[f32]) -> String {
    let mut cells: Vec<(usize, String)> = Vec::new();
    let mut cursor = 0;
    for piece in &line.pieces {
        let piece_text = piece.text.trim();
        let Some(found) = line.text[cursor..].find(piece_text) else { continue };
        let end = cursor + found + piece_text.len();
        let starts_cell = column_of(columns, piece.left).filter(|column| cells.last().is_none_or(|(last, _)| column > last));
        match (starts_cell, cells.last_mut()) {
            (None, Some((_, text))) => text.push_str(&line.text[cursor..end]),
            (column, _) => cells.push((column.unwrap_or(0), line.text[cursor..end].to_string())),
        }
        cursor = end;
    }
    let indent = cells.first().map_or(0, |(column, _)| *column);
    let texts: Vec<&str> = cells.iter().map(|(_, text)| text.trim()).collect();
    format!("{}{}", " | ".repeat(indent), texts.join(" | "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaning::lines::tests::{lines_of, run};

    fn row(cells: &[(&str, f32)], baseline: f32) -> Line {
        let runs: Vec<_> = cells.iter().map(|(text, left)| run(text, *left, baseline, 9.0, "Body")).collect();
        lines_of(&runs).remove(0)
    }

    #[test]
    fn rows_and_wrapped_cells_form_one_table() {
        let lines = [
            row(&[("Term", 60.0), ("Definition", 216.0)], 433.0),
            row(&[("Admission controller", 60.0), ("Code that counts calls", 216.0)], 421.0),
            row(&[("per user. Runs on each node", 216.0)], 410.0),
            row(&[("Annotation", 60.0), ("Object metadata", 216.0)], 379.0),
            row(&[("A normal paragraph after the table.", 54.0)], 350.0),
        ];
        let references: Vec<&Line> = lines.iter().collect();
        let mut kinds = vec![LineKind::Body; lines.len()];
        mark_table_rows(&references, &mut kinds);
        assert_eq!(kinds, [LineKind::TableRow, LineKind::TableRow, LineKind::TableRow, LineKind::TableRow, LineKind::Body]);
        let columns = [60.0, 216.0];
        assert_eq!(row_text(&lines[1], &columns), "Admission controller | Code that counts calls");
        assert_eq!(row_text(&lines[2], &columns), " | per user. Runs on each node");
    }

    #[test]
    fn a_single_split_line_is_not_a_table() {
        let lines = [row(&[("Left", 60.0), ("Right", 300.0)], 400.0), row(&[("Then prose.", 54.0)], 388.0)];
        let references: Vec<&Line> = lines.iter().collect();
        let mut kinds = vec![LineKind::Body; lines.len()];
        mark_table_rows(&references, &mut kinds);
        assert_eq!(kinds, [LineKind::Body, LineKind::Body]);
    }
}
