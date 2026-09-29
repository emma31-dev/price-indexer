use alloy::primitives::Address;
use std::collections::HashMap;

/// Which Uniswap protocol version a pair belongs to.
#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub address: Address,
    pub decimals: u8,
}

#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairData {
    /// Uniswap pair / pool contract address.
    pub pair_address: Address,
    pub version: UniswapVersion,
    pub token0: Token,
    pub token1: Token,
}

#[allow(unused)]
impl TryFrom<sled::IVec> for PairData {
    type Error = ();

    fn try_from(value: sled::IVec) -> Result<Self, Self::Error> {
        let bytes: &[u8] = value.as_ref();
        if bytes.len() != 1 + 20 + 1 + 20 + 1 + 20 {
            return Err(());
        }
        let version = UniswapVersion::from(bytes[0]);
        let pair_address = Address::from_slice(&bytes[1..21]);
        let token0 = Token {
            decimals: bytes[21],
            address: Address::from_slice(&bytes[22..42]),
        };
        let token1 = Token {
            decimals: bytes[42],
            address: Address::from_slice(&bytes[43..63]),
        };
        Ok(PairData {
            pair_address,
            version,
            token0,
            token1,
        })
    }
}

#[allow(unused)]
impl From<&PairData> for Vec<u8> {
    fn from(value: &PairData) -> Self {
        let mut bytes = Vec::with_capacity(1 + 20 + 1 + 20 + 1 + 20);
        bytes.push(u8::from(value.version));
        bytes.extend_from_slice(value.pair_address.as_slice());
        bytes.push(value.token0.decimals);
        bytes.extend_from_slice(value.token0.address.as_slice());
        bytes.push(value.token1.decimals);
        bytes.extend_from_slice(value.token1.address.as_slice());
        bytes
    }
}

#[allow(unused)]
impl From<PairData> for sled::IVec {
    fn from(value: PairData) -> Self {
        sled::IVec::from(Vec::<u8>::from(&value))
    }
}

impl PairData {
    #[allow(unused)]
    pub fn id(&self) -> PairId {
        PairId {
            version: self.version,
            pair_address: self.pair_address,
        }
    }
}

/// Fast, in-memory lookup: pair address -> pair data.
#[allow(unused)]
pub type TrackedPairs = HashMap<PairId, PairData>;

/// Fast lookup from either token to the pairs that contain it.
#[allow(unused)]
pub type PairsByToken = HashMap<Address, Vec<PairId>>;

/// A single observed price for a pair.
#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TokenPrice {
    pub pair_id: PairId,
    /// Unix timestamp (seconds) at which the price was observed.
    pub timestamp: u64,
    pub price: f64,
}

/// Historical price data keyed by the pair it belongs to.
#[allow(unused)]
pub type TokenPrices = HashMap<PairId, Vec<TokenPrice>>;
