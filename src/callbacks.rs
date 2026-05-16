//! Register callbacks

use std::sync::mpsc::Sender;

use slint::ComponentHandle;

use crate::{
    state::Message,
    ui::{Callbacks, Explorer},
};

pub fn register_callbacks(explorer: &Explorer, tx: Sender<Message>) {
    let callbacks = explorer.global::<Callbacks>();

    let txc = tx.clone();
    callbacks.on_refresh_entries(move |offset, limit| {
        txc.send(Message::RefreshSlice {
            offset: offset as usize,
            limit: limit as usize,
        })
        .unwrap();
    });

    let txc = tx.clone();
    callbacks.on_open(move |key| {
        txc.send(Message::Open { key }).unwrap();
    });

    let txc = tx.clone();
    callbacks.on_navigate(move |subcomponent| {
        txc.send(Message::Navigate {
            subcomponent: subcomponent as usize,
        })
        .unwrap();
    });

    let txc = tx.clone();
    callbacks.on_search(move |text| {
        txc.send(Message::Search { text }).unwrap();
    });

    let txc = tx.clone();
    callbacks.on_select(move |key, exclusive| {
        txc.send(Message::Select { key, exclusive }).unwrap();
    });

    let txc = tx.clone();
    callbacks.on_select_all(move |select| {
        txc.send(match select {
            true => Message::SelectAll,
            false => Message::UnselectAll,
        })
        .unwrap();
    });

    let txc = tx.clone();
    callbacks.on_select_update(move |update| {
        txc.send(Message::SelectUpdate(update)).unwrap();
    });
}
