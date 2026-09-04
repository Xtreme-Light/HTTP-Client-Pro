//! axum routes for the http-web API.
//!
//! Endpoints:
//! - `POST /execute` — body is a `.http` source for a single request;
//!   returns a JSON `DispatchResponse`.
//! - `GET /sse/execute?src=<http-source>` — same as POST but streams the
//!   response via Server-Sent Events for live UI updates.
//! - `GET /openapi.json` — OpenAPI 3.1 spec.
//! - `GET /docs/` — Swagger UI.
//!
//! Auth: if `AppState.auth_token` is `Some`, every route requires
//! `Authorization: Bearer <token>`.

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use futures_util::stream::Stream;
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;

/// Build the production axum router. The state is cloned cheaply into
/// each request handler.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/execute", post(execute).get(execute_via_sse))
        .route("/sse/execute", get(execute_via_sse))
        .route("/openapi.json", get(openapi_json))
        .route("/docs/", get(swagger_ui))
        .route("/docs/{path}", get(swagger_ui))
        .with_state(state)
}

/// `GET /healthz` — liveness probe. Does NOT require authentication so
/// Docker / orchestrator health checks can hit it without a token.
async fn healthz() -> axum::Json<serde_json::Value> {
    axum::Json(json!({ "status": "ok" }))
}

/// Authenticate the incoming request against `AppState.auth_token` (if set).
/// Returns the caller's headers on success; returns an `ApiError::Unauthorized`
/// if a token is required but not supplied or wrong.
fn require_auth(state: &AppState, headers: &HeaderMap) -> ApiResult<()> {
    let Some(expected) = &state.auth_token else {
        return Ok(());
    };
    let header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;
    let supplied = header.strip_prefix("Bearer ").unwrap_or(header);
    if supplied == expected.as_str() {
        Ok(())
    } else {
        Err(ApiError::Unauthorized)
    }
}

/// `POST /execute` body: a `.http` source. We parse the FIRST request in
/// the source — multi-request execution is a UI concern that uses the
/// SSE endpoint for streaming progress.
async fn execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> ApiResult<axum::Json<serde_json::Value>> {
    require_auth(&state, &headers)?;
    let file = http_core::parser::parse_file(&body).map_err(ApiError::Dispatch)?;
    let req = file
        .requests
        .into_iter()
        .next()
        .ok_or_else(|| ApiError::BadRequest("source contains no request".to_string()))?;
    let env = http_core::env::Environment::new();
    let res = state
        .dispatcher
        .send(&req, &env, None)
        .await
        .map_err(ApiError::Dispatch)?;
    Ok(axum::Json(json!({
        "status": res.status,
        "headers": res.headers.iter()
            .map(|(n, v)| (n.clone(), serde_json::Value::from(v.clone())))
            .collect::<serde_json::Map<String, serde_json::Value>>(),
        "body": String::from_utf8_lossy(&res.body).into_owned(),
        "elapsed_ms": res.elapsed.as_millis() as u64,
        "url": res.url,
    })))
}

#[derive(Deserialize)]
struct SseQuery {
    src: String,
}

/// `GET /sse/execute?src=<http-source>` — stream execution as SSE events.
/// Currently emits a single `done` event with the captured DispatchResponse
/// (no intermediate progress because http-core's dispatcher is synchronous
/// w.r.t. capturing — the body is read fully before returning). The SSE
/// transport still lets the UI show a progress indicator before the body
/// arrives.
async fn execute_via_sse(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SseQuery>,
) -> ApiResult<Sse<impl Stream<Item = std::result::Result<Event, Infallible>>>> {
    require_auth(&state, &headers)?;
    let file = http_core::parser::parse_file(&query.src).map_err(ApiError::Dispatch)?;
    let req = file
        .requests
        .into_iter()
        .next()
        .ok_or_else(|| ApiError::BadRequest("source contains no request".to_string()))?;

    // Execute inside the stream so the SSE handler returns immediately
    // and pushes events as they happen. For now, a single `done` event
    // wraps the DispatchResponse JSON. A future revision can push
    // `preparing` / `dispatching` / `done` events when the dispatcher
    // learns to surface intermediate progress.
    let stream = async_stream::stream! {
        let env = http_core::env::Environment::new();
        match state.dispatcher.send(&req, &env, None).await {
            Ok(res) => {
                yield Ok(Event::default()
                    .event("done")
                    .json_data(serde_json::json!({
                        "status": res.status,
                        "headers": res.headers.iter()
                            .map(|(n, v)| (n.clone(), serde_json::Value::from(v.clone())))
                            .collect::<serde_json::Map<String, serde_json::Value>>(),
                        "body": String::from_utf8_lossy(&res.body).into_owned(),
                        "elapsed_ms": res.elapsed.as_millis() as u64,
                        "url": res.url,
                    }))
                    .expect("json_data serializable"));
            }
            Err(e) => {
                yield Ok(Event::default()
                    .event("error")
                    .json_data(serde_json::json!({ "error": e.to_string() }))
                    .expect("json_data serializable"));
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// `GET /openapi.json` — serve the OpenAPI 3.1 spec.
async fn openapi_json(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<axum::Json<serde_json::Value>> {
    require_auth(&state, &headers)?;
    Ok(axum::Json(openapi_spec()))
}

/// `GET /docs/` — serve the Swagger UI at `/docs/`. The HTML is built
/// in-memory from the OpenAPI spec; no external assets needed.
async fn swagger_ui(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<impl IntoResponse> {
    require_auth(&state, &headers)?;
    let html = r#"<!DOCTYPE html>
<html>
  <head>
    <title>HTTP Client Pro — API</title>
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui.css">
  </head>
  <body>
    <div id="swagger-ui"></div>
    <script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
    <script>
      SwaggerUIBundle({ url: '/openapi.json', dom_id: '#swagger-ui' });
    </script>
  </body>
</html>"#;
    Ok((
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    ))
}

/// Build the static OpenAPI 3.1 spec.
///
/// Spec generation is kept simple (a hand-written `serde_json::Value`)
/// rather than pulling in utoipa's derive machinery: the API surface
/// is tiny, the project already uses serde_json everywhere, and
/// hand-writing avoids compile-time coupling between types and utoipa.
pub fn openapi_spec() -> serde_json::Value {
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "HTTP Client Pro Web API",
            "version": "0.1.0",
            "description": "Execute HTTP Request in Editor format (.http) sources and stream responses."
        },
        "paths": {
            "/healthz": {
                "get": {
                    "summary": "Liveness probe",
                    "description": "Returns `{\"status\":\"ok\"}`. Does not require authentication.",
                    "responses": {
                        "200": {
                            "description": "Server is alive",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": { "status": { "type": "string" } }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/execute": {
                "post": {
                    "summary": "Execute a single .http request",
                    "description": "Body is a `.http` source for one request. Returns the captured DispatchResponse as JSON.",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "text/plain": {
                                "schema": { "type": "string" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "DispatchResponse",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/DispatchResponse" }
                                }
                            }
                        },
                        "400": { "description": "Bad request — see error field" },
                        "401": { "description": "Unauthorized — missing/wrong bearer token" }
                    }
                }
            },
            "/sse/execute": {
                "get": {
                    "summary": "Execute a .http request via Server-Sent Events",
                    "parameters": [
                        {
                            "name": "src",
                            "in": "query",
                            "required": true,
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "text/event-stream of execution events",
                            "content": {
                                "text/event-stream": {}
                            }
                        },
                        "401": { "description": "Unauthorized — missing/wrong bearer token" }
                    }
                }
            },
            "/openapi.json": {
                "get": { "summary": "OpenAPI 3.1 spec" }
            },
            "/docs/": {
                "get": { "summary": "Swagger UI" }
            }
        },
        "components": {
            "schemas": {
                "DispatchResponse": {
                    "type": "object",
                    "properties": {
                        "status": { "type": "integer" },
                        "headers": { "type": "object" },
                        "body": { "type": "string" },
                        "elapsed_ms": { "type": "integer" },
                        "url": { "type": "string" }
                    },
                    "required": ["status", "headers", "body", "elapsed_ms", "url"]
                }
            }
        }
    })
}
