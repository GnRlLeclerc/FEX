use std::thread;

use slint::ComponentHandle;

use crate::state::{Message, State};

mod state;
mod ui;

fn main() {
    let explorer = ui::Explorer::new().expect("Failed to create explorer");

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
        state.load();
        state.event_loop();
    });

    explorer.run().unwrap();
}
