use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use slint::{Image, Model, SharedString, VecModel, Weak};

use crate::icons::Icons;
use crate::items::{Items, UIItem};
use crate::ui::{self, Explorer};

impl From<UIItem> for ui::Item {
    fn from(item: UIItem) -> Self {
        let icon_loaded = item.icon.is_some();
        let icon = item.icon.map(Image::from_rgba8).unwrap_or_default();

        Self {
            id: item.id as i32,
            name: item.name,
            selected: item.selected,
            icon_loaded,
            icon,
        }
    }
}

pub struct State {
    recv: Receiver<Message>,
    offset: usize,
    limit: usize,
    cwd: PathBuf,
    items: Items,
    icons: Icons,
    explorer: Weak<Explorer>,
}

pub enum Message {
    /// UI scroll changed enough to display different elements
    RefreshSlice { offset: usize, limit: usize },
    /// Item clicked
    Open { id: u64 },
    /// Navigate to path subcomponent
    Navigate { subcomponent: usize },
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
                icons: Icons::new(),
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
        let new_items = self.items.slice(self.offset, self.limit, &mut self.icons);
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
                explorer
                    .get_items()
                    .as_any()
                    .downcast_ref::<VecModel<ui::Item>>()
                    .unwrap()
                    .set_vec(
                        new_items
                            .into_iter()
                            .map(|item| item.into())
                            .collect::<Vec<ui::Item>>(),
                    );
                explorer
                    .get_cwd()
                    .as_any()
                    .downcast_ref::<VecModel<SharedString>>()
                    .unwrap()
                    .set_vec(cwd);
            })
            .unwrap();
        self.icons.load();
    }

    fn handle(&mut self, message: Message) {
        match message {
            Message::RefreshSlice { offset, limit } => {
                self.offset = offset;
                self.limit = limit;
                self.update_ui();
            }
            Message::Open { id } => {
                if let Some(path) = self.items.open(id) {
                    self.cwd = path.to_path_buf();
                    self.items.reset();
                    self.update_ui();
                    self.items.load(&self.cwd);
                    self.update_ui();
                }
            }
            Message::Navigate { subcomponent } => {
                self.cwd = self.cwd.components().take(subcomponent + 1).collect();
                self.items.reset();
                self.update_ui();
                self.items.load(&self.cwd);
                self.update_ui();
            }
        }
    }
}
