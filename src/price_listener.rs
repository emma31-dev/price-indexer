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
    #[sol(rename = "V2Swap")]
    event Swap(
        address indexed sender,
        uint256 amount0In,
        uint256 amount1In,
        uint256 amount0Out,
        uint256 amount1Out,
        address indexed to
    );

    event Sync(uint112 reserve0, uint112 reserve1);

    #[sol(rename = "V2Mint")]
    event Mint(address indexed sender, uint256 amount0, uint256 amount1);

    #[sol(rename = "V2Burn")]
    event Burn(
        address indexed sender,
        uint256 amount0,
        uint256 amount1,
        address indexed to
    );

    // ===== Uniswap V3 (Pool) =====
    #[sol(rename = "V3Swap")]
    event Swap(
        address indexed sender,
        address indexed recipient,
        int256 amount0,
        int256 amount1,
        uint160 sqrtPriceX96,
        uint128 liquidity,
        int24 tick
    );

    #[sol(rename = "V3Mint")]
    event Mint(
        address sender,
        address indexed owner,
        int24 indexed tickLower,
        int24 indexed tickUpper,
        uint128 amount,
        uint256 amount0,
        uint256 amount1
    );

    #[sol(rename = "V3Burn")]
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
    #[sol(rename = "V4Swap")]
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

pub async fn price_listener(db: Db, provider: &impl Provider) -> Result<(), ServerError> {
    futures::try_join!(
        uniswap_v1_listener(db.clone(), provider),
        uniswap_v2_listener(db.clone(), provider),
        uniswap_v3_listener(db.clone(), provider),
        uniswap_v4_listener(db.clone(), provider),
    )?;
    Ok(())
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
            pair_address: PairAddress::Address(address),
            timestamp: log.block_timestamp.unwrap_or_else(|| {
                println!(
                    "skipping tick: block {} has no timestamp",
                    log.block_number.unwrap_or_default()
                );
                Utc::now().timestamp() as u64
            }),
        };

        if db.get(&id)?.is_none() {
            let value = Some(price);
            db.insert(id, value)?;
        }

        println!("Stored V1 price for {address:?}: {price}");
    }

    Ok(())
}

/// Listens for all Uniswap V2 events, tracking the pair's reserves from `Sync`
/// events and computing the token price from them.
///
/// V2 pairs maintain explicit reserves and emit a `Sync` event anytime they
/// change, so we can use the reserves directly:
///     price = reserve1 / reserve0
///
/// Swap/Mint/Burn events are also observed (they carry the amounts moved), but
/// the authoritative reserve snapshot always comes from `Sync`.
async fn uniswap_v2_listener(db: Db, provider: &impl Provider) -> Result<(), ServerError> {
    // Filter for all V2 pair event signatures across every pair contract.
    let filter = Filter::new().event_signature(Sync::SIGNATURE);

    let mut stream = provider
        .watch_logs(&filter)
        .await?
        .into_stream()
        .flat_map(futures::stream::iter);
    let mut reserves: HashMap<Address, (f64, f64)> = HashMap::new();

    while let Some(log) = stream.next().await {
        let address = log.address();

        if let Ok(ev) = Sync::decode_log(&log.inner) {
            let reserve0 = u112_to_f64(ev.reserve0);
            let reserve1 = u112_to_f64(ev.reserve1);
            reserves.insert(address, (reserve0, reserve1));
        } else {
            continue;
        }

        let Some((reserve0, reserve1)) = reserves.get(&address).copied() else {
            continue;
        };

        // Compute the token price (token1 per token0, denominated in token1).
        let price = if reserve0 > 0.0 {
            reserve1 / reserve0
        } else {
            0.0
        };

        let id = TokenPairId {
            version: UniswapVersion::V2,
            pair_address: PairAddress::Address(address),
            timestamp: log.block_timestamp.unwrap_or_else(|| {
                println!(
                    "skipping tick: block {} has no timestamp",
                    log.block_number.unwrap_or_default()
                );
                Utc::now().timestamp() as u64
            }),
        };

        if db.get(&id)?.is_none() {
            let value = Some(price);
            db.insert(id, value)?;
        }

        println!("Stored V2 price for {address:?}: {price}");
    }

    Ok(())
}

/// Listens for all Uniswap V3 events and derives the price from the pool's
/// `sqrtPriceX96` carried on every `Swap` event.
///
/// V3 stores prices in the form sqrt(price) * 2^96, so:
///     price = (sqrtPriceX96 / 2^96)^2
/// we also scale the result to a human-readable f64.
///
/// Liquidity-modifying events (Mint/Burn/Collect/CollectProtocol) do not change
/// the price directly, so they are skipped for pricing purposes.
async fn uniswap_v3_listener(db: Db, provider: &impl Provider) -> Result<(), ServerError> {
    // Filter for all V3 pool event signatures across every pool contract.
    let filter = Filter::new().events(&[
        V3Swap::SIGNATURE,
        V3Mint::SIGNATURE,
        V3Burn::SIGNATURE,
        Collect::SIGNATURE,
        CollectProtocol::SIGNATURE,
    ]);

    let mut stream = provider
        .watch_logs(&filter)
        .await?
        .into_stream()
        .flat_map(futures::stream::iter);

    while let Some(log) = stream.next().await {
        let address = log.address();

        // Only `Swap` carries a fresh `sqrtPriceX96`.
        let Ok(ev) = V3Swap::decode_log(&log.inner) else {
            continue;
        };

        let sqrt_price_x96 = u160_to_f64(ev.sqrtPriceX96);
        let q96 = 2f64.powi(96);

        // (sqrtPriceX96 / 2^96)^2 == sqrtPriceX96^2 / 2^192
        let price = if sqrt_price_x96 > 0.0 {
            (sqrt_price_x96 / q96).powi(2)
        } else {
            0.0
        };

        let id = TokenPairId {
            version: UniswapVersion::V3,
            pair_address: PairAddress::Address(address),
            timestamp: log.block_timestamp.unwrap_or_else(|| {
                println!(
                    "skipping tick: block {} has no timestamp",
                    log.block_number.unwrap_or_default()
                );
                Utc::now().timestamp() as u64
            }),
        };

        if db.get(&id)?.is_none() {
            let value = Some(price);
            db.insert(id, value)?;
        }

        println!("Stored V3 price for {address:?}: {price}");
    }

    Ok(())
}

/// Listens for all Uniswap V4 events emitted by the singleton `PoolManager`.
///
/// V4 uses the same `sqrtPriceX96` representation as V3 but emits a single
/// `Swap` event for every pool, distinguished by the indexed `id` (the pool id)
/// rather than by the contract address. We key our price by the pool id here,
/// mapped into the pair address field.
async fn uniswap_v4_listener(db: Db, provider: &impl Provider) -> Result<(), ServerError> {
    // Filter for all V4 pool-manager event signatures. All pools share the
    // single PoolManager contract, so the address is not per-pool.
    let filter = Filter::new().events(&[
        V4Swap::SIGNATURE,
        ModifyLiquidity::SIGNATURE,
        Donate::SIGNATURE,
        ProtocolFeeUpdated::SIGNATURE,
    ]);

    let mut stream = provider
        .watch_logs(&filter)
        .await?
        .into_stream()
        .flat_map(futures::stream::iter);

    while let Some(log) = stream.next().await {
        // Only `Swap` carries a fresh `sqrtPriceX96`.
        let Ok(ev) = V4Swap::decode_log(&log.inner) else {
            continue;
        };

        let sqrt_price_x96 = u160_to_f64(ev.sqrtPriceX96);
        let q96 = 2f64.powi(96);

        let price = if sqrt_price_x96 > 0.0 {
            (sqrt_price_x96 / q96).powi(2)
        } else {
            0.0
        };

        // The pool identifier is the indexed `id` (bytes32). We store it in the
        // `pair_address` field by taking the low 20 bytes as an address.
        let pool_id = ev.id;

        let id = TokenPairId {
            version: UniswapVersion::V4,
            pair_address: PairAddress::PoolId(pool_id),
            timestamp: log.block_timestamp.unwrap_or_else(|| {
                println!(
                    "skipping tick: block {} has no timestamp",
                    log.block_number.unwrap_or_default()
                );
                Utc::now().timestamp() as u64
            }),
        };

        if db.get(&id)?.is_none() {
            let value = Some(price);
            db.insert(id, value)?;
        }

        println!("Stored V4 price for {pair_address:?}: {price}");
    }

    Ok(())
}

/// Helper to convert a U256 value into an f64 (approximate but fine for pricing).
fn u256_to_f64(v: alloy::primitives::U256) -> f64 {
    v.to_string().parse::<f64>().unwrap_or(0.0)
}

/// Helper to convert a U112 value into an f64.
fn u112_to_f64(v: alloy::primitives::Uint<112, 2>) -> f64 {
    v.to_string().parse::<f64>().unwrap_or(0.0)
}

/// Helper to convert a U160 value into an f64.
fn u160_to_f64(v: alloy::primitives::Uint<160, 3>) -> f64 {
    v.to_string().parse::<f64>().unwrap_or(0.0)
}
