use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use mime::Mime;
use rayon::iter::{ParallelBridge, ParallelIterator};
use slint::{Rgba8Pixel, SharedPixelBuffer, SharedString};
use slotmap::{SlotMap, new_key_type};

use crate::icons::Icons;

new_key_type! {
    pub struct ItemKey;
}

#[derive(Clone)]
pub enum Metadata {
    Folder { children: usize },
    File { mime: Option<Mime>, size: u64 },
}

impl Metadata {
    pub fn is_folder(&self) -> bool {
        matches!(self, Self::Folder { .. })
    }
}

#[derive(Default, Eq, PartialEq)]
pub enum SortBy {
    #[default]
    Name,
    Type,
    Size,
}

#[derive(Default, Eq, PartialEq)]
pub enum SortOrder {
    Ascending,
    #[default]
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
            (_, _, SortBy::Name) => a.name.cmp(&b.name),
            (Metadata::File { size: a, .. }, Metadata::File { size: b, .. }, SortBy::Size) => {
                a.cmp(b)
            }
            (Metadata::Folder { children: a }, Metadata::Folder { children: b }, SortBy::Size) => {
                a.cmp(b)
            }
            (Metadata::File { mime: a, .. }, Metadata::File { mime: b, .. }, SortBy::Type) => {
                a.cmp(b)
            }
            _ => Ordering::Equal,
        };

        match self.order {
            SortOrder::Ascending => ordering,
            SortOrder::Descending => ordering.reverse(),
        }
    }
}

struct Item {
    name: SharedString,
    path: PathBuf,
    selected: bool,
    metadata: Metadata,
    /// Lazily computed icon
    icon: Option<SharedPixelBuffer<Rgba8Pixel>>,
}

/// A more lightweight version of Item, to be cloned and sent to the UI
/// (basically, the PathBuf has been removed)
pub struct UIItem {
    pub name: SharedString,
    pub selected: bool,
    pub metadata: Metadata,
    /// Lazily computed icon
    pub icon: Option<SharedPixelBuffer<Rgba8Pixel>>,
}

impl From<&Item> for UIItem {
    fn from(item: &Item) -> Self {
        Self {
            name: item.name.clone(),
            selected: item.selected,
            metadata: item.metadata.clone(),
            icon: item.icon.clone(),
        }
    }
}

impl Item {
    pub fn try_load_icon(&mut self, icons: &mut Icons) {
        if self.icon.is_none() {
            let mime = match &self.metadata {
                Metadata::Folder { .. } => "folder".to_string(),
                Metadata::File { mime, .. } => mime
                    .as_ref()
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            }
            .into();
            self.icon = icons.get(&mime);
        }
    }
}

pub struct Items {
    sort: Sort,
    items: SlotMap<ItemKey, Item>,
    by_path: HashMap<PathBuf, ItemKey>,
    selected: HashSet<ItemKey>,
    ordered: Vec<ItemKey>,
}

impl Items {
    pub fn new() -> Self {
        Self {
            sort: Sort::default(),
            items: SlotMap::with_key(),
            by_path: HashMap::new(),
            selected: HashSet::new(),
            ordered: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn select_all(&mut self) {
        self.selected = self.items.keys().collect();
    }

    pub fn unselect_all(&mut self) {
        for key in self.selected.iter() {
            self.items[*key].selected = false;
        }
        self.selected.clear();
    }

    /// Get a slice of items ready to be sent to the UI, and trigger icon lazy loading
    pub fn slice(&mut self, offset: usize, limit: usize, icons: &mut Icons) -> Vec<UIItem> {
        self.ordered
            .iter_mut()
            .skip(offset)
            .take(limit)
            .map(|key| {
                let item = &mut self.items[*key];
                item.try_load_icon(icons);
                (&*item).into()
            })
            .collect()
    }

    pub fn remove(&mut self, key: ItemKey) {
        if let Some(item) = self.items.remove(key) {
            self.by_path.remove(&item.path);
            self.selected.remove(&key);
            self.ordered.retain(|k| *k != key);
        }
    }

    pub fn add(&mut self, item: Item) -> ItemKey {
        let key = self.items.insert(item);
        let item = &self.items[key];
        self.by_path.insert(item.path.clone(), key);

        let index = self
            .ordered
            .binary_search_by(|key| self.sort.compare(&item, &self.items[*key]))
            .unwrap_or_else(|e| e);
        self.ordered.insert(index, key);

        key
    }

    pub fn reset(&mut self) {
        self.items.clear();
        self.by_path.clear();
        self.selected.clear();
        self.ordered.clear();
    }

    pub fn sort(&mut self, sort: Sort) {
        if self.sort == sort {
            return;
        }

        self.sort = sort;
        self.ordered.sort_by(|a, b| {
            let item_a = &self.items[*a];
            let item_b = &self.items[*b];
            self.sort.compare(item_a, item_b)
        });
    }

    pub fn load(&mut self, path: &Path) {
        let mut items = fs::read_dir(path)
            .unwrap()
            .par_bridge()
            .filter_map(|entry| {
                if let Ok(entry) = entry
                    && let Ok(name) = entry.file_name().into_string().map(SharedString::from)
                    && let Ok(file_type) = entry.file_type()
                {
                    let metadata = match file_type.is_dir() {
                        true => Metadata::Folder {
                            children: fs::read_dir(entry.path())
                                .map(|iter| iter.count())
                                .unwrap_or(0),
                        },
                        false => Metadata::File {
                            mime: mime_guess::from_path(entry.path()).first(),
                            size: entry.metadata().map(|m| m.len()).unwrap_or(0),
                        },
                    };

                    return Some(Item {
                        name,
                        path: entry.path(),
                        selected: false,
                        metadata,
                        icon: None,
                    });
                }
                None
            })
            .collect::<Vec<_>>();

        // Sort items
        items.sort_by(|a, b| self.sort.compare(a, b));

        // Insert into containers
        items.into_iter().for_each(|item| {
            let key = self.items.insert(item);
            let item = &self.items[key];
            self.by_path.insert(item.path.clone(), key);
            self.ordered.push(key);
        });
    }
}
