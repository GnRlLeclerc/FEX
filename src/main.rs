use std::{rc::Rc, thread};

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::state::{Message, State};

mod icons;
mod items;
mod state;
mod ui;

fn main() {
    let explorer = ui::Explorer::new().expect("Failed to create explorer");
    explorer.set_items(ModelRc::from(Rc::new(VecModel::default()))); // Initialize with an empty vecmodel

    let weak = explorer.as_weak();
    let (mut state, tx) = State::new(weak);

    explorer.on_refresh_slice(move |offset, limit| {
        tx.send(Message::RefreshSlice {
            offset: offset as usize,
            limit: limit as usize,
        })
        .unwrap();
    });

    let _ = thread::spawn(move || {
        state.event_loop();
    });

    explorer.run().unwrap();
}
