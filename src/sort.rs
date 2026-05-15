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
    pub fn compare(&self, a: &Item, b: &Item) -> Ordering {
        let ordering = match (&a.metadata, &b.metadata, &self.by) {
            (Metadata::Folder { .. }, Metadata::File { .. }, _) => return Ordering::Less,
            (Metadata::File { .. }, Metadata::Folder { .. }, _) => return Ordering::Greater,
            (_, _, SortBy::Name) => a.normalized_name.cmp(&b.normalized_name),
            (Metadata::File { size: a, .. }, Metadata::File { size: b, .. }, SortBy::Size) => {
                a.cmp(b)
            }
            (Metadata::Folder { children: a }, Metadata::Folder { children: b }, SortBy::Size) => {
                a.cmp(b)
            }
            (Metadata::File { mimes: a, .. }, Metadata::File { mimes: b, .. }, SortBy::Type) => {
                a.iter().cmp(b.iter())
            }
            _ => Ordering::Equal,
        };

        match self.order {
            SortOrder::Ascending => ordering,
            SortOrder::Descending => ordering.reverse(),
        }
    }

    /// Sort a vector of keys backed by a slotmap
    pub fn sort(&self, slice: &mut [ItemKey], items: &SlotMap<ItemKey, Item>) {
        slice.sort_by(|a, b| {
            let item_a = &items[*a];
            let item_b = &items[*b];
            self.compare(item_a, item_b)
        });
    }

    /// Insert a key in a vector backed by a slotmap, keeping it sorted
    pub fn insert(&self, item: &Item, vec: &mut Vec<ItemKey>, items: &SlotMap<ItemKey, Item>) {
        let index = vec
            .binary_search_by(|key| self.compare(&item, &items[*key]))
            .unwrap_or_else(|e| e);
        vec.insert(index, item.key);
    }
}
