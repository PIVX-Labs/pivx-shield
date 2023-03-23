mod checkpoint;
mod keys;
mod transaction;
mod utils;
use wasm_bindgen::prelude::*;

#[cfg(feature = "multicore")]
pub use wasm_bindgen_rayon::init_thread_pool;

#[wasm_bindgen(start)]
pub fn run() {
    utils::set_panic_hook();
}
