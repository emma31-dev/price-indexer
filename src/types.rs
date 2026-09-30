use crate::error::ServerError;
use alloy::primitives::Address;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Which Uniswap protocol version a pair belongs to.
#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UniswapVersion {
    V1,
    V2,
    V3,
    V4,
}

impl From<u8> for UniswapVersion {
    fn from(value: u8) -> Self {
        match value {
            0 => UniswapVersion::V1,
            1 => UniswapVersion::V2,
            2 => UniswapVersion::V3,
            3 => UniswapVersion::V4,
            _ => UniswapVersion::V1,
        }
    }
}

impl From<UniswapVersion> for u8 {
    fn from(value: UniswapVersion) -> Self {
        match value {
            UniswapVersion::V1 => 0,
            UniswapVersion::V2 => 1,
            UniswapVersion::V3 => 2,
            UniswapVersion::V4 => 3,
        }
    }
}

/// Identifier for a tracked pair, scoped by protocol version.
///
/// Two pools can share the same address across versions in theory, and the same
/// token pair can exist on both v2 and v3, so the version is part of the key.
#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PairId {
    pub version: UniswapVersion,
    pub pair_address: Address,
}

impl AsRef<[u8]> for PairId {
    fn as_ref(&self) -> &[u8] {
        self.pair_address.as_ref()
    }
}

#[allow(unused)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub address: Address,
    pub decimals: u8,
    pub name: String,
    pub symbol: String,
}

/// Data emitted by the Uniswap V1 factory `NewExchange(address,address)` event.
#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewExchangeV1Data {
    /// The token that was listed (an `address`).
    pub token: Address,
    /// The newly created exchange contract address (an `address`).
    pub exchange: Address,
}

#[allow(unused)]
impl TryFrom<sled::IVec> for NewExchangeV1Data {
    type Error = ServerError;

    fn try_from(value: sled::IVec) -> Result<Self, Self::Error> {
        bincode::deserialize(value.as_ref()).map_err(ServerError::from)
    }
}

#[allow(unused)]
impl From<NewExchangeV1Data> for sled::IVec {
    fn from(value: NewExchangeV1Data) -> Self {
        sled::IVec::from(bincode::serialize(&value).unwrap_or_default())
    }
}

/// Data emitted by the Uniswap V2 factory `PairCreated(address,address,address,uint256)` event.
#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairCreatedV2Data {
    /// `token0` (an `address`).
    pub token0: Address,
    /// `token1` (an `address`).
    pub token1: Address,
    /// The newly created pair contract address (an `address`).
    pub pair: Address,
    /// `allPairsLength` (a `uint256`).
    pub all_pairs_length: alloy::primitives::U256,
}

#[allow(unused)]
impl TryFrom<sled::IVec> for PairCreatedV2Data {
    type Error = ServerError;

    fn try_from(value: sled::IVec) -> Result<Self, Self::Error> {
        bincode::deserialize(value.as_ref()).map_err(ServerError::from)
    }
}

#[allow(unused)]
impl From<PairCreatedV2Data> for sled::IVec {
    fn from(value: PairCreatedV2Data) -> Self {
        sled::IVec::from(bincode::serialize(&value).unwrap_or_default())
    }
}

/// Data emitted by the Uniswap V3 factory `PoolCreated(address,address,uint24,int24,address)` event.
#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolCreatedV3Data {
    /// `token0` (an `address`).
    pub token0: Address,
    /// `token1` (an `address`).
    pub token1: Address,
    /// The pool fee, in hundredths of a bip (a `uint24`).
    pub fee: u32,
    /// `tickSpacing` (an `int24`).
    pub tick_spacing: i32,
    /// The newly created pool contract address (an `address`).
    pub pool: Address,
}

#[allow(unused)]
impl TryFrom<sled::IVec> for PoolCreatedV3Data {
    type Error = ServerError;

    fn try_from(value: sled::IVec) -> Result<Self, Self::Error> {
        bincode::deserialize(value.as_ref()).map_err(ServerError::from)
    }
}

#[allow(unused)]
impl From<PoolCreatedV3Data> for sled::IVec {
    fn from(value: PoolCreatedV3Data) -> Self {
        sled::IVec::from(bincode::serialize(&value).unwrap_or_default())
    }
}

/// Data emitted by the Uniswap V4 factory
/// `PoolCreated(PoolId,address,address,uint24,int24,address)` event.
#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolCreatedV4Data {
    /// `token0` (an `address`).
    pub token0: Address,
    /// `token1` (an `address`).
    pub token1: Address,
    /// The pool fee, in hundredths of a bip (a `uint24`).
    pub fee: u32,
    /// `tickSpacing` (an `int24`).
    pub tick_spacing: i32,
    /// The hooks contract address (an `address`).
    pub hooks: Address,
    /// The pool identifier (a `bytes32`).
    pub pool_id: [u8; 32],
}

#[allow(unused)]
impl TryFrom<sled::IVec> for PoolCreatedV4Data {
    type Error = ServerError;

    fn try_from(value: sled::IVec) -> Result<Self, Self::Error> {
        bincode::deserialize(value.as_ref()).map_err(ServerError::from)
    }
}

#[allow(unused)]
impl From<PoolCreatedV4Data> for sled::IVec {
    fn from(value: PoolCreatedV4Data) -> Self {
        sled::IVec::from(bincode::serialize(&value).unwrap_or_default())
    }
}

/// Any new-pair event, across all supported protocol versions.
#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PairEvent {
    V1(NewExchangeV1Data),
    V2(PairCreatedV2Data),
    V3(PoolCreatedV3Data),
    V4(PoolCreatedV4Data),
}

impl PairEvent {
    #[allow(unused)]
    pub fn version(&self) -> UniswapVersion {
        match self {
            PairEvent::V1(_) => UniswapVersion::V1,
            PairEvent::V2(_) => UniswapVersion::V2,
            PairEvent::V3(_) => UniswapVersion::V3,
            PairEvent::V4(_) => UniswapVersion::V4,
        }
    }
}

impl TryFrom<sled::IVec> for PairEvent {
    type Error = ServerError;

    fn try_from(value: sled::IVec) -> Result<Self, Self::Error> {
        bincode::deserialize(value.as_ref()).map_err(ServerError::from)
    }
}

impl From<PairEvent> for sled::IVec {
    fn from(value: PairEvent) -> Self {
        sled::IVec::from(bincode::serialize(&value).unwrap_or_default())
    }
}

/// Recovers the [`PairId`] that a given new-pair event refers to.
#[allow(unused)]
pub trait AsPairId {
    fn pair_id(&self) -> PairId;
}

impl AsPairId for NewExchangeV1Data {
    fn pair_id(&self) -> PairId {
        PairId {
            version: UniswapVersion::V1,
            pair_address: self.exchange,
        }
    }
}

impl AsPairId for PairCreatedV2Data {
    fn pair_id(&self) -> PairId {
        PairId {
            version: UniswapVersion::V2,
            pair_address: self.pair,
        }
    }
}

impl AsPairId for PoolCreatedV3Data {
    fn pair_id(&self) -> PairId {
        PairId {
            version: UniswapVersion::V3,
            pair_address: self.pool,
        }
    }
}

impl AsPairId for PoolCreatedV4Data {
    fn pair_id(&self) -> PairId {
        // The v4 pool is identified by its `PoolId`; the address of the pool
        // manager is not the pair itself, so use the truncated pool id here.
        let mut pair_address = [0u8; 20];
        pair_address.copy_from_slice(&self.pool_id[0..20]);
        PairId {
            version: UniswapVersion::V4,
            pair_address: Address::from_slice(&pair_address),
        }
    }
}

impl AsPairId for PairEvent {
    fn pair_id(&self) -> PairId {
        match self {
            PairEvent::V1(data) => data.pair_id(),
            PairEvent::V2(data) => data.pair_id(),
            PairEvent::V3(data) => data.pair_id(),
            PairEvent::V4(data) => data.pair_id(),
        }
    }
}

/// A new-pair event along with the resolved [`Token`]s (address + decimals).
#[allow(unused)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedPairEvent {
    pub event: PairEvent,
    pub token0: Option<Token>,
    pub token1: Option<Token>,
}

#[allow(unused)]
impl TryFrom<sled::IVec> for ResolvedPairEvent {
    type Error = ServerError;

    fn try_from(value: sled::IVec) -> Result<Self, Self::Error> {
        bincode::deserialize(value.as_ref()).map_err(ServerError::from)
    }
}

#[allow(unused)]
impl From<ResolvedPairEvent> for sled::IVec {
    fn from(value: ResolvedPairEvent) -> Self {
        sled::IVec::from(bincode::serialize(&value).unwrap_or_default())
    }
}

impl AsPairId for ResolvedPairEvent {
    fn pair_id(&self) -> PairId {
        self.event.pair_id()
    }
}

/// Fast, in-memory lookup: pair address -> pair data.
#[allow(unused)]
pub type TrackedPairs = HashMap<PairId, PairEvent>;

/// Fast lookup from either token to the pairs that contain it.
#[allow(unused)]
pub type PairsByToken = HashMap<Address, Vec<PairId>>;

/// A single observed price for a pair.
#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TokenPrice {
    pub pair_id: PairId,
    /// Unix timestamp (seconds) at which the price was observed.
    pub timestamp: u64,
    pub price: f64,
}

/// Historical price data keyed by the pair it belongs to.
#[allow(unused)]
pub type TokenPrices = HashMap<PairId, Vec<TokenPrice>>;
