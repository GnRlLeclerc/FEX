use std::{
    collections::{HashMap, HashSet},
    fs::{self, DirEntry},
    path::{Path, PathBuf},
    sync::Arc,
};

use mime::Mime;
use slint::{Rgba8Pixel, SharedPixelBuffer, SharedString};
use slotmap::{Key, SlotMap, new_key_type};
use unidecode::unidecode;

use crate::{
    icons::Icons,
    sort::Sort,
    ui::{self, ItemData},
};

new_key_type! {
    pub struct ItemKey;
}

// ************************************************************************* //
//                                    STRUCTS                                //
// ************************************************************************* //

pub struct Item {
    pub key: ItemKey,
    pub path: PathBuf,
    pub selected: bool,
    pub normalized_name: String,
    pub name: SharedString,
}

pub enum Metadata {
    File(File),
    Folder(Folder),
    LazyFile(LazyFile),
    LazyFolder(LazyFolder),
}

impl Metadata {
    pub fn is_folder(&self) -> bool {
        matches!(self, Metadata::Folder(_) | Metadata::LazyFolder(_))
    }

    pub fn set_thumbnail(&mut self, buffer: SharedPixelBuffer<Rgba8Pixel>) {
        if let Metadata::File(metadata) = self {
            metadata.icon = Icon::Thumbnail(buffer);
        }
    }

    pub fn should_load_thumbnail(&self) -> bool {
        match self {
            Metadata::File(metadata) => {
                metadata.is_image() && matches!(metadata.icon, Icon::Path(_))
            }
            _ => false,
        }
    }

    pub fn load(&mut self, item: &Item, icons: &mut Icons) {
        match self {
            Metadata::LazyFile(file) => *self = Metadata::File(file.load(item, icons)),
            Metadata::LazyFolder(folder) => *self = Metadata::Folder(folder.load(item, icons)),
            _ => {}
        }
    }
}

// ***************************************************** //
//                   LAZY-LOADED METADATA                //
// ***************************************************** //

pub struct LazyFile {
    pub size: u64,
}

pub struct LazyFolder {}

impl LazyFile {
    pub fn load(&self, item: &Item, icons: &mut Icons) -> File {
        let mimes = icons.get_mimes(&item.name);
        let icon = Icon::Path(Arc::new(icons.get_icon(&mimes)));

        File {
            size: self.size,
            icon,
            mimes,
        }
    }
}

impl LazyFolder {
    pub fn load(&self, item: &Item, icons: &Icons) -> Folder {
        let icon = Icon::Path(icons.get_folder());
        let children = fs::read_dir(&item.path).iter().count();

        Folder { icon, children }
    }
}

// ***************************************************** //
//                  FULLY-LOADED METADATA                //
// ***************************************************** //

pub struct File {
    pub icon: Icon,
    pub mimes: Vec<Mime>,
    pub size: u64,
}

pub struct Folder {
    pub icon: Icon,
    pub children: usize,
}

impl File {
    pub fn is_image(&self) -> bool {
        return self.mimes.iter().any(|mime| mime.type_() == mime::IMAGE);
    }
}

// ***************************************************** //
//                        ITEM ICON                      //
// ***************************************************** //

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

// ************************************************************************* //
//                                    STORAGE                                //
// ************************************************************************* //

/// File explorer items.
///
/// Items are stored in a slotmap.
/// Uses pre-sorted and pre-filtered vectors for efficiency.
pub struct Items {
    sort: Sort,
    sorted: Vec<ItemKey>,
    filter: Option<Vec<String>>,
    filtered: Option<Vec<ItemKey>>,
    icons: Icons,
    items: SlotMap<ItemKey, (Item, Metadata)>,
    by_path: HashMap<PathBuf, ItemKey>,
    selected: HashSet<ItemKey>,
}

impl Items {
    pub fn new() -> Self {
        Self {
            sort: Sort::default(),
            sorted: Vec::new(),
            filter: None,
            filtered: None,
            icons: Icons::new("Papirus".into()),
            items: SlotMap::with_key(),
            by_path: HashMap::new(),
            selected: HashSet::new(),
        }
    }

    pub fn len(&self) -> usize {
        if let Some(filtered) = &self.filtered {
            return filtered.len();
        }
        self.items.len()
    }

    pub fn open(&self, key: ui::ItemKey) -> Option<&Path> {
        if let Some((item, metadata)) = self.items.get(key.into())
            && metadata.is_folder()
        {
            return Some(&item.path);
        }
        None
    }

    pub fn set_thumbnail(&mut self, key: ItemKey, buffer: SharedPixelBuffer<Rgba8Pixel>) {
        self.items[key].1.set_thumbnail(buffer);
    }

    /// Extract a list of images whose thumbnails have not been computed / loaded yet.
    pub fn prepare_thumbnails_for_slice(&self, slice: &[ItemData]) -> Vec<(ItemKey, PathBuf)> {
        slice
            .iter()
            .filter_map(|item| {
                let (item, metadata) = &self.items[item.key];

                match metadata.should_load_thumbnail() {
                    true => Some((item.key, item.path.clone())),
                    false => None,
                }
            })
            .collect()
    }

    pub fn select(&mut self, key: ItemKey) {
        self.selected.insert(key);
        self.items[key].0.selected = true;
    }

    pub fn select_all(&mut self) {
        self.selected = self
            .filtered
            .as_ref()
            .unwrap_or(&self.sorted)
            .iter()
            .cloned()
            .collect();

        for key in &self.selected {
            self.items[*key].0.selected = true;
        }
    }

    pub fn unselect_all(&mut self) {
        for key in self.selected.iter() {
            self.items[*key].0.selected = false;
        }
        self.selected.clear();
    }

    /// Update selection by range, and return the affected keys
    /// for granular update in the frontend.
    pub fn update_selection(&mut self, update: ui::SelectionUpdate) -> Vec<ui::ItemKey> {
        self.filtered
            .as_ref()
            .unwrap_or(&self.sorted)
            .iter()
            .skip(update.range.start as usize)
            .take((update.range.end - update.range.start) as usize)
            .map(|key| {
                self.items[*key].0.selected = update.add;
                match update.add {
                    true => self.selected.insert(*key),
                    false => self.selected.remove(key),
                };
                (*key).into()
            })
            .collect()
    }

    /// Get a slice of items ready to be sent to the UI
    pub fn slice(&mut self, offset: usize, limit: usize) -> Vec<ItemData> {
        self.filtered
            .as_mut()
            .unwrap_or(&mut self.sorted)
            .iter_mut()
            .skip(offset)
            .take(limit)
            .filter_map(|key| {
                let item = &mut self.items[*key];
                item.1.load(&item.0, &mut self.icons);
                (&*item).try_into().ok() // should never fail
            })
            .collect()
    }

    pub fn remove(&mut self, key: ItemKey) {
        if let Some(item) = self.items.remove(key) {
            self.by_path.remove(&item.0.path);
            self.selected.remove(&key);
            self.sorted.retain(|k| *k != key);
            if let Some(filtered) = &mut self.filtered {
                filtered.retain(|k| *k != key);
            }
        }
    }

    pub fn insert(&mut self, item: (Item, Metadata)) {
        let key = self.items.insert(item);
        self.items[key].0.key = key;
        let item = &self.items[key];
        self.by_path.insert(item.0.path.clone(), key);
        self.sort.insert(item, &mut self.sorted, &self.items);
        if let Some(filtered) = &mut self.filtered {
            self.sort.insert(item, filtered, &self.items);
        }
    }

    pub fn reset(&mut self) {
        self.items.clear();
        self.by_path.clear();
        self.selected.clear();
        self.sorted.clear();
        if let Some(filtered) = &mut self.filtered {
            filtered.clear();
        }
    }

    pub fn search(&mut self, search: Option<&str>) {
        self.filter = search.map(|s| {
            s.split_whitespace()
                .map(str::to_lowercase)
                .map(|s| unidecode(&s))
                .collect::<Vec<_>>()
        });
        self.filter();
    }

    pub fn sort(&mut self, sort: Sort) {
        self.sort = sort;
        // 1. Sort
        self.sort.sort(&mut self.sorted, &self.items);
        // 2. Filter
        self.filter();
    }

    /// Update the precomputed filtered vector from the filter params,
    /// using the pre-sorted vector.
    fn filter(&mut self) {
        self.filtered = match &self.filter {
            Some(filter) => Some(
                self.sorted
                    .iter()
                    .filter(|&key| {
                        filter
                            .iter()
                            .all(|s| self.items[*key].0.normalized_name.contains(s))
                    })
                    .cloned()
                    .collect::<Vec<_>>(),
            ),
            None => None,
        };
    }

    pub fn load(&mut self, path: &Path) {
        let read_dir = match fs::read_dir(path) {
            Ok(read_dir) => read_dir,
            Err(err) => {
                log::error!("Could not load folder {:?}: {}", path, err);
                return;
            }
        };

        // Collect items
        let mut items = read_dir
            .filter_map(|entry| entry.ok())
            .filter_map(process_entry)
            .collect::<Vec<_>>();

        // Sort items
        items.sort_by(|a, b| self.sort.compare(a, b));

        // Insert into containers
        items.into_iter().for_each(|item| {
            let key = self.items.insert(item);
            self.items[key].0.key = key;
            let item = &mut self.items[key];
            self.by_path.insert(item.0.path.clone(), key);
            self.sorted.push(key);
        });
    }
}

fn process_entry(entry: DirEntry) -> Option<(Item, Metadata)> {
    let name: SharedString = entry.file_name().to_string_lossy().to_string().into();
    let meta = fs::metadata(entry.path()).ok()?;

    let metadata = match meta.is_dir() {
        true => Metadata::LazyFolder(LazyFolder {}),
        false => Metadata::LazyFile(LazyFile {
            size: entry.metadata().map(|m| m.len()).unwrap_or(0),
        }),
    };

    Some((
        Item {
            key: ItemKey::null(),
            path: entry.path(),
            selected: false,
            normalized_name: unidecode(&name.to_lowercase()),
            name,
        },
        metadata,
    ))
}
