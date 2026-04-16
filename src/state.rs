use std::sync::mpsc::{self, Receiver, Sender};
use std::{fs, path::PathBuf};

use slint::{ModelRc, SharedString, VecModel, Weak};

use crate::ui::Explorer;

pub struct Item {
    is_dir: bool,
    name: SharedString,
}

pub struct State {
    recv: Receiver<Message>,
    offset: usize,
    limit: usize,
    cwd: PathBuf,
    items: Vec<Item>,
    explorer: Weak<Explorer>,
}

pub enum Message {
    /// UI scroll changed enough to display different elements
    RefreshSlice { offset: usize, limit: usize },
}

impl State {
    pub fn new(explorer: Weak<Explorer>) -> (Self, Sender<Message>) {
        let (tx, rx) = mpsc::channel();
        (
            Self {
                recv: rx,
                offset: 0,
                limit: 0,
                items: Vec::new(),
                cwd: std::env::current_dir().unwrap(),
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

    pub fn load(&mut self) {
        self.items = fs::read_dir(&self.cwd)
            .unwrap()
            .filter_map(|entry| {
                if let Ok(entry) = entry
                    && let Ok(name) = entry.file_name().into_string().map(SharedString::from)
                    && let Ok(file_type) = entry.file_type()
                {
                    return Some(Item {
                        name,
                        is_dir: file_type.is_dir(),
                    });
                }
                None
            })
            .collect();
        self.items.sort_by_key(|item| item.name.to_lowercase());
    }

    fn update_ui(&self) {
        // 1. Compute names slice to send to the frontend
        let names = match self.items.len() <= self.offset {
            true => vec![],
            false => {
                let end = (self.limit + self.offset).min(self.items.len());

                self.items[self.offset..end]
                    .iter()
                    .map(|item| item.name.clone())
                    .collect::<Vec<_>>()
            }
        };
        let remaining = self.items.len().saturating_sub(self.offset + self.limit);

        // 2. Send the cloned slice to the frontend
        self.explorer
            .upgrade_in_event_loop(move |explorer| {
                explorer.set_remaining(remaining as i32);
                explorer.set_names(ModelRc::new(VecModel::from(names)));
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
        }
    }
}
