use std::collections::HashMap;

use crate::error::ServerError;
use crate::types::*;
use alloy::{
    primitives::Address,
    providers::Provider,
    rpc::types::{Filter, Log},
    sol,
    sol_types::{SolEvent, SolEventInterface},
};
use chrono::Utc;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use sled::Db;

sol! {
    // ===== Uniswap V1 (Factory + Exchange) =====
    // V1 had no Swap event; only Mint, Burn, EthPurchase, TokenPurchase.
    event TokenPurchase(
        address indexed buyer,
        uint256 eth_sold,
        uint256 tokens_bought
    );

    event EthPurchase(
        address indexed buyer,
        uint256 tokens_sold,
        uint256 eth_bought
    );

    event AddLiquidity(
        address indexed provider,
        uint256 eth_amount,
        uint256 token_amount
    );

    event RemoveLiquidity(
        address indexed provider,
        uint256 eth_amount,
        uint256 token_amount
    );

    event Transfer(
        address indexed from,
        address indexed to,
        uint256 value
    );

    event Approval(
        address indexed owner,
        address indexed spender,
        uint256 value
    );

    // ===== Uniswap V2 (Pair) =====
    event Swap(
        address indexed sender,
        uint256 amount0In,
        uint256 amount1In,
        uint256 amount0Out,
        uint256 amount1Out,
        address indexed to
    );

    event Sync(uint112 reserve0, uint112 reserve1);

    event Mint(address indexed sender, uint256 amount0, uint256 amount1);

    event Burn(
        address indexed sender,
        uint256 amount0,
        uint256 amount1,
        address indexed to
    );

    // ===== Uniswap V3 (Pool) =====
    event Swap(
        address indexed sender,
        address indexed recipient,
        int256 amount0,
        int256 amount1,
        uint160 sqrtPriceX96,
        uint128 liquidity,
        int24 tick
    );

    event Mint(
        address sender,
        address indexed owner,
        int24 indexed tickLower,
        int24 indexed tickUpper,
        uint128 amount,
        uint256 amount0,
        uint256 amount1
    );

    event Burn(
        address indexed owner,
        int24 indexed tickLower,
        int24 indexed tickUpper,
        uint128 amount,
        uint256 amount0,
        uint256 amount1
    );

    event Collect(
        address indexed owner,
        address recipient,
        int24 indexed tickLower,
        int24 indexed tickUpper,
        uint128 amount0,
        uint128 amount1
    );

    event CollectProtocol(
        address indexed sender,
        address indexed recipient,
        uint128 amount0,
        uint128 amount1
    );

    // ===== Uniswap V4 (PoolManager) =====
    event Swap(
        bytes32 indexed id,
        address indexed sender,
        int128 amount0,
        int128 amount1,
        uint160 sqrtPriceX96,
        uint128 liquidity,
        int24 tick,
        uint24 fee
    );

    event ModifyLiquidity(
        bytes32 indexed id,
        address indexed sender,
        int24 tickLower,
        int24 tickUpper,
        int256 liquidityDelta,
        bytes32 salt
    );

    event Donate(bytes32 indexed id, address indexed sender, uint256 amount0, uint256 amount1);

    event ProtocolFeeUpdated(bytes32 indexed id, uint16 protocolFee);

    // V4 also emits ERC20-style Transfer / Approval via the PoolManager's ERC6909.
}

pub async fn price_listener(db: Db, provider: impl Provider) -> Result<(), ServerError> {
    uniswap_v1_listener(db.clone(), &provider).await
}

/// Listens for all Uniswap V1 events via a block listener, computes the token
/// price from the vault reserves, and stores it in sled keyed by `TokenPairId`.
///
/// Price is computed from the exchange's ETH and token reserves:
///     price = eth_reserve / token_reserve
///
/// Under the V1 model the constant-product invariant means:
///     k = eth_reserve * token_reserve
/// so after any buy/sell/mint/burn the new reserve can be recomputed.
async fn uniswap_v1_listener(db: Db, provider: &impl Provider) -> Result<(), ServerError> {
    // Filter for all V1 event signatures across every exchange contract.
    let filter = Filter::new().events(&[
        TokenPurchase::SIGNATURE,
        EthPurchase::SIGNATURE,
        AddLiquidity::SIGNATURE,
        RemoveLiquidity::SIGNATURE,
    ]);

    // Subscribe to the block/pending stream through the provider.
    let mut stream = provider
        .watch_logs(&filter)
        .await?
        .into_stream()
        .flat_map(futures::stream::iter);
    let mut reserves: HashMap<Address, (f64, f64)> = HashMap::new();

    while let Some(log) = stream.next().await {
        let address = log.address();
        let (mut eth_reserve, mut token_reserve) =
            reserves.get(&address).copied().unwrap_or((0.0, 0.0));

        // V1 events are emitted by the exchange with the same topic layout.
        if let Ok(ev) = TokenPurchase::decode_log(&log.inner) {
            let eth_sold = u256_to_f64(ev.eth_sold);
            let tokens_bought = u256_to_f64(ev.tokens_bought);
            eth_reserve += eth_sold;
            token_reserve -= tokens_bought;
        } else if let Ok(ev) = EthPurchase::decode_log(&log.inner) {
            let tokens_sold = u256_to_f64(ev.tokens_sold);
            let eth_bought = u256_to_f64(ev.eth_bought);
            token_reserve += tokens_sold;
            eth_reserve -= eth_bought;
        } else if let Ok(ev) = AddLiquidity::decode_log(&log.inner) {
            eth_reserve += u256_to_f64(ev.eth_amount);
            token_reserve += u256_to_f64(ev.token_amount);
        } else if let Ok(ev) = RemoveLiquidity::decode_log(&log.inner) {
            eth_reserve -= u256_to_f64(ev.eth_amount);
            token_reserve -= u256_to_f64(ev.token_amount);
        } else {
            continue;
        }

        reserves.insert(address, (eth_reserve, token_reserve));

        // Compute the token price in ETH.
        let price = if token_reserve > 0.0 {
            eth_reserve / token_reserve
        } else {
            0.0
        };

        let id = TokenPairId {
            version: UniswapVersion::V1,
            pair_address: address,
            timestamp: log.block_timestamp.unwrap_or_else(|| {
                println!(
                    "skipping tick: block {} has no timestamp",
                    log.block_number.unwrap_or_default()
                );
                Utc::now().timestamp() as u64
            }),
        };

        if db.get(&id)?.is_none() {
            let value = price.to_le_bytes();
            db.insert(id, value)?;
        }

        println!("Stored V1 price for {address:?}: {price}");
    }

    Ok(())
}

/// Helper to convert a U256 value into an f64 (approximate but fine for pricing).
fn u256_to_f64(v: alloy::primitives::U256) -> f64 {
    v.to_string().parse::<f64>().unwrap_or(0.0)
}
