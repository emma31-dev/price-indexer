use crate::error::ServerError;
use alloy::primitives::Address;
use std::collections::HashMap;

/// The number of bytes in an ABI-encoded EVM address (`address` is 20 bytes).
pub const ADDRESS_LEN: usize = 20;
/// The number of bytes in an ABI-encoded `uint256` value.
pub const UINT256_LEN: usize = 32;
/// The number of bytes in an ABI-encoded `uint24` fee value.
pub const UINT24_LEN: usize = 4;
/// The number of bytes in an ABI-encoded `int24` tick spacing value.
pub const INT24_LEN: usize = 4;
/// The number of bytes in a Uniswap V4 `PoolId` (`bytes32`).
pub const POOL_ID_LEN: usize = 32;
/// The number of bytes used to store a token's decimal count (`uint8`).
pub const DECIMALS_LEN: usize = 1;

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

impl Token {
    /// The fixed-size byte layout of an encoded [`Token`]:
    /// `address` ([`ADDRESS_LEN`]) followed by `decimals` ([`DECIMALS_LEN`]).
    const ENCODED_LEN: usize = ADDRESS_LEN + DECIMALS_LEN;

    fn encode_into(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(self.address.as_slice());
        bytes.push(self.decimals);
    }

    fn decode_from(bytes: &[u8]) -> Self {
        let address = Address::from_slice(&bytes[0..ADDRESS_LEN]);
        let decimals = bytes[ADDRESS_LEN];
        Token { address, decimals }
    }
}

/// Data emitted by the Uniswap V1 factory `NewExchange(address,address)` event.
#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewExchangeV1Data {
    /// The token that was listed (an `address`, [`ADDRESS_LEN`] bytes).
    pub token: Address,
    /// The newly created exchange contract address (an `address`, [`ADDRESS_LEN`] bytes).
    pub exchange: Address,
}

#[allow(unused)]
impl NewExchangeV1Data {
    /// Encoded size: `token` ([`ADDRESS_LEN`]) + `exchange` ([`ADDRESS_LEN`]).
    const ENCODED_LEN: usize = ADDRESS_LEN + ADDRESS_LEN;
}

#[allow(unused)]
impl TryFrom<sled::IVec> for NewExchangeV1Data {
    type Error = ServerError;

    fn try_from(value: sled::IVec) -> Result<Self, Self::Error> {
        let bytes: &[u8] = value.as_ref();
        if bytes.len() != Self::ENCODED_LEN {
            return Err(ServerError::MissMatchByte {
                expected: Self::ENCODED_LEN,
                returned: bytes.len(),
            });
        }
        let token = Address::from_slice(&bytes[0..ADDRESS_LEN]);
        let exchange = Address::from_slice(&bytes[ADDRESS_LEN..ADDRESS_LEN + ADDRESS_LEN]);
        Ok(NewExchangeV1Data { token, exchange })
    }
}

#[allow(unused)]
impl From<&NewExchangeV1Data> for Vec<u8> {
    fn from(value: &NewExchangeV1Data) -> Self {
        let mut bytes = Vec::with_capacity(NewExchangeV1Data::ENCODED_LEN);
        bytes.extend_from_slice(value.token.as_slice());
        bytes.extend_from_slice(value.exchange.as_slice());
        bytes
    }
}

#[allow(unused)]
impl From<NewExchangeV1Data> for sled::IVec {
    fn from(value: NewExchangeV1Data) -> Self {
        sled::IVec::from(Vec::<u8>::from(&value))
    }
}

impl From<NewExchangeV1Data> for UniswapVersion {
    fn from(_: NewExchangeV1Data) -> Self {
        UniswapVersion::V1
    }
}

/// Data emitted by the Uniswap V2 factory `PairCreated(address,address,address,uint256)` event.
#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairCreatedV2Data {
    /// `token0` (an `address`, [`ADDRESS_LEN`] bytes).
    pub token0: Address,
    /// `token1` (an `address`, [`ADDRESS_LEN`] bytes).
    pub token1: Address,
    /// The newly created pair contract address (an `address`, [`ADDRESS_LEN`] bytes).
    pub pair: Address,
    /// `allPairsLength` (a `uint256`, [`UINT256_LEN`] bytes).
    pub all_pairs_length: alloy::primitives::U256,
}

#[allow(unused)]
impl PairCreatedV2Data {
    /// Encoded size: `token0` ([`ADDRESS_LEN`]) + `token1` ([`ADDRESS_LEN`])
    /// + `pair` ([`ADDRESS_LEN`]) + `all_pairs_length` ([`UINT256_LEN`]).
    const ENCODED_LEN: usize = ADDRESS_LEN + ADDRESS_LEN + ADDRESS_LEN + UINT256_LEN;
}

#[allow(unused)]
impl TryFrom<sled::IVec> for PairCreatedV2Data {
    type Error = ServerError;

    fn try_from(value: sled::IVec) -> Result<Self, Self::Error> {
        let bytes: &[u8] = value.as_ref();
        if bytes.len() != Self::ENCODED_LEN {
            return Err(ServerError::MissMatchByte {
                expected: Self::ENCODED_LEN,
                returned: bytes.len(),
            });
        }
        let token0 = Address::from_slice(&bytes[0..ADDRESS_LEN]);
        let token1 = Address::from_slice(&bytes[ADDRESS_LEN..2 * ADDRESS_LEN]);
        let pair = Address::from_slice(&bytes[2 * ADDRESS_LEN..3 * ADDRESS_LEN]);
        let all_pairs_length = alloy::primitives::U256::from_be_slice(
            &bytes[3 * ADDRESS_LEN..3 * ADDRESS_LEN + UINT256_LEN],
        );
        Ok(PairCreatedV2Data {
            token0,
            token1,
            pair,
            all_pairs_length,
        })
    }
}

#[allow(unused)]
impl From<&PairCreatedV2Data> for Vec<u8> {
    fn from(value: &PairCreatedV2Data) -> Self {
        let mut bytes = Vec::with_capacity(PairCreatedV2Data::ENCODED_LEN);
        bytes.extend_from_slice(value.token0.as_slice());
        bytes.extend_from_slice(value.token1.as_slice());
        bytes.extend_from_slice(value.pair.as_slice());
        bytes.extend_from_slice(&value.all_pairs_length.to_be_bytes::<UINT256_LEN>());
        bytes
    }
}

#[allow(unused)]
impl From<PairCreatedV2Data> for sled::IVec {
    fn from(value: PairCreatedV2Data) -> Self {
        sled::IVec::from(Vec::<u8>::from(&value))
    }
}

impl From<PairCreatedV2Data> for UniswapVersion {
    fn from(_: PairCreatedV2Data) -> Self {
        UniswapVersion::V2
    }
}

/// Data emitted by the Uniswap V3 factory `PoolCreated(address,address,uint24,int24,address)` event.
#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolCreatedV3Data {
    /// `token0` (an `address`, [`ADDRESS_LEN`] bytes).
    pub token0: Address,
    /// `token1` (an `address`, [`ADDRESS_LEN`] bytes).
    pub token1: Address,
    /// The pool fee, in hundredths of a bip (a `uint24`, [`UINT24_LEN`] bytes).
    pub fee: u32,
    /// `tickSpacing` (an `int24`, [`INT24_LEN`] bytes).
    pub tick_spacing: i32,
    /// The newly created pool contract address (an `address`, [`ADDRESS_LEN`] bytes).
    pub pool: Address,
}

#[allow(unused)]
impl PoolCreatedV3Data {
    /// Encoded size: `token0` ([`ADDRESS_LEN`]) + `token1` ([`ADDRESS_LEN`])
    /// + `fee` ([`UINT24_LEN`]) + `tick_spacing` ([`INT24_LEN`]) + `pool` ([`ADDRESS_LEN`]).
    const ENCODED_LEN: usize = ADDRESS_LEN + ADDRESS_LEN + UINT24_LEN + INT24_LEN + ADDRESS_LEN;
}

#[allow(unused)]
impl TryFrom<sled::IVec> for PoolCreatedV3Data {
    type Error = ServerError;

    fn try_from(value: sled::IVec) -> Result<Self, Self::Error> {
        let bytes: &[u8] = value.as_ref();
        if bytes.len() != Self::ENCODED_LEN {
            return Err(ServerError::MissMatchByte {
                expected: Self::ENCODED_LEN,
                returned: bytes.len(),
            });
        }
        let token0 = Address::from_slice(&bytes[0..ADDRESS_LEN]);
        let token1 = Address::from_slice(&bytes[ADDRESS_LEN..2 * ADDRESS_LEN]);
        let fee = u32::from_be_bytes(
            bytes[2 * ADDRESS_LEN..2 * ADDRESS_LEN + UINT24_LEN]
                .try_into()
                .unwrap(),
        );
        let tick_spacing = i32::from_be_bytes(
            bytes[2 * ADDRESS_LEN + UINT24_LEN..2 * ADDRESS_LEN + UINT24_LEN + INT24_LEN]
                .try_into()
                .unwrap(),
        );
        let pool = Address::from_slice(
            &bytes[2 * ADDRESS_LEN + UINT24_LEN + INT24_LEN..Self::ENCODED_LEN],
        );
        Ok(PoolCreatedV3Data {
            token0,
            token1,
            fee,
            tick_spacing,
            pool,
        })
    }
}

#[allow(unused)]
impl From<&PoolCreatedV3Data> for Vec<u8> {
    fn from(value: &PoolCreatedV3Data) -> Self {
        let mut bytes = Vec::with_capacity(PoolCreatedV3Data::ENCODED_LEN);
        bytes.extend_from_slice(value.token0.as_slice());
        bytes.extend_from_slice(value.token1.as_slice());
        bytes.extend_from_slice(&value.fee.to_be_bytes());
        bytes.extend_from_slice(&value.tick_spacing.to_be_bytes());
        bytes.extend_from_slice(value.pool.as_slice());
        bytes
    }
}

#[allow(unused)]
impl From<PoolCreatedV3Data> for sled::IVec {
    fn from(value: PoolCreatedV3Data) -> Self {
        sled::IVec::from(Vec::<u8>::from(&value))
    }
}

impl From<PoolCreatedV3Data> for UniswapVersion {
    fn from(_: PoolCreatedV3Data) -> Self {
        UniswapVersion::V3
    }
}

/// Data emitted by the Uniswap V4 factory
/// `PoolCreated(PoolId,address,address,uint24,int24,address)` event.
#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolCreatedV4Data {
    /// `token0` (an `address`, [`ADDRESS_LEN`] bytes).
    pub token0: Address,
    /// `token1` (an `address`, [`ADDRESS_LEN`] bytes).
    pub token1: Address,
    /// The pool fee, in hundredths of a bip (a `uint24`, [`UINT24_LEN`] bytes).
    pub fee: u32,
    /// `tickSpacing` (an `int24`, [`INT24_LEN`] bytes).
    pub tick_spacing: i32,
    /// The hooks contract address (an `address`, [`ADDRESS_LEN`] bytes).
    pub hooks: Address,
    /// The pool identifier (a `bytes32`, [`POOL_ID_LEN`] bytes).
    pub pool_id: [u8; 32],
}

#[allow(unused)]
impl PoolCreatedV4Data {
    /// Encoded size: `token0` ([`ADDRESS_LEN`]) + `token1` ([`ADDRESS_LEN`])
    /// + `fee` ([`UINT24_LEN`]) + `tick_spacing` ([`INT24_LEN`])
    /// + `hooks` ([`ADDRESS_LEN`]) + `pool_id` ([`POOL_ID_LEN`]).
    const ENCODED_LEN: usize =
        ADDRESS_LEN + ADDRESS_LEN + UINT24_LEN + INT24_LEN + ADDRESS_LEN + POOL_ID_LEN;
}

#[allow(unused)]
impl TryFrom<sled::IVec> for PoolCreatedV4Data {
    type Error = ServerError;

    fn try_from(value: sled::IVec) -> Result<Self, Self::Error> {
        let bytes: &[u8] = value.as_ref();
        if bytes.len() != Self::ENCODED_LEN {
            return Err(ServerError::MissMatchByte {
                expected: Self::ENCODED_LEN,
                returned: bytes.len(),
            });
        }
        let token0 = Address::from_slice(&bytes[0..ADDRESS_LEN]);
        let token1 = Address::from_slice(&bytes[ADDRESS_LEN..2 * ADDRESS_LEN]);
        let fee = u32::from_be_bytes(
            bytes[2 * ADDRESS_LEN..2 * ADDRESS_LEN + UINT24_LEN]
                .try_into()
                .unwrap(),
        );
        let tick_spacing = i32::from_be_bytes(
            bytes[2 * ADDRESS_LEN + UINT24_LEN..2 * ADDRESS_LEN + UINT24_LEN + INT24_LEN]
                .try_into()
                .unwrap(),
        );
        let hooks = Address::from_slice(
            &bytes[2 * ADDRESS_LEN + UINT24_LEN + INT24_LEN
                ..2 * ADDRESS_LEN + UINT24_LEN + INT24_LEN + ADDRESS_LEN],
        );
        let mut pool_id = [0u8; 32];
        pool_id.copy_from_slice(
            &bytes[2 * ADDRESS_LEN + UINT24_LEN + INT24_LEN + ADDRESS_LEN..Self::ENCODED_LEN],
        );
        Ok(PoolCreatedV4Data {
            token0,
            token1,
            fee,
            tick_spacing,
            hooks,
            pool_id,
        })
    }
}

#[allow(unused)]
impl From<&PoolCreatedV4Data> for Vec<u8> {
    fn from(value: &PoolCreatedV4Data) -> Self {
        let mut bytes = Vec::with_capacity(PoolCreatedV4Data::ENCODED_LEN);
        bytes.extend_from_slice(value.token0.as_slice());
        bytes.extend_from_slice(value.token1.as_slice());
        bytes.extend_from_slice(&value.fee.to_be_bytes());
        bytes.extend_from_slice(&value.tick_spacing.to_be_bytes());
        bytes.extend_from_slice(value.hooks.as_slice());
        bytes.extend_from_slice(&value.pool_id);
        bytes
    }
}

#[allow(unused)]
impl From<PoolCreatedV4Data> for sled::IVec {
    fn from(value: PoolCreatedV4Data) -> Self {
        sled::IVec::from(Vec::<u8>::from(&value))
    }
}

impl From<PoolCreatedV4Data> for UniswapVersion {
    fn from(_: PoolCreatedV4Data) -> Self {
        UniswapVersion::V4
    }
}

/// Any new-pair event, across all supported protocol versions.
#[allow(unused)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl From<&PairEvent> for Vec<u8> {
    fn from(value: &PairEvent) -> Self {
        let mut bytes = Vec::new();
        match value {
            PairEvent::V1(data) => {
                bytes.push(UniswapVersion::V1.into());
                bytes.extend_from_slice(&Vec::<u8>::from(data));
            }
            PairEvent::V2(data) => {
                bytes.push(UniswapVersion::V2.into());
                bytes.extend_from_slice(&Vec::<u8>::from(data));
            }
            PairEvent::V3(data) => {
                bytes.push(UniswapVersion::V3.into());
                bytes.extend_from_slice(&Vec::<u8>::from(data));
            }
            PairEvent::V4(data) => {
                bytes.push(UniswapVersion::V4.into());
                bytes.extend_from_slice(&Vec::<u8>::from(data));
            }
        }
        bytes
    }
}

impl TryFrom<&[u8]> for PairEvent {
    type Error = ServerError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let (&tag, rest) = bytes.split_first().ok_or_else(|| {
            ServerError::Unknown("Bytes cannot be split for pair event".to_string())
        })?;
        let version = UniswapVersion::from(tag);
        let data = rest.to_vec();
        match version {
            UniswapVersion::V1 => Ok(PairEvent::V1(NewExchangeV1Data::try_from(
                sled::IVec::from(data),
            )?)),
            UniswapVersion::V2 => Ok(PairEvent::V2(PairCreatedV2Data::try_from(
                sled::IVec::from(data),
            )?)),
            UniswapVersion::V3 => Ok(PairEvent::V3(PoolCreatedV3Data::try_from(
                sled::IVec::from(data),
            )?)),
            UniswapVersion::V4 => Ok(PairEvent::V4(PoolCreatedV4Data::try_from(
                sled::IVec::from(data),
            )?)),
        }
    }
}

impl From<&PairEvent> for sled::IVec {
    fn from(value: &PairEvent) -> Self {
        sled::IVec::from(Vec::<u8>::from(value))
    }
}

impl From<PairEvent> for sled::IVec {
    fn from(value: PairEvent) -> Self {
        sled::IVec::from(Vec::<u8>::from(&value))
    }
}

impl TryFrom<sled::IVec> for PairEvent {
    type Error = ServerError;

    fn try_from(value: sled::IVec) -> Result<Self, Self::Error> {
        PairEvent::try_from(value.as_ref())
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
        let mut pair_address = [0u8; ADDRESS_LEN];
        pair_address.copy_from_slice(&self.pool_id[0..ADDRESS_LEN]);
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedPairEvent {
    pub event: PairEvent,
    pub token0: Option<Token>,
    pub token1: Option<Token>,
}

#[allow(unused)]
impl From<&ResolvedPairEvent> for Vec<u8> {
    fn from(value: &ResolvedPairEvent) -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&Vec::<u8>::from(&value.event));
        for token in [value.token0, value.token1] {
            match token {
                Some(token) => {
                    bytes.push(1);
                    token.encode_into(&mut bytes);
                }
                None => bytes.push(0),
            }
        }
        bytes
    }
}

#[allow(unused)]
impl From<ResolvedPairEvent> for Vec<u8> {
    fn from(value: ResolvedPairEvent) -> Self {
        Vec::<u8>::from(&value)
    }
}

impl TryFrom<&[u8]> for ResolvedPairEvent {
    type Error = ServerError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let (event, rest) = decode_pair_event(bytes)?;
        let (token0, token1) = decode_optional_tokens(rest)?;
        Ok(ResolvedPairEvent {
            event,
            token0,
            token1,
        })
    }
}

/// Decodes a [`PairEvent`] from the front of `bytes`, returning the event and
/// the remaining unparsed bytes.
fn decode_pair_event(bytes: &[u8]) -> Result<(PairEvent, &[u8]), ServerError> {
    let (&tag, rest) = bytes
        .split_first()
        .ok_or_else(|| ServerError::Unknown("Bytes cannot be split for pair event".to_string()))?;
    let version = UniswapVersion::from(tag);
    let data_len = match version {
        UniswapVersion::V1 => NewExchangeV1Data::ENCODED_LEN,
        UniswapVersion::V2 => PairCreatedV2Data::ENCODED_LEN,
        UniswapVersion::V3 => PoolCreatedV3Data::ENCODED_LEN,
        UniswapVersion::V4 => PoolCreatedV4Data::ENCODED_LEN,
    };
    if rest.len() < data_len {
        return Err(ServerError::MissMatchByte {
            expected: data_len,
            returned: rest.len(),
        });
    }
    let (data, remaining) = rest.split_at(data_len);
    let data = sled::IVec::from(data.to_vec());
    let event = match version {
        UniswapVersion::V1 => PairEvent::V1(NewExchangeV1Data::try_from(data)?),
        UniswapVersion::V2 => PairEvent::V2(PairCreatedV2Data::try_from(data)?),
        UniswapVersion::V3 => PairEvent::V3(PoolCreatedV3Data::try_from(data)?),
        UniswapVersion::V4 => PairEvent::V4(PoolCreatedV4Data::try_from(data)?),
    };
    Ok((event, remaining))
}

/// Decodes the two length-prefixed optional [`Token`]s (token0 then token1).
fn decode_optional_tokens(bytes: &[u8]) -> Result<(Option<Token>, Option<Token>), ServerError> {
    let (token0, rest) = decode_optional_token(bytes)?;
    let (token1, rest) = decode_optional_token(rest)?;
    if !rest.is_empty() {
        return Err(ServerError::Unknown(
            "Trailing bytes after resolved pair event".to_string(),
        ));
    }
    Ok((token0, token1))
}

fn decode_optional_token(bytes: &[u8]) -> Result<(Option<Token>, &[u8]), ServerError> {
    let (&present, rest) = bytes.split_first().ok_or_else(|| {
        ServerError::Unknown("Bytes cannot be split for optional token".to_string())
    })?;
    match present {
        0 => Ok((None, rest)),
        1 => {
            if rest.len() < Token::ENCODED_LEN {
                return Err(ServerError::MissMatchByte {
                    expected: Token::ENCODED_LEN,
                    returned: rest.len(),
                });
            }
            let (data, remaining) = rest.split_at(Token::ENCODED_LEN);
            Ok((Some(Token::decode_from(data)), remaining))
        }
        _ => Err(ServerError::Unknown(format!(
            "Invalid token presence byte: {present}"
        ))),
    }
}

#[allow(unused)]
impl From<ResolvedPairEvent> for sled::IVec {
    fn from(value: ResolvedPairEvent) -> Self {
        sled::IVec::from(Vec::<u8>::from(&value))
    }
}

#[allow(unused)]
impl From<&ResolvedPairEvent> for sled::IVec {
    fn from(value: &ResolvedPairEvent) -> Self {
        sled::IVec::from(Vec::<u8>::from(value))
    }
}

#[allow(unused)]
impl TryFrom<sled::IVec> for ResolvedPairEvent {
    type Error = ServerError;

    fn try_from(value: sled::IVec) -> Result<Self, Self::Error> {
        ResolvedPairEvent::try_from(value.as_ref())
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
