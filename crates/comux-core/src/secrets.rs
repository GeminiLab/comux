//! Generation of listener credentials and display-safe token masking.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;

/// Generates a token from 32 random bytes using unpadded URL-safe Base64.
pub fn generate_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Masks a token while preserving identifying edges for longer values.
///
/// Empty input remains empty. Values shorter than nine Unicode scalar values
/// are fully masked; longer values preserve four scalar values at each end.
pub fn mask_token(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    if chars.len() < 9 {
        return "*".repeat(chars.len());
    }

    let mut masked = String::with_capacity(value.len());
    masked.extend(chars.iter().take(4));
    masked.push_str(&"*".repeat(chars.len() - 8));
    masked.extend(chars.iter().skip(chars.len() - 4));
    masked
}

#[cfg(test)]
mod tests {
    use super::{generate_token, mask_token};
    use std::collections::HashSet;

    #[test]
    fn masks_middle_and_preserves_four_character_edges() {
        assert_eq!(mask_token("abcdefghijklm"), "abcd*****jklm");
    }

    #[test]
    fn masks_short_tokens_entirely() {
        assert_eq!(mask_token("12345678"), "********");
        assert_eq!(mask_token(""), "");
    }

    #[test]
    fn generated_tokens_are_url_safe_and_unique() {
        let tokens: HashSet<_> = (0..32).map(|_| generate_token()).collect();
        assert_eq!(tokens.len(), 32);
        assert!(tokens.iter().all(|token| token.len() >= 40));
        assert!(tokens.iter().all(|token| token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))));
    }
}
