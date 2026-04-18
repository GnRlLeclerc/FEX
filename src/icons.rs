//! Loading and caching file icons

use resvg::{
    tiny_skia::Pixmap,
    usvg::{self, Transform, Tree},
};
use std::{
    collections::{HashMap, HashSet},
    fs,
};

use freedesktop_icons::lookup;
use linicon_theme::get_icon_theme;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use slint::{Rgba8Pixel, SharedPixelBuffer, SharedString};

pub struct Icons {
    /// Icons pending to be loaded in the background
    pending: HashSet<SharedString>,
    /// Icon cache by mimetype
    cache: HashMap<SharedString, SharedPixelBuffer<Rgba8Pixel>>,
    theme: String,
    size: u32,
    options: usvg::Options<'static>,
}

impl Icons {
    pub fn new() -> Self {
        Self {
            theme: get_icon_theme().unwrap_or("hicolor".to_string()),
            pending: HashSet::new(),
            cache: HashMap::new(),
            size: 512,
            options: usvg::Options::default(),
        }
    }

    /// Get an icon by mimetype, add the mimetype to the pending list if not already in cache
    pub fn get(&mut self, mime: &SharedString) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
        match self.cache.get(mime) {
            Some(icon) => Some(icon.clone()),
            None => {
                self.pending.insert(mime.clone());
                None
            }
        }
    }

    /// Load pending icons in parallel
    pub fn load(&mut self) {
        let loaded = self
            .pending
            .par_iter()
            .filter_map(|mime| {
                let path = lookup(mime).with_size(512).with_theme(&self.theme).find()?;

                let icon_mime = mime_guess::from_path(&path).first()?;
                let icon = match icon_mime == mime::IMAGE_SVG {
                    true => {
                        let text = fs::read_to_string(&path).ok()?;
                        let tree = Tree::from_str(&text, &self.options).ok()?;
                        let size = tree.size().to_int_size();
                        let mut pixmap =
                            Pixmap::new(size.width().max(1) as u32, size.height().max(1) as u32)?;
                        resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());

                        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                            pixmap.data(),
                            pixmap.width(),
                            pixmap.height(),
                        )
                    }
                    false => {
                        let i = image::open(&path).ok()?.into_rgba8();
                        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&i, i.width(), i.height())
                    }
                };

                Some((mime.to_string().into(), icon))
            })
            .collect::<Vec<_>>();
        self.pending.clear();

        self.cache.extend(loaded);
    }
}
