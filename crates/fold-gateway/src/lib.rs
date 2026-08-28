// SPDX-License-Identifier: AGPL-3.0-or-later

//! localhostからhealthと検証済みCAS objectだけを返すread-only Gateway。

#![forbid(unsafe_code)]

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header::{
    CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE, ETAG, HeaderValue, RETRY_AFTER,
};
use axum::http::{Response, StatusCode};
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use cid::Cid;
use fold_kamii::{
    DEFAULT_GATE_TIMEOUT_MILLIS, KamiiInspectionRequest, KamiiOutcome, KamiiVerdict,
    allow_all_placeholder_adapter, invoke_kamii_adapter,
};
use fold_store::{LocalCas, StoreErrorCode};
use serde::Serialize;

const FOLD_CID: &str = "x-fold-cid";
const CONTENT_TYPE_OPTIONS: &str = "x-content-type-options";

#[derive(Debug, Clone, Serialize)]
pub struct GatewayHealth {
    pub schema: &'static str,
    pub status: &'static str,
    pub runtime_state: &'static str,
    pub network_scope: &'static str,
    pub capabilities: Vec<&'static str>,
    pub explicit_non_capabilities: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    schema: &'static str,
    code: &'static str,
    detail: &'static str,
}

#[derive(Debug)]
struct GatewayResponseError {
    status: StatusCode,
    code: &'static str,
    detail: &'static str,
    retry_after_seconds: Option<u32>,
}

impl GatewayResponseError {
    const fn new(status: StatusCode, code: &'static str, detail: &'static str) -> Self {
        Self {
            status,
            code,
            detail,
            retry_after_seconds: None,
        }
    }

    /// 503等、再試行が意味を持つ応答へ`Retry-After`候補秒数を添える版。
    const fn new_with_retry_after(
        status: StatusCode,
        code: &'static str,
        detail: &'static str,
        retry_after_seconds: u32,
    ) -> Self {
        Self {
            status,
            code,
            detail,
            retry_after_seconds: Some(retry_after_seconds),
        }
    }
}

impl IntoResponse for GatewayResponseError {
    fn into_response(self) -> axum::response::Response {
        let mut response = (
            self.status,
            Json(ErrorBody {
                schema: "fold-gateway-error/0",
                code: self.code,
                detail: self.detail,
            }),
        )
            .into_response();
        if let Some(seconds) = self.retry_after_seconds
            && let Ok(value) = HeaderValue::from_str(&seconds.to_string())
        {
            response.headers_mut().insert(RETRY_AFTER, value);
        }
        response
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindScopeError;

impl std::fmt::Display for BindScopeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("GATEWAY_NON_LOOPBACK_BIND_DENIED: Stage 0はloopbackだけへbindできます")
    }
}

impl std::error::Error for BindScopeError {}

/// Stage 0で外部interfaceへのbindを拒否する。
///
/// # Errors
///
/// IP addressがloopbackでない場合に返す。
pub fn validate_bind_scope(bind: SocketAddr) -> Result<(), BindScopeError> {
    if bind.ip().is_loopback() {
        Ok(())
    } else {
        Err(BindScopeError)
    }
}

/// [`get_object`]がobject配信前に呼ぶKamii adapterの型。
///
/// 実process分離までのStage 0では単純な関数ポインタとして扱う。
pub type KamiiAdapterFn = fn(&KamiiInspectionRequest) -> KamiiVerdict;

#[derive(Clone)]
struct GatewayState {
    cas: LocalCas,
    kamii_adapter: KamiiAdapterFn,
}

pub fn router(cas: LocalCas) -> Router {
    router_with_kamii_adapter(cas, allow_all_placeholder_adapter)
}

/// [`router`]と同じroutingに加え、Kamii adapterを差し替えられる版。
///
/// 実process分離やtest向けにadapterを注入する用途を想定する。
pub fn router_with_kamii_adapter(cas: LocalCas, kamii_adapter: KamiiAdapterFn) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/v0/objects/{cid}", get(get_object))
        .with_state(GatewayState { cas, kamii_adapter })
}

async fn health() -> Json<GatewayHealth> {
    Json(GatewayHealth {
        schema: "fold-gateway-health/0",
        status: "pass",
        runtime_state: "READ_ONLY_LOCAL_GATEWAY_ALPHA",
        network_scope: "LOOPBACK_ONLY",
        capabilities: vec!["health", "verified-local-cas-read"],
        explicit_non_capabilities: vec![
            "write-endpoint",
            "origin-fetch",
            "p2p",
            "standalone-runtime",
        ],
    })
}

async fn get_object(
    State(GatewayState { cas, kamii_adapter }): State<GatewayState>,
    Path(cid_text): Path<String>,
) -> Result<Response<Body>, GatewayResponseError> {
    let cid = cid_text.parse::<Cid>().map_err(|_| {
        GatewayResponseError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_CID",
            "CIDを解釈できません",
        )
    })?;
    let cid_for_read = cid;
    let bytes = tokio::task::spawn_blocking(move || cas.get(&cid_for_read))
        .await
        .map_err(|_| {
            GatewayResponseError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CAS_TASK_FAILED",
                "CAS read taskが完了しませんでした",
            )
        })?
        .map_err(|error| match error.code {
            StoreErrorCode::ObjectNotFound => GatewayResponseError::new(
                StatusCode::NOT_FOUND,
                "CAS_OBJECT_NOT_FOUND",
                "CAS objectがありません",
            ),
            StoreErrorCode::CorruptObject => GatewayResponseError::new(
                StatusCode::CONFLICT,
                "CAS_CORRUPT_OBJECT",
                "CAS objectの検証に失敗しました",
            ),
            _ => GatewayResponseError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CAS_READ_FAILED",
                "CAS objectを安全に読めませんでした",
            ),
        })?;

    let cid_text = cid.to_string();
    let byte_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let inspection = KamiiInspectionRequest {
        cid: cid_text.clone(),
        byte_len,
    };
    let outcome = tokio::task::spawn_blocking(move || {
        invoke_kamii_adapter(
            inspection,
            Duration::from_millis(DEFAULT_GATE_TIMEOUT_MILLIS),
            kamii_adapter,
        )
    })
    .await
    .map_err(|_| {
        GatewayResponseError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "KAMII_TASK_FAILED",
            "Kamii gate taskが完了しませんでした",
        )
    })?;
    if outcome.resolved_verdict() != KamiiVerdict::Allow {
        return Err(kamii_rejection_error(&outcome));
    }
    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = StatusCode::OK;
    let headers = response.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("public, immutable, max-age=31536000"),
    );
    headers.insert(CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(
        ETAG,
        HeaderValue::from_str(&format!("\"{cid_text}\""))
            .expect("CID text is a valid header value"),
    );
    headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{cid_text}.bin\""))
            .expect("CID text is a valid header value"),
    );
    headers.insert(
        FOLD_CID,
        HeaderValue::from_str(&cid_text).expect("CID text is a valid header value"),
    );
    Ok(response)
}

/// Kamii adapterのtimeout／crash／容量超過を再試行の目安として提示するdefault秒数。
const KAMII_UNAVAILABLE_RETRY_AFTER_SECONDS: u32 = 1;

/// Kamii拒否理由を、安全判定（明示Deny／Quarantine）と障害・容量診断
/// （timeout／crash／capacity超過）で別status・別codeに分ける。
///
/// 明示的な`Deny`は403（再試行しても変わらない判断）、`Timeout`／`Crashed`／
/// `Saturated`は503 + `Retry-After`（一時的な利用不能）として区別する。
fn kamii_rejection_error(outcome: &KamiiOutcome) -> GatewayResponseError {
    match outcome {
        KamiiOutcome::Verdict(_) => GatewayResponseError::new(
            StatusCode::FORBIDDEN,
            "GATEWAY_KAMII_DENIED",
            "Kamii adapterがobjectの配信を明示的に拒否しました",
        ),
        KamiiOutcome::Timeout | KamiiOutcome::Crashed | KamiiOutcome::Saturated => {
            GatewayResponseError::new_with_retry_after(
                StatusCode::SERVICE_UNAVAILABLE,
                "GATEWAY_KAMII_UNAVAILABLE",
                "Kamii adapterが一時的に応答しませんでした",
                KAMII_UNAVAILABLE_RETRY_AFTER_SECONDS,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use axum::body::to_bytes;
    use http::Request;
    use tower::ServiceExt;

    use super::*;

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time after epoch")
                .as_nanos();
            Self(std::env::temp_dir().join(format!(
                "fold-gateway-test-{label}-{}-{nonce}",
                std::process::id()
            )))
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn healthは未実装境界を返す() {
        let root = TestRoot::new("health");
        let cas = LocalCas::open(&root.0, 1024).expect("open CAS");
        let response = router(cas)
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 4096)
            .await
            .expect("health body");
        let text = String::from_utf8(body.to_vec()).expect("UTF-8 health");
        assert!(text.contains("\"p2p\""));
        assert!(text.contains("\"standalone-runtime\""));
    }

    #[tokio::test]
    async fn cas_objectを再検証して返す() {
        let root = TestRoot::new("object");
        let cas = LocalCas::open(&root.0, 1024).expect("open CAS");
        let receipt = cas.put(b"fold gateway fixture").expect("put fixture");
        let response = router(cas)
            .oneshot(
                Request::builder()
                    .uri(format!("/v0/objects/{}", receipt.cid))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(CONTENT_TYPE_OPTIONS),
            Some(&HeaderValue::from_static("nosniff"))
        );
        let body = to_bytes(response.into_body(), 1024)
            .await
            .expect("object body");
        assert_eq!(&body[..], b"fold gateway fixture");
    }

    fn deny_adapter(_: &KamiiInspectionRequest) -> KamiiVerdict {
        KamiiVerdict::Deny
    }

    fn hanging_adapter(_: &KamiiInspectionRequest) -> KamiiVerdict {
        std::thread::sleep(Duration::from_secs(2));
        KamiiVerdict::Allow
    }

    #[tokio::test]
    async fn kamiiがdenyを返せば配信を拒否する() {
        let root = TestRoot::new("kamii-deny");
        let cas = LocalCas::open(&root.0, 1024).expect("open CAS");
        let receipt = cas.put(b"quarantine candidate").expect("put fixture");
        let response = router_with_kamii_adapter(cas, deny_adapter)
            .oneshot(
                Request::builder()
                    .uri(format!("/v0/objects/{}", receipt.cid))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn kamiiがtimeoutすれば503とretry_afterで配信を拒否する() {
        let root = TestRoot::new("kamii-timeout");
        let cas = LocalCas::open(&root.0, 1024).expect("open CAS");
        let receipt = cas.put(b"timeout candidate").expect("put fixture");
        let response = router_with_kamii_adapter(cas, hanging_adapter)
            .oneshot(
                Request::builder()
                    .uri(format!("/v0/objects/{}", receipt.cid))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(response.headers().get(RETRY_AFTER).is_some());
    }

    #[tokio::test]
    async fn 不在objectは404を返す() {
        let root = TestRoot::new("missing");
        let cas = LocalCas::open(&root.0, 1024).expect("open CAS");
        let missing = fold_store::cid_for(b"missing");
        let response = router(cas)
            .oneshot(
                Request::builder()
                    .uri(format!("/v0/objects/{missing}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn 壊れた_cidは400を返す() {
        let root = TestRoot::new("bad-cid");
        let cas = LocalCas::open(&root.0, 1024).expect("open CAS");
        let response = router(cas)
            .oneshot(
                Request::builder()
                    .uri("/v0/objects/not-a-cid")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn 外部interfaceへのbindを拒否する() {
        let bind = "0.0.0.0:7743".parse().expect("socket address");
        assert_eq!(validate_bind_scope(bind), Err(BindScopeError));
    }
}
