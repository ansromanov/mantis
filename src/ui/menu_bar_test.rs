use super::*;

#[test]
fn menu_hit_ranges_cover_only_their_labels() {
    let area = Rect::new(4, 2, 80, 1);
    let ranges = menu_ranges(area);
    assert_eq!(ranges.len(), MENUS.len());
    for (index, (start, end)) in ranges.iter().enumerate() {
        assert_eq!(menu_index_at(area, *start), Some(index));
        assert_eq!(menu_index_at(area, end.saturating_sub(1)), Some(index));
        if index + 1 < ranges.len() {
            assert_eq!(menu_index_at(area, *end), None);
        }
    }
    assert_eq!(menu_index_at(area, area.x.saturating_sub(1)), None);
}

#[test]
fn menu_hit_testing_rejects_columns_past_the_row() {
    let area = Rect::new(0, 0, 24, 1);
    assert_eq!(menu_index_at(area, 24), None);
}
