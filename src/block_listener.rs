use crate::error::ServerError;
use crate::types::*;
use alloy::primitives::Address;
use alloy::providers::Provider;
use alloy::rpc::types::Filter;
use alloy::sol;
use alloy::sol_types::SolEvent;
use chrono::Utc;
use futures::StreamExt;
use sled::Db;

// ----- Minimal ABI/event definitions for Uniswap factory contracts -----
sol! {
    // Uniswap V1 factory: `NewExchange(address,address)`
    #[derive(Debug)]
    event NewExchangeV1(address indexed token, address indexed exchange);

    // Uniswap V2 factory: `PairCreated(address,address,address,uint256)`
    #[derive(Debug)]
    event PairCreatedV2(
        address indexed token0,
        address indexed token1,
        address pair,
        uint256 allPairsLength
    );

    // Uniswap V3 factory: `PoolCreated(address,address,uint24,int24,address)`
    #[derive(Debug)]
    event PoolCreatedV3(
        address indexed token0,
        address indexed token1,
        uint24 indexed fee,
        int24 tickSpacing,
        address pool
    );

    // Uniswap V4 factory: `PoolCreated(PoolId,address,address,uint24,int24,address)`
    event PoolCreatedV4(
        address indexed token0,
        address indexed token1,
        uint24 indexed fee,
        int24 tickSpacing,
        address hooks,
        bytes32 poolId
    );

    // Minimal ERC20 for resolving decimals.
    #[derive(Debug)]
    #[sol(rpc)]
    interface IERC20 {
        function decimals() external view returns (uint8);
        function name() external view returns (string);
        function symbol() external view returns (string);
    }
}

/// Resolves a token's decimals by calling `decimals()` on the token contract.
/// Returns `None` if the call fails.
async fn resolve_token(provider: &impl Provider, address: Address) -> Option<Token> {
    if address == Address::ZERO {
        return Some(Token {
            address: Address::ZERO,
            decimals: 18,
            name: "Etheruem".into(),
            symbol: "ETH".into(),
        });
    }
    let contract = IERC20::new(address, &provider);
    let decimals = contract.decimals().call().await.ok()?;
    let name = contract.name().call().await.ok()?;
    let symbol = contract.symbol().call().await.ok()?;
    Some(Token {
        address,
        decimals,
        name,
        symbol,
    })
}

/// V1 listener: watches `NewExchange` logs for the V1 factory
/// and decodes them into `PairEvent`.
pub async fn v1_listener(provider: &impl Provider, sled_db: Db) -> Result<(), ServerError> {
    let factory: Address = std::env::var("UNISWAP_V1_FACTORY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Address::ZERO);
    let filter = Filter::new()
        .address(factory)
        .event_signature(NewExchangeV1::SIGNATURE_HASH);

    let mut stream = provider
        .watch_logs(&filter)
        .await?
        .into_stream()
        .flat_map(futures::stream::iter);

    while let Some(log) = stream.next().await {
        if let Ok(data) = NewExchangeV1::decode_log(&log.inner) {
            let event = NewExchangeV1Data {
                token: data.token,
                exchange: data.address,
            };
            let id = event.pair_id();
            let token0 = resolve_token(provider, event.token).await;
            let resolved = ResolvedPairEvent {
                event: PairEvent::V1(event),
                token0,
                token1: None,
            };
            sled_db.insert(id, resolved)?;

            // Store the pool price at this moment as 0.0.
            let id = TokenPairId {
                version: UniswapVersion::V1,
                pair_address: PairAddress::Address(data.address),
                timestamp: log.block_timestamp.unwrap_or_else(|| {
                    println!(
                        "skipping tick: block {} has no timestamp",
                        log.block_number.unwrap_or_default()
                    );
                    Utc::now().timestamp() as u64
                }),
            };

            let value: Option<f64> = None;
            sled_db.insert(id, value)?;
        }
    }

    Ok(())
}

/// V2 listener: watches `PairCreated` logs for the V2 factory
/// and decodes them into `PairEvent`.
pub async fn v2_listener(provider: &impl Provider, sled_db: Db) -> Result<(), ServerError> {
    let factory: Address = std::env::var("UNISWAP_V2_FACTORY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Address::ZERO);
    let filter = Filter::new()
        .address(factory)
        .event_signature(PairCreatedV2::SIGNATURE_HASH);

    let mut stream = provider
        .watch_logs(&filter)
        .await?
        .into_stream()
        .flat_map(futures::stream::iter);
    while let Some(log) = stream.next().await {
        if let Ok(data) = PairCreatedV2::decode_log(&log.inner) {
            let event = PairCreatedV2Data {
                token0: data.token0,
                token1: data.token1,
                pair: data.pair,
                all_pairs_length: data.allPairsLength,
            };
            let id = event.pair_id();
            let token0 = resolve_token(provider, event.token0).await;
            let token1 = resolve_token(provider, event.token1).await;
            let resolved = ResolvedPairEvent {
                event: PairEvent::V2(event),
                token0,
                token1,
            };
            sled_db.insert(id, resolved)?;

            // Store the pool price at this moment as 0.0.
            let id = TokenPairId {
                version: UniswapVersion::V2,
                pair_address: PairAddress::Address(data.pair),
                timestamp: log.block_timestamp.unwrap_or_else(|| {
                    println!(
                        "skipping tick: block {} has no timestamp",
                        log.block_number.unwrap_or_default()
                    );
                    Utc::now().timestamp() as u64
                }),
            };

            let value: Option<f64> = None;
            sled_db.insert(id, value)?;
        }
    }

    Ok(())
}

/// V3 listener: watches `PoolCreated` logs for the V3 factory
/// and decodes them into `PairEvent`.
pub async fn v3_listener(provider: &impl Provider, sled_db: Db) -> Result<(), ServerError> {
    let factory: Address = std::env::var("UNISWAP_V3_FACTORY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Address::ZERO);
    let filter = Filter::new()
        .address(factory)
        .event_signature(PoolCreatedV3::SIGNATURE_HASH);

    let mut stream = provider
        .watch_logs(&filter)
        .await?
        .into_stream()
        .flat_map(futures::stream::iter);
    while let Some(log) = stream.next().await {
        if let Ok(data) = PoolCreatedV3::decode_log(&log.inner) {
            let event = PoolCreatedV3Data {
                token0: data.token0,
                token1: data.token1,
                fee: data.fee.to(),
                tick_spacing: data.tickSpacing.low_i32(),
                pool: data.pool,
            };
            let id = event.pair_id();
            let token0 = resolve_token(provider, event.token0).await;
            let token1 = resolve_token(provider, event.token1).await;
            let resolved = ResolvedPairEvent {
                event: PairEvent::V3(event),
                token0,
                token1,
            };
            sled_db.insert(id, resolved)?;

            // Store the pool price at this moment as 0.0.
            let id = TokenPairId {
                version: UniswapVersion::V3,
                pair_address: PairAddress::Address(data.pool),
                timestamp: log.block_timestamp.unwrap_or_else(|| {
                    println!(
                        "skipping tick: block {} has no timestamp",
                        log.block_number.unwrap_or_default()
                    );
                    Utc::now().timestamp() as u64
                }),
            };

            let value: Option<f64> = None;
            sled_db.insert(id, value)?;
        }
    }

    Ok(())
}

/// V4 listener: watches `PoolCreated` logs for the V4 factory
/// and decodes them into `PairEvent`.
pub async fn v4_listener(provider: &impl Provider, sled_db: Db) -> Result<(), ServerError> {
    let factory: Address = std::env::var("UNISWAP_V4_FACTORY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Address::ZERO);
    let filter = Filter::new()
        .address(factory)
        .event_signature(PoolCreatedV4::SIGNATURE_HASH);

    let mut stream = provider
        .watch_logs(&filter)
        .await?
        .into_stream()
        .flat_map(futures::stream::iter);
    while let Some(log) = stream.next().await {
        if let Ok(data) = PoolCreatedV4::decode_log(&log.inner) {
            let event = PoolCreatedV4Data {
                token0: data.token0,
                token1: data.token1,
                fee: data.fee.to(),
                tick_spacing: data.tickSpacing.low_i32(),
                hooks: data.hooks,
                pool_id: data.poolId.0,
            };
            let id = event.pair_id();
            let token0 = resolve_token(provider, event.token0).await;
            let token1 = resolve_token(provider, event.token1).await;
            let resolved = ResolvedPairEvent {
                event: PairEvent::V4(event),
                token0,
                token1,
            };
            sled_db.insert(id, resolved)?;

            // Store the pool price at this moment as 0.0.
            let id = TokenPairId {
                version: UniswapVersion::V4,
                pair_address: PairAddress::PoolId(data.poolId.0),
                timestamp: log.block_timestamp.unwrap_or_else(|| {
                    println!(
                        "skipping tick: block {} has no timestamp",
                        log.block_number.unwrap_or_default()
                    );
                    Utc::now().timestamp() as u64
                }),
            };

            let value: Option<f64> = None;
            sled_db.insert(id, value)?;
        }
    }

    Ok(())
}
