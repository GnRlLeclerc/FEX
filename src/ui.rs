use slint::{Image, SharedString};
use slotmap::{Key, KeyData};

use crate::items::{self, Icon};

slint::include_modules!();

/// Item data is passed from the background thread to the UI thread
/// by copy, because slint's upgrade_in_event_loop runs a non-blocking
/// closures, making it impossible to track lifetimes for borrowing.
///
/// However, there are 2 problems:
/// - ui::Item can only be instanciated on the UI thread because it holds
///   a slint::Image instance that cannot be sent between threads.
/// - items::Item holds additional data not useful for display that may
///   be heavy to copy.
///
/// The solution is the ItemData struct, an intermediary struct that copies
/// only the needed fields from items::Item, and that can be sent between
/// threads.
pub struct ItemData {
    key: items::ItemKey,
    name: SharedString,
    folder: bool,
    icon: Icon,
}

impl From<&items::Item> for ItemData {
    fn from(item: &items::Item) -> Self {
        Self {
            key: item.key,
            name: item.name.clone(),
            folder: item.metadata.is_folder(),
            icon: item.icon.clone(),
        }
    }
}

impl From<ItemData> for Item {
    fn from(item: ItemData) -> Self {
        Self {
            key: item.key.into(),
            name: item.name,
            selected: false,
            icon: item.icon.into(),
        }
    }
}

impl From<Icon> for Image {
    fn from(icon: Icon) -> Self {
        match icon {
            Icon::Path(path) => Image::load_from_path(&path).unwrap_or_default(),
            Icon::Thumbnail(buffer) => Image::from_rgba8(buffer),
        }
    }
}

impl From<ItemKey> for items::ItemKey {
    fn from(key: ItemKey) -> Self {
        let i = ((key.upper as u64) << 32) | (key.lower as u64);
        Self::from(KeyData::from_ffi(i))
    }
}

impl From<items::ItemKey> for ItemKey {
    fn from(key: items::ItemKey) -> Self {
        let i = key.data().as_ffi();
        Self {
            lower: i as i32,
            upper: (i >> 32) as i32,
        }
    }
}
