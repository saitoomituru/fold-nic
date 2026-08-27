// SPDX-License-Identifier: AGPL-3.0-or-later

//! HTTP responseをpeerへ共有してよいか、保守的に判定するpure policy。

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use http::header::{AUTHORIZATION, CACHE_CONTROL, COOKIE, SET_COOKIE, VARY};
use http::{HeaderMap, Method, StatusCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CacheDisposition {
    ShareCache,
    PrivateCacheOnly,
    BypassSharedCache,
    NoStore,
    Error,
}

impl CacheDisposition {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ShareCache => "SHARE_CACHE",
            Self::PrivateCacheOnly => "PRIVATE_CACHE_ONLY",
            Self::BypassSharedCache => "BYPASS_SHARED_CACHE",
            Self::NoStore => "NO_STORE",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheDecision {
    pub disposition: CacheDisposition,
    pub reason_code: &'static str,
}

impl CacheDecision {
    #[must_use]
    pub const fn new(disposition: CacheDisposition, reason_code: &'static str) -> Self {
        Self {
            disposition,
            reason_code,
        }
    }
}

/// request／response metadataだけからshared-cache可否を決定する。
///
/// Stage 0では、GET／HEAD、status 200、明示freshness、personalizationなしの場合だけ
/// `SHARE_CACHE`を返す。判定不能を共有へ昇格しない。
#[must_use]
pub fn evaluate_shared_cache(
    method: &Method,
    request_headers: &HeaderMap,
    status: StatusCode,
    response_headers: &HeaderMap,
) -> CacheDecision {
    if method != Method::GET && method != Method::HEAD {
        return CacheDecision::new(
            CacheDisposition::BypassSharedCache,
            "UNSAFE_OR_MUTATING_METHOD",
        );
    }
    if status != StatusCode::OK {
        return CacheDecision::new(
            CacheDisposition::BypassSharedCache,
            "STATUS_NOT_SUPPORTED_STAGE0",
        );
    }

    let directives = match parse_cache_control(response_headers) {
        Ok(directives) => directives,
        Err(reason) => return CacheDecision::new(CacheDisposition::Error, reason),
    };

    if directives.contains_key("no-store") {
        return CacheDecision::new(CacheDisposition::NoStore, "RESPONSE_NO_STORE");
    }
    if directives.contains_key("private") && directives.contains_key("public") {
        return CacheDecision::new(
            CacheDisposition::Error,
            "CONFLICTING_PRIVATE_PUBLIC_DIRECTIVES",
        );
    }
    if directives.contains_key("private") {
        return CacheDecision::new(CacheDisposition::PrivateCacheOnly, "RESPONSE_PRIVATE");
    }
    if request_headers.contains_key(COOKIE) || response_headers.contains_key(SET_COOKIE) {
        return CacheDecision::new(
            CacheDisposition::PrivateCacheOnly,
            "COOKIE_OR_SET_COOKIE_PRESENT",
        );
    }
    if vary_contains_star(response_headers) {
        return CacheDecision::new(
            CacheDisposition::BypassSharedCache,
            "VARY_STAR_NOT_REUSABLE",
        );
    }
    if directives.contains_key("no-cache") {
        return CacheDecision::new(
            CacheDisposition::BypassSharedCache,
            "REVALIDATION_NOT_IMPLEMENTED",
        );
    }

    let has_shared_freshness = valid_nonnegative_seconds(&directives, "s-maxage")
        || valid_nonnegative_seconds(&directives, "max-age");
    let has_authorization = request_headers.contains_key(AUTHORIZATION);
    if has_authorization
        && !(directives.contains_key("public")
            || valid_nonnegative_seconds(&directives, "s-maxage"))
    {
        return CacheDecision::new(
            CacheDisposition::BypassSharedCache,
            "AUTHORIZATION_WITHOUT_SHARED_PERMISSION",
        );
    }
    if !has_shared_freshness {
        return CacheDecision::new(
            CacheDisposition::BypassSharedCache,
            "EXPLICIT_FRESHNESS_REQUIRED_STAGE0",
        );
    }

    CacheDecision::new(CacheDisposition::ShareCache, "EXPLICIT_SHARED_FRESHNESS")
}

type Directives = BTreeMap<String, Vec<Option<String>>>;

fn parse_cache_control(headers: &HeaderMap) -> Result<Directives, &'static str> {
    let mut directives = BTreeMap::<String, Vec<Option<String>>>::new();
    for value in headers.get_all(CACHE_CONTROL) {
        let text = value.to_str().map_err(|_| "CACHE_CONTROL_NON_UTF8")?;
        for part in split_directives(text)? {
            let (name, value) = match part.split_once('=') {
                Some((name, value)) => (name.trim(), Some(unquote(value.trim())?)),
                None => (part.trim(), None),
            };
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            {
                return Err("CACHE_CONTROL_INVALID_DIRECTIVE");
            }
            directives
                .entry(name.to_ascii_lowercase())
                .or_default()
                .push(value);
        }
    }
    Ok(directives)
}

fn split_directives(value: &str) -> Result<Vec<&str>, &'static str> {
    let mut parts = Vec::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut start = 0;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ',' if !quoted => {
                parts.push(value[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    if quoted || escaped {
        return Err("CACHE_CONTROL_UNTERMINATED_QUOTE");
    }
    parts.push(value[start..].trim());
    if parts.iter().any(|part| part.is_empty()) {
        return Err("CACHE_CONTROL_EMPTY_DIRECTIVE");
    }
    Ok(parts)
}

fn unquote(value: &str) -> Result<String, &'static str> {
    if !value.starts_with('"') {
        if value.contains('"') {
            return Err("CACHE_CONTROL_INVALID_QUOTE");
        }
        return Ok(value.to_owned());
    }
    if value.len() < 2 || !value.ends_with('"') {
        return Err("CACHE_CONTROL_UNTERMINATED_QUOTE");
    }
    let inner = &value[1..value.len() - 1];
    let mut result = String::with_capacity(inner.len());
    let mut escaped = false;
    for character in inner.chars() {
        if escaped {
            result.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Err("CACHE_CONTROL_INVALID_QUOTE");
        } else {
            result.push(character);
        }
    }
    if escaped {
        return Err("CACHE_CONTROL_INVALID_ESCAPE");
    }
    Ok(result)
}

fn valid_nonnegative_seconds(directives: &Directives, name: &str) -> bool {
    directives.get(name).is_some_and(|values| {
        values.len() == 1
            && values.iter().all(|value| {
                value.as_deref().is_some_and(|value| {
                    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
                })
            })
    })
}

fn vary_contains_star(headers: &HeaderMap) -> bool {
    headers.get_all(VARY).iter().any(|value| {
        value
            .to_str()
            .map(|text| text.split(',').any(|item| item.trim() == "*"))
            .unwrap_or(true)
    })
}

#[cfg(test)]
mod tests {
    use http::HeaderValue;

    use super::*;

    fn response_headers(cache_control: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CACHE_CONTROL, HeaderValue::from_static(cache_control));
        headers
    }

    fn evaluate(request: &HeaderMap, response: &HeaderMap) -> CacheDecision {
        evaluate_shared_cache(&Method::GET, request, StatusCode::OK, response)
    }

    #[test]
    fn public_max_ageを共有できる() {
        let decision = evaluate(&HeaderMap::new(), &response_headers("public, max-age=300"));
        assert_eq!(decision.disposition, CacheDisposition::ShareCache);
    }

    #[test]
    fn no_storeを保存しない() {
        let decision = evaluate(
            &HeaderMap::new(),
            &response_headers("public, max-age=300, no-store"),
        );
        assert_eq!(decision.disposition, CacheDisposition::NoStore);
    }

    #[test]
    fn privateをlocal専用へ落とす() {
        let decision = evaluate(&HeaderMap::new(), &response_headers("private, max-age=300"));
        assert_eq!(decision.disposition, CacheDisposition::PrivateCacheOnly);
    }

    #[test]
    fn cookie付きrequestを共有しない() {
        let mut request = HeaderMap::new();
        request.insert(COOKIE, HeaderValue::from_static("session=secret"));
        let decision = evaluate(&request, &response_headers("public, max-age=300"));
        assert_eq!(decision.disposition, CacheDisposition::PrivateCacheOnly);
    }

    #[test]
    fn set_cookie付きresponseを共有しない() {
        let mut response = response_headers("public, max-age=300");
        response.insert(SET_COOKIE, HeaderValue::from_static("session=secret"));
        let decision = evaluate(&HeaderMap::new(), &response);
        assert_eq!(decision.disposition, CacheDisposition::PrivateCacheOnly);
    }

    #[test]
    fn authorizationは明示shared_permissionなしで共有しない() {
        let mut request = HeaderMap::new();
        request.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret"));
        let decision = evaluate(&request, &response_headers("max-age=300"));
        assert_eq!(decision.disposition, CacheDisposition::BypassSharedCache);
    }

    #[test]
    fn authorizationとs_maxageは共有できる() {
        let mut request = HeaderMap::new();
        request.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret"));
        let decision = evaluate(&request, &response_headers("s-maxage=300"));
        assert_eq!(decision.disposition, CacheDisposition::ShareCache);
    }

    #[test]
    fn vary_starを共有しない() {
        let mut response = response_headers("public, max-age=300");
        response.insert(VARY, HeaderValue::from_static("*"));
        let decision = evaluate(&HeaderMap::new(), &response);
        assert_eq!(decision.disposition, CacheDisposition::BypassSharedCache);
    }

    #[test]
    fn freshnessなしを共有しない() {
        let decision = evaluate(&HeaderMap::new(), &response_headers("public"));
        assert_eq!(decision.disposition, CacheDisposition::BypassSharedCache);
    }

    #[test]
    fn 重複max_ageを共有しない() {
        let decision = evaluate(
            &HeaderMap::new(),
            &response_headers("public, max-age=300, max-age=600"),
        );
        assert_eq!(decision.disposition, CacheDisposition::BypassSharedCache);
    }

    #[test]
    fn 壊れたquoteをerrorにする() {
        let decision = evaluate(&HeaderMap::new(), &response_headers("max-age=\"300"));
        assert_eq!(decision.disposition, CacheDisposition::Error);
    }

    #[test]
    fn private_public競合をerrorにする() {
        let decision = evaluate(
            &HeaderMap::new(),
            &response_headers("private, public, max-age=300"),
        );
        assert_eq!(decision.disposition, CacheDisposition::Error);
    }

    #[test]
    fn postを共有しない() {
        let decision = evaluate_shared_cache(
            &Method::POST,
            &HeaderMap::new(),
            StatusCode::OK,
            &response_headers("public, max-age=300"),
        );
        assert_eq!(decision.disposition, CacheDisposition::BypassSharedCache);
    }

    #[test]
    fn partial_contentを共有しない() {
        let decision = evaluate_shared_cache(
            &Method::GET,
            &HeaderMap::new(),
            StatusCode::PARTIAL_CONTENT,
            &response_headers("public, max-age=300"),
        );
        assert_eq!(decision.disposition, CacheDisposition::BypassSharedCache);
    }
}
