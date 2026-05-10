use std::{rc::Rc, thread};

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::state::{Message, State};

mod config;
mod icons;
mod items;
mod search;
mod sort;
mod state;
mod thumbnails;
mod ui;

fn main() {
    let explorer = ui::Explorer::new().expect("Failed to create explorer");
    explorer.set_items(ModelRc::from(Rc::new(VecModel::default()))); // Initialize with an empty vecmodel
    explorer.set_cwd(ModelRc::from(Rc::new(VecModel::default()))); // Initialize with an empty vecmodel

    let weak = explorer.as_weak();
    let (mut state, tx) = State::new(weak);

    let txc = tx.clone();
    explorer.on_refresh(move |offset, limit| {
        txc.send(Message::RefreshSlice {
            offset: offset as usize,
            limit: limit as usize,
        })
        .unwrap();
    });

    let txc = tx.clone();
    explorer.on_open(move |key| {
        txc.send(Message::Open { key }).unwrap();
    });

    let txc = tx.clone();
    explorer.on_navigate(move |subcomponent| {
        txc.send(Message::Navigate {
            subcomponent: subcomponent as usize,
        })
        .unwrap();
    });

    let txc = tx.clone();
    explorer.on_search(move |text| {
        txc.send(Message::Search { text }).unwrap();
    });

    let _ = thread::spawn(move || {
        state.event_loop();
    });

    explorer.run().unwrap();
}
