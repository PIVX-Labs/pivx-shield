mod checkpoint;
mod keys;
mod transaction;
mod utils;

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

pub use wasm_bindgen_rayon::init_thread_pool;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn run() {
    utils::set_panic_hook();
}
