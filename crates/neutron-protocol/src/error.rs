//! Error types for packet encoding and decoding.

/// Errors that occur when decoding (reading) packets from bytes.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    /// Not enough bytes remaining to read the expected data.
    #[error("insufficient bytes: need {need} but only {have} available")]
    InsufficientBytes { need: usize, have: usize },

    /// An invalid packet ID was received for the current protocol state.
    #[error("unknown packet id 0x{id:02X} in state {state}")]
    UnknownPacketId { id: u32, state: &'static str },

    /// A VarInt could not be decoded (exceeded 5 bytes or missing terminator).
    #[error("invalid varint encoding")]
    InvalidVarInt,

    /// A VarLong could not be decoded.
    #[error("invalid varlong encoding")]
    InvalidVarLong,

    /// The decoded string was not valid UTF-8.
    #[error("invalid UTF-8 string: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),

    /// The decoded string exceeded the maximum allowed length.
    #[error("string too long: {len} bytes (max {max})")]
    StringTooLong { len: usize, max: usize },

    /// Invalid position encoding (not within valid world bounds).
    #[error("invalid block position: {0}")]
    InvalidPosition(i64),

    /// Invalid angle value (must be 0-255).
    #[error("invalid angle: {0}")]
    InvalidAngle(u8),

    /// The compressed packet failed to decompress.
    #[error("decompression failed: {0}")]
    DecompressionFailed(String),

    /// Generic decode error with a message.
    #[error("decode error: {0}")]
    Other(String),
}

/// Errors that occur when encoding (writing) packets to bytes.
#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    /// The encoded data exceeds the maximum packet size.
    #[error("packet too large: {size} bytes (max {max})")]
    PacketTooLarge { size: usize, max: usize },

    /// A VarInt could not be encoded (value out of range).
    #[error("varint out of range: {0}")]
    VarIntOutOfRange(i32),

    /// The string exceeds the maximum encoded length.
    #[error("string too long to encode: {len} bytes (max {max})")]
    StringTooLong { len: usize, max: usize },

    /// Generic encode error with a message.
    #[error("encode error: {0}")]
    Other(String),
}

/// Unified error type for protocol operations.
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error(transparent)]
    Decode(#[from] DecodeError),

    #[error(transparent)]
    Encode(#[from] EncodeError),

    /// I/O error from the underlying transport.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// A specialized `Result` type for protocol operations.
pub type Result<T> = std::result::Result<T, ProtocolError>;
