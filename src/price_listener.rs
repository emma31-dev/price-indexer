use std::collections::HashMap;

use crate::error::ServerError;
use crate::types::*;
use alloy::{
    primitives::Address, providers::Provider, rpc::types::Filter, sol, sol_types::SolEvent,
};
use chrono::Utc;
use futures::StreamExt;
use sled::Db;

pub async fn price_listener(db: Db, provider: &impl Provider) -> Result<(), ServerError> {
    futures::try_join!(
        uniswap_v1_listener(db.clone(), provider),
        uniswap_v2_listener(db.clone(), provider),
        uniswap_v3_listener(db.clone(), provider),
        uniswap_v4_listener(db.clone(), provider),
    )?;
    Ok(())
}

/// Looks up the most recent `TickMeta` stored for a given pair, so that
/// listeners can carry the previous price forward when an event only changes
/// volume (or vice versa) instead of overwriting it with a zero.
fn last_tick(db: &Db, id: &TokenPairId) -> Result<Option<TickMeta>, ServerError> {
    let start = TokenPairId {
        pair_address: id.pair_address.clone(),
        version: id.version.clone(),
        timestamp: id.timestamp.clone().saturating_add(1),
    };
    let end = TokenPairId {
        pair_address: id.pair_address.clone(),
        version: id.version.clone(),
        timestamp: id.timestamp.clone().saturating_sub(3_600),
    };

    let range = start..end;
    // `id` is the leading key, so a range over just that pair walks its ticks.
    // `next_back` gives us the latest (highest) key, which is the newest tick.
    let Some(entry) = db.range(range).next_back() else {
        return Ok(None);
    };
    let (_key, value) = entry?;
    let meta = rkyv::from_bytes::<TickMeta, rkyv::rancor::Error>(&value)?;
    Ok(Some(meta))
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
    }

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
        // Every V1 event changes either price (via reserves) or volume, but a
        // reserve change can leave the price untouched and vice versa, so we
        // track whether each was actually refreshed.
        let (volume, price_changed) = if let Ok(ev) = TokenPurchase::decode_log(&log.inner) {
            let eth_sold = u256_to_f64(ev.eth_sold);
            let tokens_bought = u256_to_f64(ev.tokens_bought);
            eth_reserve += eth_sold;
            token_reserve -= tokens_bought;
            (eth_sold, true)
        } else if let Ok(ev) = EthPurchase::decode_log(&log.inner) {
            let tokens_sold = u256_to_f64(ev.tokens_sold);
            let eth_bought = u256_to_f64(ev.eth_bought);
            token_reserve += tokens_sold;
            eth_reserve -= eth_bought;
            (eth_bought, true)
        } else if let Ok(ev) = AddLiquidity::decode_log(&log.inner) {
            let eth_amount = u256_to_f64(ev.eth_amount);
            eth_reserve += eth_amount;
            token_reserve += u256_to_f64(ev.token_amount);
            (eth_amount, true)
        } else if let Ok(ev) = RemoveLiquidity::decode_log(&log.inner) {
            let eth_amount = u256_to_f64(ev.eth_amount);
            eth_reserve -= eth_amount;
            token_reserve -= u256_to_f64(ev.token_amount);
            (eth_amount, true)
        } else {
            continue;
        };

        reserves.insert(address, (eth_reserve, token_reserve));

        // Compute the token price in ETH.
        let new_price = if token_reserve > 0.0 {
            eth_reserve / token_reserve
        } else {
            0.0
        };

        let id = TokenPairId {
            version: UniswapVersion::V1,
            pair_address: PairAddress::Address(address.into()),
            timestamp: log.block_timestamp.unwrap_or_else(|| {
                println!(
                    "skipping tick: block {} has no timestamp",
                    log.block_number.unwrap_or_default()
                );
                Utc::now().timestamp() as u64
            }),
        };

        // If this event only moved volume, it should not overwrite the last
        // known price with the (possibly zero) freshly computed one.
        let price = if price_changed {
            new_price
        } else {
            last_tick(&db, &id)?.map(|t| t.price).unwrap_or(new_price)
        };

        let tick = TickMeta {
            price,
            volume,
            block: log.block_number.unwrap_or_default(),
        };

        if db.get(&id)?.is_none() {
            db.insert(id, tick)?;
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
/// Swap/Mint/Burn events are also observed (they carry the amounts moved), and
/// their amounts are accumulated into the running volume that is recorded with
/// each tick, since volume does change. Because a Swap/Mint/Burn can change
/// volume without a matching `Sync` in the same iteration, and a `Sync` can
/// change price with no volume, each tick carries forward whichever value was
/// not touched by the current event.
async fn uniswap_v2_listener(db: Db, provider: &impl Provider) -> Result<(), ServerError> {
    sol! {
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
    }

    // Filter for all V2 pair event signatures across every pair contract.
    let filter = Filter::new().events(&[
        Swap::SIGNATURE,
        Sync::SIGNATURE,
        Mint::SIGNATURE,
        Burn::SIGNATURE,
    ]);

    let mut stream = provider
        .watch_logs(&filter)
        .await?
        .into_stream()
        .flat_map(futures::stream::iter);
    let mut reserves: HashMap<Address, (f64, f64)> = HashMap::new();
    // Running volume per pair, updated from Swap/Mint/Burn events.
    let mut volumes: HashMap<Address, f64> = HashMap::new();

    while let Some(log) = stream.next().await {
        let address = log.address();

        // Track the volume carried by the amount-bearing events. These do not
        // change the reserves directly, but they do change the volume.
        let mut volume_changed = false;
        if let Ok(ev) = Swap::decode_log(&log.inner) {
            let swap_volume = u256_to_f64(ev.amount0In)
                + u256_to_f64(ev.amount1In)
                + u256_to_f64(ev.amount0Out)
                + u256_to_f64(ev.amount1Out);
            *volumes.entry(address).or_insert(0.0) += swap_volume;
            volume_changed = true;
        } else if let Ok(ev) = Mint::decode_log(&log.inner) {
            let mint_volume = u256_to_f64(ev.amount0) + u256_to_f64(ev.amount1);
            *volumes.entry(address).or_insert(0.0) += mint_volume;
            volume_changed = true;
        } else if let Ok(ev) = Burn::decode_log(&log.inner) {
            let burn_volume = u256_to_f64(ev.amount0) + u256_to_f64(ev.amount1);
            *volumes.entry(address).or_insert(0.0) += burn_volume;
            volume_changed = true;
        }

        // The authoritative reserve snapshot always comes from `Sync`.
        let mut price_changed = false;
        if let Ok(ev) = Sync::decode_log(&log.inner) {
            let reserve0 = u112_to_f64(ev.reserve0);
            let reserve1 = u112_to_f64(ev.reserve1);
            reserves.insert(address, (reserve0, reserve1));
            price_changed = true;
        } else if !volume_changed && !price_changed {
            // Neither price nor volume was touched, nothing to store.
            continue;
        }

        let Some((reserve0, reserve1)) = reserves.get(&address).copied() else {
            continue;
        };

        // Compute the token price (token1 per token0, denominated in token1).
        let new_price = if reserve0 > 0.0 {
            reserve1 / reserve0
        } else {
            0.0
        };

        // Take the accumulated volume for this pair and reset it for the next
        // tick, since volume changes across swaps.
        let volume = volumes.get(&address).copied().unwrap_or(0.0);
        volumes.insert(address, 0.0);

        let id = TokenPairId {
            version: UniswapVersion::V2,
            pair_address: PairAddress::Address(address.into()),
            timestamp: log.block_timestamp.unwrap_or_else(|| {
                println!(
                    "skipping tick: block {} has no timestamp",
                    log.block_number.unwrap_or_default()
                );
                Utc::now().timestamp() as u64
            }),
        };

        // A volume-only event (no `Sync`) must carry the last known price
        // forward instead of writing a stale/zero one.
        let price = if price_changed {
            new_price
        } else {
            last_tick(&db, &id)?.map(|t| t.price).unwrap_or(new_price)
        };

        let tick = TickMeta {
            price,
            volume,
            block: log.block_number.unwrap_or_default(),
        };

        if db.get(&id)?.is_none() {
            db.insert(id, tick)?;
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
    sol! {
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
    }

    // Filter for all V3 pool event signatures across every pool contract.
    let filter = Filter::new().events(&[
        Swap::SIGNATURE,
        Mint::SIGNATURE,
        Burn::SIGNATURE,
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

        // `Swap` carries both a fresh `sqrtPriceX96` (price) and the amounts
        // moved (volume), so it updates both. Liquidity events (Mint/Burn/
        // Collect/CollectProtocol) change volume only, so they must carry the
        // last known price forward rather than zeroing it out.
        let (price_changed, new_price, volume) = if let Ok(ev) = Swap::decode_log(&log.inner) {
            let sqrt_price_x96 = u160_to_f64(ev.sqrtPriceX96);
            let q96 = 2f64.powi(96);

            // (sqrtPriceX96 / 2^96)^2 == sqrtPriceX96^2 / 2^192
            let price = if sqrt_price_x96 > 0.0 {
                (sqrt_price_x96 / q96).powi(2)
            } else {
                0.0
            };

            // The absolute amounts moved by the swap give us the volume.
            let volume =
                u256_to_f64(ev.amount0.unsigned_abs()) + u256_to_f64(ev.amount1.unsigned_abs());

            (true, price, volume)
        } else if let Ok(ev) = Mint::decode_log(&log.inner) {
            (
                false,
                0.0,
                u256_to_f64(ev.amount0) + u256_to_f64(ev.amount1),
            )
        } else if let Ok(ev) = Burn::decode_log(&log.inner) {
            (
                false,
                0.0,
                u256_to_f64(ev.amount0) + u256_to_f64(ev.amount1),
            )
        } else if let Ok(ev) = Collect::decode_log(&log.inner) {
            (
                false,
                0.0,
                u128_to_f64(ev.amount0) + u128_to_f64(ev.amount1),
            )
        } else if let Ok(ev) = CollectProtocol::decode_log(&log.inner) {
            (
                false,
                0.0,
                u128_to_f64(ev.amount0) + u128_to_f64(ev.amount1),
            )
        } else {
            continue;
        };

        let id = TokenPairId {
            version: UniswapVersion::V3,
            pair_address: PairAddress::Address(address.into()),
            timestamp: log.block_timestamp.unwrap_or_else(|| {
                println!(
                    "skipping tick: block {} has no timestamp",
                    log.block_number.unwrap_or_default()
                );
                Utc::now().timestamp() as u64
            }),
        };

        // Only overwrite the price when this event actually produced a new one;
        // otherwise reuse the last stored price for this pool.
        let price = if price_changed {
            new_price
        } else {
            last_tick(&db, &id)?.map(|t| t.price).unwrap_or(new_price)
        };

        let tick = TickMeta {
            price,
            volume,
            block: log.block_number.unwrap_or_default(),
        };

        if db.get(&id)?.is_none() {
            db.insert(id, tick)?;
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
    sol! {
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
    }

    // Filter for all V4 pool-manager event signatures. All pools share the
    // single PoolManager contract, so the address is not per-pool.
    let filter = Filter::new().events(&[
        Swap::SIGNATURE,
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
        // `Swap` carries both a fresh `sqrtPriceX96` (price) and the amounts
        // moved (volume), so it updates both. `Donate` moves amounts without a
        // price change, so it must carry the last known price forward. The
        // remaining events carry neither, so they are ignored.
        let (pool_id, price_changed, new_price, volume) =
            if let Ok(ev) = Swap::decode_log(&log.inner) {
                let sqrt_price_x96 = u160_to_f64(ev.sqrtPriceX96);
                let q96 = 2f64.powi(96);

                let price = if sqrt_price_x96 > 0.0 {
                    (sqrt_price_x96 / q96).powi(2)
                } else {
                    0.0
                };

                // The absolute amounts moved by the swap give us the volume.
                let volume = i128_to_f64(ev.amount0.abs()) + i128_to_f64(ev.amount1.abs());

                (ev.id, true, price, volume)
            } else if let Ok(ev) = Donate::decode_log(&log.inner) {
                (
                    ev.id,
                    false,
                    0.0,
                    u256_to_f64(ev.amount0) + u256_to_f64(ev.amount1),
                )
            } else {
                continue;
            };

        let id = TokenPairId {
            version: UniswapVersion::V4,
            pair_address: PairAddress::PoolId(pool_id.into()),
            timestamp: log.block_timestamp.unwrap_or_else(|| {
                println!(
                    "skipping tick: block {} has no timestamp",
                    log.block_number.unwrap_or_default()
                );
                Utc::now().timestamp() as u64
            }),
        };

        // Only overwrite the price when this event actually produced a new one;
        // otherwise reuse the last stored price for this pool.
        let price = if price_changed {
            new_price
        } else {
            last_tick(&db, &id)?.map(|t| t.price).unwrap_or(new_price)
        };

        let tick = TickMeta {
            price,
            volume,
            block: log.block_number.unwrap_or_default(),
        };

        if db.get(&id)?.is_none() {
            db.insert(id, tick)?;
        }

        let pair_address = log.address();
        println!("Stored V4 price for {pair_address:?}: {price}");
    }

    Ok(())
}

/// Helper to convert a U256 value into an f64 (approximate but fine for pricing).
fn u256_to_f64(v: alloy::primitives::U256) -> f64 {
    v.to_string().parse::<f64>().unwrap_or(0.0)
}

/// Helper to convert a U128 value into an f64.
fn u128_to_f64(v: u128) -> f64 {
    v as f64
}

/// Helper to convert a U112 value into an f64.
fn u112_to_f64(v: alloy::primitives::Uint<112, 2>) -> f64 {
    v.to_string().parse::<f64>().unwrap_or(0.0)
}

/// Helper to convert a U160 value into an f64.
fn u160_to_f64(v: alloy::primitives::Uint<160, 3>) -> f64 {
    v.to_string().parse::<f64>().unwrap_or(0.0)
}

/// Helper to convert an i128 value into an f64.
fn i128_to_f64(v: i128) -> f64 {
    v as f64
}
