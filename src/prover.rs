#[cfg(not(test))]
use sapling::circuit::{OutputParameters, SpendParameters};
#[cfg(test)]
use sapling::prover::mock::{MockOutputProver, MockSpendProver};

use reqwest::Client;
use std::error::Error;
use tokio::sync::OnceCell;
use wasm_bindgen::prelude::*;

#[cfg(not(test))]
pub type ImplTxProver = (OutputParameters, SpendParameters);

#[cfg(test)]
pub type ImplTxProver = (MockOutputProver, MockSpendProver);

static PROVER: OnceCell<ImplTxProver> = OnceCell::const_new();

pub async fn get_prover() -> &'static ImplTxProver {
    let default_urls = &["https://pivxla.bz", "https://duddino.com"];
    for url in default_urls {
        if let Ok(prover) = get_with_url(url).await {
            return prover;
        }
    }

    panic!("Failed to download prover");
}

/**
 * gets prover using the specified url. If the prover has already been downloaded
 * no request will be made
 */
#[cfg(not(test))]
pub async fn get_with_url(url: &str) -> Result<&'static ImplTxProver, Box<dyn Error>> {
    PROVER
        .get_or_try_init(|| async {
            let c = Client::new();
            let out_url = format!("{}/sapling-output.params", url);
            let spend_url = format!("{}/sapling-spend.params", url);
            let sapling_output_bytes = c.get(&out_url).send().await?.bytes().await?;
            let sapling_spend_bytes = c.get(&spend_url).send().await?.bytes().await?;
            check_and_create_prover(&sapling_output_bytes, &sapling_spend_bytes)
        })
        .await
}

#[cfg(not(test))]
fn check_and_create_prover(
    sapling_output_bytes: &[u8],
    sapling_spend_bytes: &[u8],
) -> Result<ImplTxProver, Box<dyn Error>> {
    if sha256::digest(sapling_output_bytes)
        != "2f0ebbcbb9bb0bcffe95a397e7eba89c29eb4dde6191c339db88570e3f3fb0e4"
    {
        Err("Sha256 does not match for sapling output")?;
    }

    if sha256::digest(sapling_spend_bytes)
        != "8e48ffd23abb3a5fd9c5589204f32d9c31285a04b78096ba40a79b75677efc13"
    {
        Err("Sha256 does not match for sapling spend")?;
    }

    Ok((
        OutputParameters::read(sapling_output_bytes, false)?,
        SpendParameters::read(sapling_spend_bytes, false)?,
    ))
}

#[cfg(not(test))]
pub async fn init_with_bytes(
    sapling_output_bytes: &[u8],
    sapling_spend_bytes: &[u8],
) -> Result<&'static ImplTxProver, Box<dyn Error>> {
    PROVER
        .get_or_try_init(|| async {
            check_and_create_prover(sapling_output_bytes, sapling_spend_bytes)
        })
        .await
}

#[cfg(test)]
pub async fn init_with_bytes(
    _sapling_output_bytes: &[u8],
    _sapling_spend_bytes: &[u8],
) -> Result<&'static ImplTxProver, Box<dyn Error>> {
    Ok(PROVER
        .get_or_init(|| async { (MockOutputProver, MockSpendProver) })
        .await)
}

#[cfg(test)]
pub async fn get_with_url(_url: &str) -> Result<&'static ImplTxProver, Box<dyn Error>> {
    Ok(PROVER
        .get_or_init(|| async { (MockOutputProver, MockSpendProver) })
        .await)
}

#[wasm_bindgen]
pub async fn load_prover() -> bool {
    get_prover().await;
    true
}

#[wasm_bindgen]
pub async fn load_prover_with_url(url: &str) -> bool {
    get_with_url(url).await.is_ok()
}

#[wasm_bindgen]
pub async fn load_prover_with_bytes(
    sapling_output_bytes: &[u8],
    sapling_spend_bytes: &[u8],
) -> bool {
    init_with_bytes(sapling_output_bytes, sapling_spend_bytes)
        .await
        .is_ok()
}

#[wasm_bindgen]
pub fn prover_is_loaded() -> bool {
    PROVER.initialized()
}
