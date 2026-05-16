use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use slint::{Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel, Weak};

use crate::items::{ItemKey, Items};
use crate::ui::{self, Explorer};

pub struct State {
    recv: Receiver<Message>,
    offset: usize,
    limit: usize,
    cwd: PathBuf,
    items: Items,
    explorer: Weak<Explorer>,
}

pub enum Message {
    /// UI scroll changed enough to display different elements
    RefreshSlice {
        offset: usize,
        limit: usize,
    },
    /// Item double clicked
    Open {
        key: ui::ItemKey,
    },
    /// Item clicked
    Select {
        /// Whether to unselect all other items
        exclusive: bool,
        key: ui::ItemKey,
    },
    /// Navigate to path subcomponent
    Navigate {
        subcomponent: usize,
    },
    /// Thumbnail loaded for an image
    ThumbnailLoaded {
        key: ItemKey,
        buffer: SharedPixelBuffer<Rgba8Pixel>,
    },
    Search {
        text: SharedString,
    },
    SelectAll,
    UnselectAll,
}

impl State {
    pub fn new(explorer: Weak<Explorer>) -> (Self, Sender<Message>) {
        let (tx, rx) = mpsc::channel();
        let cwd = std::env::current_dir().unwrap();
        let mut items = Items::new();
        items.load(&cwd);

        (
            Self {
                recv: rx,
                offset: 0,
                limit: 0,
                cwd,
                items,
                explorer,
            },
            tx,
        )
    }

    pub fn event_loop(&mut self) {
        while let Ok(message) = self.recv.recv() {
            self.handle(message);
        }
    }

    fn update_ui(&mut self) {
        // 1. Compute items cloned slice to send to the frontend
        let new_items = self.items.slice(self.offset, self.limit);
        let remaining = self.items.len().saturating_sub(self.offset + self.limit);
        let cwd = self
            .cwd
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string().into())
            .collect::<Vec<SharedString>>();

        // 2. Send the cloned slice to the frontend
        self.explorer
            .upgrade_in_event_loop(move |explorer| {
                explorer.set_remaining(remaining as i32);
                downcast_vec(&explorer.get_items()).set_vec(
                    new_items
                        .into_iter()
                        .map(|item| item.into())
                        .collect::<Vec<_>>(),
                );

                downcast_vec(&explorer.get_cwd()).set_vec(cwd);
            })
            .unwrap();
    }

    fn handle(&mut self, message: Message) {
        match message {
            Message::RefreshSlice { offset, limit } => {
                self.offset = offset;
                self.limit = limit;
                self.update_ui();
            }
            Message::Open { key } => {
                if let Some(path) = self.items.open(key) {
                    self.cwd = path.to_path_buf();
                    self.items.reset();
                    self.update_ui();
                    self.items.load(&self.cwd);
                    self.update_ui();
                }
            }
            Message::Select { key, exclusive } => {
                if exclusive {
                    self.items.unselect_all();
                    self.items.select(key.into());
                    self.update_ui();
                } else {
                    self.items.select(key.into());
                }
            }
            Message::Navigate { subcomponent } => {
                self.cwd = self.cwd.components().take(subcomponent + 1).collect();
                self.items.reset();
                self.update_ui();
                self.items.load(&self.cwd);
                self.update_ui();
            }
            Message::ThumbnailLoaded { key, buffer } => {
                let cloned = buffer.clone();
                self.items.set_thumbnail(key, buffer);

                self.explorer
                    .upgrade_in_event_loop(move |explorer| {
                        update_item(&explorer, key, move |item| {
                            item.icon = Image::from_rgba8(cloned)
                        });
                    })
                    .unwrap();
            }
            Message::Search { text } => {
                let search = match text.is_empty() {
                    false => Some(text.as_str()),
                    true => None,
                };

                self.items.search(search);
                self.update_ui();
            }
            Message::SelectAll => {
                self.items.select_all();
                self.update_ui();
            }
            Message::UnselectAll => {
                self.items.unselect_all();
                self.update_ui();
            }
        }
    }
}

fn downcast_vec<T: 'static>(model: &ModelRc<T>) -> &VecModel<T> {
    model.as_any().downcast_ref::<VecModel<T>>().unwrap()
}
/// Update an item by key
fn update_item<F>(explorer: &Explorer, key: ItemKey, update: F)
where
    F: FnOnce(&mut ui::Item),
{
    let key: ui::ItemKey = key.into();
    let items = explorer.get_items();
    let downcasted = downcast_vec(&items);
    if let Some(idx) = downcasted
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            if item.key == key {
                return Some(i);
            } else {
                None
            }
        })
        .next()
        && let Some(row) = downcasted.row_data(idx)
    {
        let mut row = row.clone();
        update(&mut row);
        downcasted.set_row_data(idx, row);
    }
}
