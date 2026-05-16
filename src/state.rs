use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use slint::{Image, Model, Rgba8Pixel, SharedPixelBuffer, SharedString, Weak};

use crate::items::{ItemKey, Items};
use crate::ui::{self, Explorer, downcast_vec, update_items};

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
        key: ui::ItemKey,
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
        let _ = self.explorer.upgrade_in_event_loop(move |explorer| {
            explorer.set_remaining(remaining as i32);
            downcast_vec(&explorer.get_items()).set_vec(
                new_items
                    .into_iter()
                    .map(|item| item.into())
                    .collect::<Vec<_>>(),
            );

            downcast_vec(&explorer.get_cwd()).set_vec(cwd);
        });
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
                    self.items.select(key.clone().into());
                    let _ = self.explorer.upgrade_in_event_loop(move |explorer| {
                        update_items(
                            &explorer.get_items(),
                            |item| {
                                (item.key == key && !item.selected)
                                    || (item.key != key && item.selected)
                            },
                            |item| item.selected = item.key == key,
                        );
                    });
                } else {
                    self.items.select(key.clone().into());
                    let _ = self.explorer.upgrade_in_event_loop(move |explorer| {
                        update_items(
                            &explorer.get_items(),
                            |item| item.key == key,
                            |item| item.selected = true,
                        );
                    });
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
                self.items.set_thumbnail(key.clone().into(), buffer.clone());

                self.explorer
                    .upgrade_in_event_loop(move |explorer| {
                        update_items(
                            &explorer.get_items(),
                            |item| item.key == key,
                            move |item| item.icon = Image::from_rgba8(buffer.clone()),
                        );
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
                let _ = self.explorer.upgrade_in_event_loop(|explorer| {
                    update_items(&explorer.get_items(), |_| true, |item| item.selected = true);
                });
            }
            Message::UnselectAll => {
                self.items.unselect_all();
                let _ = self.explorer.upgrade_in_event_loop(|explorer| {
                    update_items(
                        &explorer.get_items(),
                        |_| true,
                        |item| item.selected = false,
                    );
                });
            }
        }
    }
}
