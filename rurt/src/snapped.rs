use crate::item::Item;
use crate::ui_state::{SortedItems, Ui};
use nucleo::Snapshot;

pub fn ui_item_range<'s>(ui: &mut Ui, snap: &'s Snapshot<Item>, len: u32) -> Snapped<'s> {
    item_range(
        snap,
        ui.view_start,
        len,
        should_sort(ui),
        &mut ui.sorted_items,
    )
}

fn one_item<'s>(idx: u32, ui: &mut Ui, snap: &'s Snapshot<Item>) -> Option<&'s Item> {
    item_range(snap, idx, 1, should_sort(ui), &mut ui.sorted_items)
        .items
        .pop()
}

fn should_sort(ui: &Ui) -> bool {
    ui.input.value().is_empty()
}

pub fn revalidate_cursor(ui: &mut Ui, snap: &Snapshot<Item>, len: u32) {
    let mut pos = match one_item(ui.cursor.last_pos, ui, snap) {
        Some(item) if Some(item) == ui.cursor_showing.as_ref() => ui.cursor.last_pos,

        _ => item_range(
            snap,
            0,
            ui.sorted_items.until.saturating_add(64),
            should_sort(ui),
            &mut ui.sorted_items,
        )
        .items
        .into_iter()
        .position(|item| Some(item) == ui.cursor_showing.as_ref())
        .and_then(|i| u32::try_from(i).ok())
        // if it's gone, jump to the start
        .unwrap_or(0),
    };

    let list_end = snap.matched_item_count().saturating_sub(1);
    if let Some(move_req) = ui.cursor.pending_move.take() {
        pos = u32::try_from((pos as isize).saturating_add(move_req))
            .unwrap_or(0)
            .min(list_end)
    }

    ui.cursor.last_pos = pos;

    ui.cursor_showing = item_range(snap, pos, 1, should_sort(ui), &mut ui.sorted_items)
        .items
        .pop()
        .cloned();

    if pos < ui.view_start {
        ui.view_start = pos;
    } else if pos + 1 >= ui.view_start + len {
        ui.view_start = pos.saturating_sub(len) + 2;
    }
}

pub struct Snapped<'i> {
    pub items: Vec<&'i Item>,
    pub start: u32,
    pub matched: u32,
    pub total: u32,
}

fn item_range<'s>(
    snap: &'s Snapshot<Item>,
    start: u32,
    len: u32,
    sort: bool,
    sorted_items: &mut SortedItems,
) -> Snapped<'s> {
    let mut end = start.saturating_add(len);
    if end > snap.matched_item_count() {
        end = snap.matched_item_count();
    }
    if start >= end {
        return Snapped {
            items: Vec::new(),
            start: 0,
            matched: snap.matched_item_count(),
            total: snap.item_count(),
        };
    }

    let items = if !sort {
        snap.matched_items(start..end)
            .map(|item| item.data)
            .collect()
    } else {
        item_range_sorted(snap, start, end, sorted_items)
    };

    Snapped {
        items,
        start,
        matched: snap.matched_item_count(),
        total: snap.item_count(),
    }
}

fn item_range_sorted<'s>(
    snap: &'s Snapshot<Item>,
    start: u32,
    end: u32,
    sorted_items: &mut SortedItems,
) -> Vec<&'s Item> {
    let real_end = snap.matched_item_count();
    let cache_end = sorted_items.items.len() as u32;
    let could_extend = real_end > cache_end;
    let should_extend = end * 2 > cache_end || real_end % 64 == 0;
    let should_sort = end > sorted_items.until;

    if should_sort || (could_extend && should_extend) {
        sorted_items.items.extend(cache_end..real_end);

        let target_until = end.min(100_000);

        if target_until < real_end {
            sorted_items
                .items
                .select_nth_unstable_by_key(target_until as usize, |&i| {
                    snap.get_item(i).expect("<end").data
                });
        }

        sorted_items.items[0..target_until as usize]
            .sort_unstable_by_key(|&i| snap.get_item(i).expect("<end").data);
        sorted_items.until = target_until;
    }

    sorted_items.items[start as usize..end as usize]
        .iter()
        .map(|&i| snap.get_item(i).expect("<end").data)
        .collect()
}
