use std::{fs, rc::Rc};

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

mod ui;

fn main() {
    let explorer = ui::Explorer::new().expect("Failed to create explorer");

    let paths = fs::read_dir(".")
        .unwrap()
        .filter_map(|entry| {
            if let Ok(entry) = entry {
                return entry.file_name().into_string().map(SharedString::from).ok();
            }
            None
        })
        .collect::<Vec<SharedString>>();

    explorer.set_per_col(5);
    explorer.set_names(ModelRc::new(Rc::new(VecModel::from(paths))));

    explorer.run().unwrap();
}
