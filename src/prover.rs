use pivx_primitives::sapling::prover::TxProver;

#[cfg(test)]
use pivx_primitives::sapling::prover::mock::MockTxProver;
#[cfg(not(test))]
use pivx_proofs::prover::LocalTxProver;

use reqwest::Client;
use std::error::Error;
use tokio::sync::OnceCell;
use wasm_bindgen::prelude::*;

#[cfg(not(test))]
type ImplTxProver = LocalTxProver;

#[cfg(test)]
type ImplTxProver = MockTxProver;

static PROVER: OnceCell<ImplTxProver> = OnceCell::const_new();

pub async fn get_prover() -> &'static impl TxProver {
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
pub async fn get_with_url(url: &str) -> Result<&'static impl TxProver, Box<dyn Error>> {
    PROVER
        .get_or_try_init(|| async {
            let c = Client::new();
            let out_url = format!("{}/sapling-output.params", url);
            let spend_url = format!("{}/sapling-spend.params", url);
            let sapling_output_bytes = c.get(&out_url).send().await?.bytes().await?;
            let sapling_spend_bytes = c.get(&spend_url).send().await?.bytes().await?;

            if sha256::digest(&*sapling_output_bytes)
                != "2f0ebbcbb9bb0bcffe95a397e7eba89c29eb4dde6191c339db88570e3f3fb0e4"
            {
                Err("Sha256 does not match for sapling output")?;
            }

            if sha256::digest(&*sapling_spend_bytes)
                != "8e48ffd23abb3a5fd9c5589204f32d9c31285a04b78096ba40a79b75677efc13"
            {
                Err("Sha256 does not match for sapling spend")?;
            }
            Ok(LocalTxProver::from_bytes(
                &sapling_spend_bytes,
                &sapling_output_bytes,
            ))
        })
        .await
}

#[cfg(test)]
pub async fn get_with_url(_url: &str) -> Result<&'static impl TxProver, Box<dyn Error>> {
    Ok(PROVER.get_or_init(|| async { MockTxProver }).await)
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
