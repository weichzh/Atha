use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Transaction;

use super::model::MessageError;

pub(crate) fn random_id(transaction: &Transaction<'_>) -> Result<Vec<u8>, MessageError> {
    transaction
        .query_row("SELECT randomblob(16)", [], |row| row.get(0))
        .map_err(|_| MessageError::Database)
}

pub(crate) fn now_millis() -> Result<i64, MessageError> {
    let value = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| MessageError::Database)?
        .as_millis();
    i64::try_from(value).map_err(|_| MessageError::Database)
}

pub(crate) fn decode_hex<const N: usize>(value: &str) -> Result<Vec<u8>, MessageError> {
    if value.len() != N * 2 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(MessageError::InvalidInput);
    }
    (0..N)
        .map(|index| u8::from_str_radix(&value[index * 2..index * 2 + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| MessageError::InvalidInput)
}

pub(crate) fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(char::from(HEX[usize::from(byte >> 4)]));
        value.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    value
}
