// SPDX-License-Identifier: MIT

//! Small dependency-free helpers for bounded raw URL-safe Base64 validation.

fn value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

pub(crate) fn decoded_len_if_canonical(value_text: &str) -> Option<usize> {
    let bytes = value_text.as_bytes();
    let remainder = bytes.len() % 4;
    if remainder == 1 || bytes.iter().any(|byte| value(*byte).is_none()) {
        return None;
    }
    let final_value = bytes.last().and_then(|byte| value(*byte));
    if (remainder == 2 && final_value.is_none_or(|item| item & 0x0f != 0))
        || (remainder == 3 && final_value.is_none_or(|item| item & 0x03 != 0))
    {
        return None;
    }
    Some(bytes.len() / 4 * 3 + usize::from(remainder == 2) + 2 * usize::from(remainder == 3))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_raw_urlsafe_alphabet_length_and_unused_bits() {
        assert_eq!(decoded_len_if_canonical(""), Some(0));
        assert_eq!(decoded_len_if_canonical(&"A".repeat(43)), Some(32));
        assert_eq!(decoded_len_if_canonical(&"A".repeat(1579)), Some(1184));
        assert_eq!(decoded_len_if_canonical("A"), None);
        assert_eq!(decoded_len_if_canonical("+///"), None);
        assert_eq!(
            decoded_len_if_canonical(&format!("{}B", "A".repeat(42))),
            None
        );
    }
}
