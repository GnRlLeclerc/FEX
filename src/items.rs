use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use mime_guess::MimeGuess;
use rayon::{iter::ParallelIterator, slice::ParallelSlice};
use slint::{Rgba8Pixel, SharedPixelBuffer, SharedString};
use slotmap::{Key, KeyData, SlotMap, new_key_type};

use crate::{icons::Icons, ui};

new_key_type! {
    pub struct ItemKey;
}

#[derive(Clone)]
pub enum Metadata {
    Folder { children: usize },
    File { mime: MimeGuess, size: u64 },
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
            (Metadata::File { mime: a, .. }, Metadata::File { mime: b, .. }, SortBy::Type) => {
                a.iter_raw().cmp(b.iter_raw())
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
    key: ItemKey,
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
    pub key: ui::ItemKey,
    pub name: SharedString,
    pub selected: bool,
    pub metadata: Metadata,
    /// Lazily computed icon
    pub icon: Option<SharedPixelBuffer<Rgba8Pixel>>,
}

impl From<&Item> for UIItem {
    fn from(item: &Item) -> Self {
        let i = item.key.data().as_ffi();
        Self {
            key: ui::ItemKey {
                lower: i as i32,
                upper: (i >> 32) as i32,
            },
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
            match &self.metadata {
                Metadata::Folder { .. } => self.icon = icons.get("folder"),
                Metadata::File { mime, .. } => {
                    if mime.is_empty() {
                        self.icon = icons.get("application-x-core");
                    } else {
                        for mime in mime.iter_raw() {
                            if let Some(icon) = icons.get(mime.replace('/', "-").as_str()) {
                                self.icon = Some(icon);
                                break;
                            }
                        }
                    }
                }
            };
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

    pub fn open(&self, key: ui::ItemKey) -> Option<&Path> {
        let i = ((key.upper as u64) << 32) | (key.lower as u64);
        let key = ItemKey::from(KeyData::from_ffi(i));
        if let Some(item) = self.items.get(key)
            && let Metadata::Folder { .. } = item.metadata
        {
            return Some(&item.path);
        }
        None
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

    pub fn add(&mut self, item: Item) {
        let key = self.items.insert(item);
        self.items[key].key = key;
        let item = &self.items[key];
        self.by_path.insert(item.path.clone(), key);

        let index = self
            .ordered
            .binary_search_by(|key| self.sort.compare(&item, &self.items[*key]))
            .unwrap_or_else(|e| e);
        self.ordered.insert(index, key);
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
        // 1. Collect entries to be processed
        let entries = fs::read_dir(path).unwrap().collect::<Vec<_>>();

        // 2. Process entries in parallel and collect them
        let mut items = entries
            .par_chunks(10_000)
            .flat_map(|chunk| {
                chunk
                    .iter()
                    .filter_map(|entry| {
                        if let Ok(entry) = entry
                            && let Ok(name) =
                                entry.file_name().into_string().map(SharedString::from)
                            && let Ok(file_type) = entry.file_type()
                        {
                            let metadata = match file_type.is_dir() {
                                true => Metadata::Folder {
                                    children: fs::read_dir(entry.path())
                                        .map(|iter| iter.count())
                                        .unwrap_or(0),
                                },
                                false => Metadata::File {
                                    mime: mime_guess::from_path(entry.path()),
                                    size: entry.metadata().map(|m| m.len()).unwrap_or(0),
                                },
                            };

                            return Some(Item {
                                key: ItemKey::null(),
                                name,
                                path: entry.path(),
                                selected: false,
                                metadata,
                                icon: None,
                            });
                        }
                        None
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        // Sort items
        items.sort_by(|a, b| self.sort.compare(a, b));

        // Insert into containers
        items.into_iter().for_each(|item| {
            let key = self.items.insert(item);
            self.items[key].key = key;
            let item = &mut self.items[key];
            self.by_path.insert(item.path.clone(), key);
            self.ordered.push(key);
        });
    }
}
