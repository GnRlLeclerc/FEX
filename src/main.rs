use std::sync::mpsc::channel;
use std::{rc::Rc, thread};

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::thumbnails::Thumbnails;
use crate::{callbacks::register_callbacks, state::State};

mod callbacks;
mod config;
mod icons;
mod items;
mod sort;
mod state;
mod thumbnails;
mod ui;

fn main() {
    let explorer = ui::Explorer::new().expect("Failed to create explorer");
    explorer.set_items(ui::Items {
        items: ModelRc::from(Rc::new(VecModel::default())),
        remaining: 0,
        offset: 0,
    }); // Initialize with an empty vecmodel
    explorer.set_cwd(ModelRc::from(Rc::new(VecModel::default()))); // Initialize with an empty vecmodel

    let weak = explorer.as_weak();
    let (tx, rx) = channel();
    let (tx2, rx2) = channel();
    let mut state = State::new(weak, rx, tx2);
    let mut thumbnails = Thumbnails::new(rx2, tx.clone());

    register_callbacks(&explorer, tx);

    let _ = thread::spawn(move || {
        state.event_loop();
    });

    let _ = thread::spawn(move || {
        thumbnails.event_loop();
    });

    explorer.run().unwrap();
}
