//! Loading file icons and image thumbnails

use mime::Mime;
use std::{collections::HashMap, path::PathBuf};
use xdg_mime::SharedMimeInfo;

use freedesktop_icons::lookup;

/// Icon loader
pub struct Icons {
    db: SharedMimeInfo,
    theme: String,
    size: u16,
    folder: PathBuf,
    cache: HashMap<Vec<Mime>, PathBuf>,
}

impl Icons {
    pub fn new(theme: String) -> Self {
        let db = SharedMimeInfo::new();
        let size = 64;
        let folder = lookup("folder")
            .with_cache()
            .with_theme(&theme)
            .with_size(size)
            .find()
            .expect("Failed to find folder icon in the specified theme.");

        Self {
            db,
            theme,
            size,
            folder,
            cache: HashMap::new(),
        }
    }

    pub fn get_mimes(&self, name: &str) -> Vec<Mime> {
        self.db.get_mime_types_from_file_name(name)
    }

    /// Get the default folder icon
    pub fn get_folder(&self) -> PathBuf {
        self.folder.clone()
    }

    /// Get the icon path for a file
    pub fn get_icon(&mut self, mimes: &[Mime]) -> PathBuf {
        if let Some(icon) = self.cache.get(mimes) {
            return icon.clone();
        }

        let icon = self.get_icon_no_cache(mimes);
        self.cache.insert(mimes.into(), icon.clone());
        return icon;
    }

    fn get_icon_no_cache(&self, mimes: &[Mime]) -> PathBuf {
        // 1. Look for matching icons
        for mime in mimes {
            let icons = self.db.lookup_icon_names(mime);
            for icon in icons {
                if let Some(path) = lookup(&icon)
                    .with_cache()
                    .with_theme(&self.theme)
                    .with_size(self.size)
                    .find()
                {
                    return path;
                }
            }
        }

        // 2. Look for fallback generic icons
        for mime in mimes {
            if let Some(icon) = self.db.lookup_generic_icon_name(mime) {
                if let Some(path) = lookup(&icon)
                    .with_cache()
                    .with_theme(&self.theme)
                    .with_size(self.size)
                    .find()
                {
                    return path;
                }
            }
        }

        unreachable!("Failed to default to a generic icon");
    }
}
