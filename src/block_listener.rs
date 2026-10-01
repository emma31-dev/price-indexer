use crate::error::ServerError;
use crate::types::*;
use alloy::primitives::Address;
use alloy::providers::Provider;
use alloy::rpc::types::Filter;
use alloy::sol;
use alloy::sol_types::SolEvent;
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

/// V1 listener: fetches `NewExchange` logs for the V1 factory at `block_number`
/// and decodes them into `PairEvent`.
pub async fn v1_listener(provider: &impl Provider, sled_db: Db) -> Result<(), ServerError> {
    let factory: Address = std::env::var("UNISWAP_V1_FACTORY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Address::ZERO);
    let filter = Filter::new()
        .address(factory)
        .event_signature(NewExchangeV1::SIGNATURE_HASH);

    let logs = provider.get_logs(&filter).await?;

    for log in logs {
        let ev = NewExchangeV1::decode_log(&log.inner)?;
        let data = NewExchangeV1Data {
            token: ev.token,
            exchange: ev.address,
        };
        let id = data.pair_id();
        if sled_db.contains_key(id).unwrap() {
            continue;
        }
        let token0 = resolve_token(provider, data.token).await;
        let resolved = ResolvedPairEvent {
            event: PairEvent::V1(data),
            token0,
            token1: None,
        };
        sled_db.insert(id, resolved).unwrap();
    }

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
            sled_db.insert(id, resolved).unwrap();
        }
    }

    Ok(())
}

/// V2 listener: fetches `PairCreated` logs for the V2 factory at `block_number`
/// and decodes them into `PairEvent`.
pub async fn v2_listener(provider: &impl Provider, sled_db: Db) -> Result<(), ServerError> {
    let factory: Address = std::env::var("UNISWAP_V2_FACTORY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Address::ZERO);
    let filter = Filter::new()
        .address(factory)
        .event_signature(PairCreatedV2::SIGNATURE_HASH);

    let logs = provider.get_logs(&filter).await?;

    for log in logs {
        let ev = PairCreatedV2::decode_log(&log.inner)?;
        let data = PairCreatedV2Data {
            token0: ev.token0,
            token1: ev.token1,
            pair: ev.pair,
            all_pairs_length: ev.allPairsLength,
        };
        let id = data.pair_id();
        if sled_db.contains_key(id).unwrap() {
            continue;
        }
        let token0 = resolve_token(provider, data.token0).await;
        let token1 = resolve_token(provider, data.token1).await;
        let resolved = ResolvedPairEvent {
            event: PairEvent::V2(data),
            token0,
            token1,
        };
        sled_db.insert(id, resolved).unwrap();
    }

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
            sled_db.insert(id, resolved).unwrap();
        }
    }

    Ok(())
}

/// V3 listener: fetches `PoolCreated` logs for the V3 factory at `block_number`
/// and decodes them into `PairEvent`.
pub async fn v3_listener(provider: &impl Provider, sled_db: Db) -> Result<(), ServerError> {
    let factory: Address = std::env::var("UNISWAP_V3_FACTORY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Address::ZERO);
    let filter = Filter::new()
        .address(factory)
        .event_signature(PoolCreatedV3::SIGNATURE_HASH);

    let logs = provider.get_logs(&filter).await?;

    for log in logs {
        let ev = PoolCreatedV3::decode_log(&log.inner)?;
        let data = PoolCreatedV3Data {
            token0: ev.token0,
            token1: ev.token1,
            fee: ev.fee.to(),
            tick_spacing: ev.tickSpacing.low_i32(),
            pool: ev.pool,
        };
        let id = data.pair_id();
        if sled_db.contains_key(id).unwrap() {
            continue;
        }
        let token0 = resolve_token(provider, data.token0).await;
        let token1 = resolve_token(provider, data.token1).await;
        let resolved = ResolvedPairEvent {
            event: PairEvent::V3(data),
            token0,
            token1,
        };
        sled_db.insert(id, resolved).unwrap();
    }

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
            sled_db.insert(id, resolved).unwrap();
        }
    }

    Ok(())
}

/// V4 listener: fetches `PoolCreated` logs for the V4 factory at `block_number`
/// and decodes them into `PairEvent`.
pub async fn v4_listener(provider: &impl Provider, sled_db: Db) -> Result<(), ServerError> {
    let factory: Address = std::env::var("UNISWAP_V4_FACTORY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Address::ZERO);
    let filter = Filter::new()
        .address(factory)
        .event_signature(PoolCreatedV4::SIGNATURE_HASH);

    let logs = provider.get_logs(&filter).await?;

    for log in logs {
        let ev = PoolCreatedV4::decode_log(&log.inner)?;
        let data = PoolCreatedV4Data {
            token0: ev.token0,
            token1: ev.token1,
            fee: ev.fee.to(),
            tick_spacing: ev.tickSpacing.low_i32(),
            hooks: ev.hooks,
            pool_id: ev.poolId.0,
        };
        let id = data.pair_id();
        if sled_db.contains_key(id).unwrap() {
            continue;
        }
        let token0 = resolve_token(provider, data.token0).await;
        let token1 = resolve_token(provider, data.token1).await;
        let resolved = ResolvedPairEvent {
            event: PairEvent::V4(data),
            token0,
            token1,
        };
        sled_db.insert(id, resolved).unwrap();
    }

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
            sled_db.insert(id, resolved).unwrap();
        }
    }

    Ok(())
}
