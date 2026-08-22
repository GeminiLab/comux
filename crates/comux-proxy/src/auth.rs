use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthError {
    Missing,
    Invalid,
}

pub fn bearer_token(headers: &HeaderMap) -> Result<&str, AuthError> {
    let value = headers.get(AUTHORIZATION).ok_or(AuthError::Missing)?;
    let value = value.to_str().map_err(|_| AuthError::Invalid)?;
    let (scheme, token) = value.split_once(' ').ok_or(AuthError::Invalid)?;
    if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() || token.contains(' ') {
        return Err(AuthError::Invalid);
    }
    Ok(token)
}

pub fn is_valid(headers: &HeaderMap, expected: &str) -> bool {
    bearer_token(headers).is_ok_and(|token| token == expected)
}

#[cfg(test)]
mod tests {
    use super::{bearer_token, is_valid, AuthError};
    use axum::http::header::AUTHORIZATION;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn extracts_bearer_token_case_insensitively() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("bEaReR secret"));
        assert_eq!(bearer_token(&headers), Ok("secret"));
        assert!(is_valid(&headers, "secret"));
    }

    #[test]
    fn rejects_missing_malformed_and_wrong_tokens() {
        let headers = HeaderMap::new();
        assert_eq!(bearer_token(&headers), Err(AuthError::Missing));

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Basic secret"));
        assert_eq!(bearer_token(&headers), Err(AuthError::Invalid));
        assert!(!is_valid(&headers, "secret"));
    }
}
