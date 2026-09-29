use alloy::providers::ProviderBuilder;

mod block_listener;
mod types;

#[tokio::main]
async fn main() {
    let sled_db = sled::open("uniswap_pairs.sled").unwrap();
    let provider = &ProviderBuilder::new()
        .connect("http://127.0.0.1:8485")
        .await
        .unwrap();
    futures::try_join!(
        crate::block_listener::v1_listener(&provider, sled_db.clone()),
        crate::block_listener::v2_listener(&provider, sled_db.clone()),
        crate::block_listener::v3_listener(&provider, sled_db.clone()),
        crate::block_listener::v4_listener(&provider, sled_db.clone())
    )
    .unwrap();
}
