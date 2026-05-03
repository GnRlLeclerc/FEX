//! Image thumbnails

use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
};

use allmytoes::{AMT, AMTConfiguration, ThumbSize};
use slint::{Rgba8Pixel, SharedPixelBuffer};

use crate::{items::ItemKey, state::Message};

/// Because allmytoes::AMT is not Sync, it cannot be used together
/// with rayon to process thumbnails in parallel.
/// In order to avoid blockage of the main background thread,
/// this instance lives in another thread and is accessed through channels.
pub struct Thumbnails {
    amt: AMT,
    size: ThumbSize,
    tx: Sender<Message>,
    rx: Receiver<Vec<(ItemKey, PathBuf)>>,
}

impl Thumbnails {
    /// Create a new thumbnail processing instance.
    /// Call this function in the background thread where the processing
    /// should happen.
    pub fn new(rx: Receiver<Vec<(ItemKey, PathBuf)>>, tx: Sender<Message>) -> Self {
        Self {
            amt: AMT::new(&AMTConfiguration::default()),
            size: ThumbSize::XLarge,
            tx,
            rx,
        }
    }

    /// Run the thumbnail processing even loop
    pub fn event_loop(&mut self) {
        while let Ok(paths) = self.rx.recv() {
            for (key, path) in paths {
                match self.amt.get(&path, self.size) {
                    Ok(thumbnail) => match image::open(&thumbnail.path) {
                        Ok(image) => {
                            let image = image.to_rgba8();
                            let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                                &image,
                                image.width(),
                                image.height(),
                            );

                            if let Err(err) = self.tx.send(Message::ThumbnailLoaded { key, buffer })
                            {
                                log::error!("Failed to send loaded thumbnail: {:?}", err);
                            }
                        }
                        Err(err) => log::error!(
                            "Failed to open thumbnail image at {:?}: {:?}",
                            &thumbnail.path,
                            err
                        ),
                    },
                    Err(err) => log::error!("Failed to load thumbnail for {:?}: {:?}", path, err),
                }
            }
        }
    }
}
