//! Item sorting

use std::cmp::Ordering;

use slotmap::SlotMap;

use crate::items::{Item, ItemKey, Metadata};

#[derive(Default, Eq, PartialEq)]
pub enum SortBy {
    #[default]
    Name,
    Type,
    Size,
}

#[derive(Default, Eq, PartialEq)]
pub enum SortOrder {
    #[default]
    Ascending,
    Descending,
}

#[derive(Default, Eq, PartialEq)]
pub struct Sort {
    by: SortBy,
    order: SortOrder,
}

impl Sort {
    pub fn compare(&self, a: &(Item, Metadata), b: &(Item, Metadata)) -> Ordering {
        let ordering = match (&a.1, &b.1, &self.by) {
            (
                Metadata::Folder(_) | Metadata::LazyFolder(_),
                Metadata::File(_) | Metadata::LazyFile(_),
                _,
            ) => return Ordering::Less,
            (
                Metadata::File(_) | Metadata::LazyFile(_),
                Metadata::Folder(_) | Metadata::LazyFolder(_),
                _,
            ) => return Ordering::Greater,
            (_, _, SortBy::Name) => a.0.normalized_name.cmp(&b.0.normalized_name),
            _ => Ordering::Equal,
        };

        match self.order {
            SortOrder::Ascending => ordering,
            SortOrder::Descending => ordering.reverse(),
        }
    }

    /// Sort a vector of keys backed by a slotmap
    pub fn sort(&self, slice: &mut [ItemKey], items: &SlotMap<ItemKey, (Item, Metadata)>) {
        slice.sort_by(|a, b| {
            let item_a = &items[*a];
            let item_b = &items[*b];
            self.compare(item_a, item_b)
        });
    }

    /// Insert a key in a vector backed by a slotmap, keeping it sorted
    pub fn insert(
        &self,
        item: &(Item, Metadata),
        vec: &mut Vec<ItemKey>,
        items: &SlotMap<ItemKey, (Item, Metadata)>,
    ) {
        let index = vec
            .binary_search_by(|key| self.compare(&item, &items[*key]))
            .unwrap_or_else(|e| e);
        vec.insert(index, item.0.key);
    }
}
