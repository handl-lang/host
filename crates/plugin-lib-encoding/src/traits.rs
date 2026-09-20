use crate::EncodingError;

/// Encodes a value of type `T` into a wire representation.
pub trait Encoder<T> {
    /// The encoded output type, typically `String`, `Vec<u8>`, or a
    /// typed frame (e.g. a WebSocket `Frame`).
    type Output;

    /// Encode `value` into the output format.
    fn encode(&self, value: &T) -> Result<Self::Output, EncodingError>;
}

/// Decodes input of type `I` into a value of type `T`.
pub trait Decoder<I: ?Sized, T> {
    /// Decode `input` from the wire format.
    fn decode(&self, input: &I) -> Result<T, EncodingError>;
}
