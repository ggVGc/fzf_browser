use crate::item::Item;
use crate::ui_state::Ui;
use nucleo::Snapshot;
use ratatui::layout::Rect;

pub fn ui_item_range<'s>(ui: &mut Ui, snap: &'s Snapshot<Item>, item_area: Rect) -> Snapped<'s> {
    item_range(
        snap,
        ui.view_start,
        ui.view_start.saturating_add(u32::from(item_area.height)),
        ui,
    )
}

pub fn item_under_cursor<'s>(ui: &mut Ui, snap: &'s Snapshot<Item>) -> Option<&'s Item> {
    item_range(snap, ui.cursor, ui.cursor + 1, ui)
        .items
        .pop()
        .map(|(_, it)| it)
}

pub fn revalidate_cursor(ui: &mut Ui, snap: &Snapshot<Item>, area: Rect) {
    ui.cursor = ui.cursor.min(snap.matched_item_count().saturating_sub(1));
    ui.cursor_showing = item_under_cursor(ui, snap).cloned();

    if ui.cursor < ui.view_start {
        ui.view_start = ui.cursor;
    } else if ui.cursor + 1 >= ui.view_start + u32::from(area.height) {
        ui.view_start = ui.cursor.saturating_sub(u32::from(area.height)) + 2;
    }
}

pub struct Snapped<'i> {
    pub items: Vec<(u32, &'i Item)>,
    pub matched: u32,
    pub total: u32,
}

#[inline]
pub fn item_range<'s>(
    snap: &'s Snapshot<Item>,
    start: u32,
    mut end: u32,
    ui: &mut Ui,
) -> Snapped<'s> {
    if end > snap.matched_item_count() {
        end = snap.matched_item_count();
    }
    if start >= end {
        return Snapped {
            items: Vec::new(),
            matched: snap.matched_item_count(),
            total: snap.item_count(),
        };
    }

    let sort = ui.input.value().is_empty();
    let items = if !sort {
        snap.matched_items(start..end)
            .map(|item| item.data)
            .enumerate()
            .map(|(pos, item)| (start + pos as u32, item))
            .collect()
    } else {
        let real_end = snap.matched_item_count();
        let cache_end = ui.sorted_items.len() as u32;
        let could_extend = real_end > cache_end;
        let should_extend = end * 2 > cache_end || real_end % 64 == 0;
        let should_sort = end as usize > ui.sorted_until;
        if should_sort || (could_extend && should_extend) {
            ui.sorted_items.extend(cache_end..real_end);

            if end < real_end {
                ui.sorted_items
                    .select_nth_unstable_by_key(end as usize, |&i| {
                        snap.get_item(i).expect("<end").data
                    });
            }

            ui.sorted_items[0..end as usize]
                .sort_unstable_by_key(|&i| snap.get_item(i).expect("<end").data);
            ui.sorted_until = end as usize;
        }

        ui.sorted_items[start as usize..end as usize]
            .iter()
            .enumerate()
            .map(|(pos, &i)| (start + pos as u32, snap.get_item(i).expect("<end").data))
            .collect()
    };

    Snapped {
        items,
        matched: snap.matched_item_count(),
        total: snap.item_count(),
    }
}
