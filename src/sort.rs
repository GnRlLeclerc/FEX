//! Item sorting

use std::cmp::Ordering;

use crate::items::{Item, Metadata};

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
            (_, _, SortBy::Name) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
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
}
