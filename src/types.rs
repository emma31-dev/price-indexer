use crate::error::ServerError;
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};

/// Which Uniswap protocol version a pair belongs to.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Archive,
    RkyvSerialize,
    RkyvDeserialize,
)]
#[rkyv(derive(Debug))]
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

/// The address/index that identifies a pair within its protocol version.
///
/// For Uniswap V1/V2/V3 a pair is identified by a contract address. For
/// Uniswap V4 a pool is instead identified by its 32-byte `PoolId`.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Archive,
    RkyvSerialize,
    RkyvDeserialize,
)]
#[rkyv(derive(Debug))]
pub enum PairAddress {
    /// A 20-byte contract address, used by V1/V2/V3.
    Address([u8; 20]),
    /// The 32-byte `PoolId`, used by V4.
    PoolId([u8; 32]),
}

impl AsRef<[u8]> for PairAddress {
    fn as_ref(&self) -> &[u8] {
        match self {
            PairAddress::Address(address) => address.as_ref(),
            PairAddress::PoolId(pool_id) => pool_id.as_ref(),
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Archive,
    RkyvSerialize,
    RkyvDeserialize,
)]
#[rkyv(derive(Debug))]
pub struct TokenPairId {
    pub version: UniswapVersion,
    pub pair_address: PairAddress,
    pub timestamp: u64,
}

impl AsRef<[u8]> for TokenPairId {
    fn as_ref(&self) -> &[u8] {
        self.pair_address.as_ref()
    }
}

/// The value stored for each observed price tick.
#[derive(
    Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Archive, RkyvSerialize, RkyvDeserialize,
)]
#[rkyv(derive(Debug))]
pub struct TickMeta {
    /// The price derived from the event.
    pub price: f64,
    /// The volume moved by the event (in the pair's base units).
    pub volume: f64,
    /// The block number the event belongs to, used to verify the tick ordering.
    pub block: u64,
}

impl TryFrom<sled::IVec> for TickMeta {
    type Error = ServerError;

    fn try_from(value: sled::IVec) -> Result<Self, Self::Error> {
        let archived = rkyv::access::<ArchivedTickMeta, rkyv::rancor::Error>(value.as_ref())?;
        let meta = rkyv::deserialize::<TickMeta, rkyv::rancor::Error>(archived)?;
        Ok(meta)
    }
}

#[allow(unused)]
impl From<TickMeta> for sled::IVec {
    fn from(value: TickMeta) -> Self {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&value).unwrap();
        sled::IVec::from(bytes.as_ref())
    }
}

// ============ Request & Response Types ==============
#[derive(serde::Deserialize)]
pub struct PriceRequest {
    pub pair_address: PairAddress,
    pub version: UniswapVersion,
}

#[derive(serde::Deserialize)]
pub struct PriceAtRequest {
    pub pair_address: PairAddress,
    pub version: UniswapVersion,
    pub timestamp: u64,
}

#[derive(serde::Deserialize)]
pub struct PricesRangeRequest {
    pub pair_address: PairAddress,
    pub version: UniswapVersion,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
}

#[derive(Serialize)]
pub struct PriceResponse {
    pub price: f64,
    pub last_updated: u64,
}

#[derive(Serialize)]
pub struct TimestampedPrice {
    pub price: f64,
    pub timestamp: u64,
}

#[derive(Serialize)]
pub struct PricesRangeResponse {
    pub prices: Vec<TimestampedPrice>,
}

#[derive(Serialize)]
pub struct Ohlc {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub timestamp: u64,
}
