# Run slint live preview (.slint files are live-reloaded)
live-preview *args:
  RUST_LOG=minilauncher=debug SLINT_LIVE_PREVIEW=1 cargo run --features slint/live-preview -- {{args}}
