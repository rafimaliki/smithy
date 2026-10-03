//! Column math for a monospace grid where a tab is shown as `TAB_WIDTH` columns.
pub const TAB_WIDTH: usize = 4;

/// Display column of char column `col` in `line`.
pub fn visual_col(line: &str, col: usize) -> usize {
    line.chars()
        .take(col)
        .map(|c| if c == '\t' { TAB_WIDTH } else { 1 })
        .sum()
}

/// Char column whose cell contains display column `vcol` (rounded to the nearer edge).
pub fn col_at_visual(line: &str, vcol: usize) -> usize {
    let mut v = 0;
    for (i, c) in line.chars().enumerate() {
        let w = if c == '\t' { TAB_WIDTH } else { 1 };
        if vcol < v + w {
            return if vcol * 2 >= v * 2 + w { i + 1 } else { i };
        }
        v += w;
    }
    line.chars().count()
}

/// The line as drawn: tabs become spaces.
pub fn expand_tabs(line: &str) -> String {
    line.replace('\t', &" ".repeat(TAB_WIDTH))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_count_as_four_columns_both_ways() {
        let l = "\tab";
        assert_eq!(visual_col(l, 1), 4);
        assert_eq!(visual_col(l, 3), 6);
        assert_eq!(col_at_visual(l, 5), 2);
        assert_eq!(col_at_visual(l, 99), 3);
        assert_eq!(col_at_visual(l, 0), 0);
        assert_eq!(expand_tabs(l), "    ab");
    }
}
