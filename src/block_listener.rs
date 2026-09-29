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
    interface IERC20 {
        function decimals() external view returns (uint8);
    }
}

/// V1 listener: fetches `NewExchange` logs for the V1 factory at `block_number`
/// and decodes them into `PairData`.
pub async fn v1_listener(
    provider: &impl Provider,
    sled_db: Db,
) -> Result<(), Box<dyn std::error::Error>> {
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
        let id = PairId {
            version: UniswapVersion::V1,
            pair_address: ev.address,
        };
        if !sled_db.contains_key(id).unwrap() {
            continue;
        }
        let token0 = Token {
            address: ev.token,
            decimals: 18,
        };
        let token1 = Token {
            address: Address::ZERO,
            decimals: 18,
        };
        let data = PairData {
            pair_address: ev.address,
            version: UniswapVersion::V1,
            token0,
            token1,
        };
        sled_db.insert(id, data).unwrap();
    }

    let mut stream = provider
        .watch_logs(&filter)
        .await?
        .into_stream()
        .flat_map(futures::stream::iter);
    while let Some(log) = stream.next().await {
        if let Ok(data) = NewExchangeV1::decode_log(&log.inner) {
            let id = PairId {
                version: UniswapVersion::V1,
                pair_address: data.address,
            };
            let token0 = Token {
                address: data.token,
                decimals: 18,
            };
            let token1 = Token {
                address: Address::ZERO,
                decimals: 18,
            };
            let pair_data = PairData {
                pair_address: data.address,
                version: UniswapVersion::V1,
                token0,
                token1,
            };
            sled_db.insert(id, pair_data).unwrap();
        }
    }

    Ok(())
}

/// V2 listener: fetches `PairCreated` logs for the V2 factory at `block_number`
/// and decodes them into `PairData`.
pub async fn v2_listener(
    provider: &impl Provider,
    sled_db: Db,
) -> Result<(), Box<dyn std::error::Error>> {
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
        let id = PairId {
            version: UniswapVersion::V2,
            pair_address: ev.pair,
        };
        if !sled_db.contains_key(id).unwrap() {
            continue;
        }
        let token0 = Token {
            address: ev.token0,
            decimals: 18,
        };
        let token1 = Token {
            address: ev.token1,
            decimals: 18,
        };
        let data = PairData {
            pair_address: ev.pair,
            version: UniswapVersion::V2,
            token0,
            token1,
        };
        sled_db.insert(id, data).unwrap();
    }

    let mut stream = provider
        .watch_logs(&filter)
        .await?
        .into_stream()
        .flat_map(futures::stream::iter);
    while let Some(log) = stream.next().await {
        if let Ok(data) = PairCreatedV2::decode_log(&log.inner) {
            let id = PairId {
                version: UniswapVersion::V2,
                pair_address: data.pair,
            };
            let token0 = Token {
                address: data.token0,
                decimals: 18,
            };
            let token1 = Token {
                address: data.token1,
                decimals: 18,
            };
            let pair_data = PairData {
                pair_address: data.pair,
                version: UniswapVersion::V2,
                token0,
                token1,
            };
            sled_db.insert(id, pair_data).unwrap();
        }
    }

    Ok(())
}

/// V3 listener: fetches `PoolCreated` logs for the V3 factory at `block_number`
/// and decodes them into `PairData`.
pub async fn v3_listener(
    provider: &impl Provider,
    sled_db: Db,
) -> Result<(), Box<dyn std::error::Error>> {
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
        let id = PairId {
            version: UniswapVersion::V3,
            pair_address: ev.pool,
        };
        if !sled_db.contains_key(id).unwrap() {
            continue;
        }
        let token0 = Token {
            address: ev.token0,
            decimals: 18,
        };
        let token1 = Token {
            address: ev.token1,
            decimals: 18,
        };
        let data = PairData {
            pair_address: ev.pool,
            version: UniswapVersion::V3,
            token0,
            token1,
        };
        sled_db.insert(id, data).unwrap();
    }

    let mut stream = provider
        .watch_logs(&filter)
        .await?
        .into_stream()
        .flat_map(futures::stream::iter);
    while let Some(log) = stream.next().await {
        if let Ok(data) = PoolCreatedV3::decode_log(&log.inner) {
            let id = PairId {
                version: UniswapVersion::V3,
                pair_address: data.pool,
            };
            let token0 = Token {
                address: data.token0,
                decimals: 18,
            };
            let token1 = Token {
                address: data.token1,
                decimals: 18,
            };
            let pair_data = PairData {
                pair_address: data.pool,
                version: UniswapVersion::V3,
                token0,
                token1,
            };
            sled_db.insert(id, pair_data).unwrap();
        }
    }

    Ok(())
}

/// V4 listener: fetches `PoolCreated` logs for the V4 factory at `block_number`
/// and decodes them into `PairData`.
pub async fn v4_listener(
    provider: &impl Provider,
    sled_db: Db,
) -> Result<(), Box<dyn std::error::Error>> {
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
        // V4 pools are identified by poolId; the counterfactual address is
        // derived from the pool manager + PoolId. Stored here as the pool
        // manager address with the PoolId reserved for later resolution.
        let pair_address: Address = ev.hooks;
        let id = PairId {
            version: UniswapVersion::V4,
            pair_address,
        };
        if !sled_db.contains_key(id).unwrap() {
            continue;
        }
        let token0 = Token {
            address: ev.token0,
            decimals: 18,
        };
        let token1 = Token {
            address: ev.token1,
            decimals: 18,
        };
        let data = PairData {
            pair_address,
            version: UniswapVersion::V4,
            token0,
            token1,
        };
        sled_db.insert(id, data).unwrap();
    }

    let mut stream = provider
        .watch_logs(&filter)
        .await?
        .into_stream()
        .flat_map(futures::stream::iter);
    while let Some(log) = stream.next().await {
        if let Ok(data) = PoolCreatedV4::decode_log(&log.inner) {
            let pair_address: Address = data.hooks;
            let id = PairId {
                version: UniswapVersion::V4,
                pair_address,
            };
            let token0 = Token {
                address: data.token0,
                decimals: 18,
            };
            let token1 = Token {
                address: data.token1,
                decimals: 18,
            };
            let pair_data = PairData {
                pair_address,
                version: UniswapVersion::V4,
                token0,
                token1,
            };
            sled_db.insert(id, pair_data).unwrap();
        }
    }

    Ok(())
}
