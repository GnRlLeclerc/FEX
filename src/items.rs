use std::{
    collections::{HashMap, HashSet},
    fs::{self, DirEntry},
    path::{Path, PathBuf},
    sync::Arc,
};

use mime::Mime;
use rayon::{iter::ParallelIterator, slice::ParallelSlice};
use slint::{Rgba8Pixel, SharedPixelBuffer, SharedString};
use slotmap::{Key, KeyData, SlotMap, new_key_type};

use crate::{
    icons::Icons,
    sort::Sort,
    ui::{self, ItemData},
};

new_key_type! {
    pub struct ItemKey;
}

#[derive(Clone)]
pub enum Metadata {
    Folder { children: usize },
    File { mimes: Vec<Mime>, size: u64 },
}

impl Metadata {
    pub fn is_folder(&self) -> bool {
        matches!(self, Self::Folder { .. })
    }
}

/// Item icon.
#[derive(Clone)]
pub enum Icon {
    /// Folders or files with icons.
    /// Loaded on the UI thread, with Slint's builtin path-based cache.
    /// Necessary to handle SVG icons without rasterizing them,
    /// because slint's Image cannot be sent between threads,
    /// and Image cannot be created from SVG content without
    /// processing it again every time.
    ///
    /// Because the path will be copied to the UI thread
    /// every time an item scrolls into view, we use Arc
    /// to make it cheap.
    Path(Arc<PathBuf>),
    /// Images with precomputed thumbnails.
    /// Not cached in the background thread,
    /// because they are likely unique and not reused across images.
    Thumbnail(SharedPixelBuffer<Rgba8Pixel>),
}

pub struct Item {
    pub key: ItemKey,
    path: PathBuf,
    selected: bool,
    pub name: SharedString,
    pub metadata: Metadata,
    pub icon: Icon,
}

pub struct Items {
    sort: Sort,
    icons: Icons,
    items: SlotMap<ItemKey, Item>,
    by_path: HashMap<PathBuf, ItemKey>,
    selected: HashSet<ItemKey>,
    ordered: Vec<ItemKey>,
}

impl Items {
    pub fn new() -> Self {
        Self {
            sort: Sort::default(),
            icons: Icons::new("Papirus".into()),
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

    /// Get a slice of items ready to be sent to the UI
    pub fn slice(&mut self, offset: usize, limit: usize) -> Vec<ItemData> {
        self.ordered
            .iter_mut()
            .skip(offset)
            .take(limit)
            .map(|key| (&self.items[*key]).into())
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
                    .into_iter()
                    .filter_map(|entry| {
                        entry
                            .as_ref()
                            .ok()
                            .and_then(|e| process_entry(e, &self.icons))
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

fn process_entry(entry: &DirEntry, icons: &Icons) -> Option<Item> {
    let name: SharedString = entry.file_name().to_string_lossy().to_string().into();
    let meta = fs::metadata(entry.path()).ok()?;

    let (metadata, icon) = match meta.is_dir() {
        true => (
            Metadata::Folder {
                children: fs::read_dir(entry.path())
                    .map(|iter| iter.count())
                    .unwrap_or(0),
            },
            Icon::Path(icons.get_folder().into()),
        ),
        false => {
            let mimes = icons.get_mimes(&name);
            let icon = icons.get_icon(&mimes);
            (
                Metadata::File {
                    mimes,
                    size: entry.metadata().map(|m| m.len()).unwrap_or(0),
                },
                Icon::Path(icon.into()),
            )
        }
    };

    Some(Item {
        key: ItemKey::null(),
        name,
        path: entry.path(),
        selected: false,
        metadata,
        icon,
    })
}
