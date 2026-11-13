use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::{env, sync::Arc, time::Duration};
use subtle::ConstantTimeEq;
use tower_http::{request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer}, trace::TraceLayer};

#[derive(Clone)]
struct AppState {
    db: PgPool,
    api_key: Arc<Vec<u8>>,
    rpc: RpcConfig,
    client: reqwest::Client,
}

#[derive(Clone)]
struct RpcConfig { source_url: String, destination_url: String }

#[derive(Debug, Serialize, sqlx::FromRow)]
struct MessageRow {
    message_id: String,
    source_domain: String,
    destination_domain: String,
    source_program: String,
    destination_program: String,
    sender: String,
    receiver: String,
    sequence: i64,
    payload_hash: String,
    source_signature: Option<String>,
    source_slot: i64,
    source_finalized_slot: Option<i64>,
    destination_signature: Option<String>,
    status: String,
    reason: String,
    confirmations: i64,
    required_confirmations: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct ListQuery { status: Option<String>, limit: Option<i64>, before: Option<DateTime<Utc>> }

#[derive(Serialize)]
struct ClusterStatus {
    name: &'static str,
    rpc_url: String,
    genesis_hash: Option<String>,
    domain_hash: Option<String>,
    slot: Option<u64>,
    healthy: bool,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApiError { code: &'static str, message: &'static str }

impl IntoResponse for ApiError {
    fn into_response(self) -> Response { (StatusCode::SERVICE_UNAVAILABLE, Json(self)).into_response() }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info,tower_http=info".into()))
        .init();

    let database_url = required_env("DATABASE_URL")?;
    let api_key = required_env("API_KEY")?;
    if api_key.len() < 24 && env::var("ALLOW_WEAK_LOCAL_API_KEY").as_deref() != Ok("true") {
        anyhow::bail!("API_KEY must be at least 24 bytes; short keys require ALLOW_WEAK_LOCAL_API_KEY=true for local development");
    }
    let db = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url).await?;
    sqlx::migrate!().run(&db).await?;
    let state = AppState {
        db,
        api_key: Arc::new(api_key.into_bytes()),
        rpc: RpcConfig {
            source_url: required_env("SOURCE_RPC_URL")?,
            destination_url: required_env("DESTINATION_RPC_URL")?,
        },
        client: reqwest::Client::builder().timeout(Duration::from_secs(3)).build()?,
    };
    initialize_cluster(&state, "local-source", "source", &state.rpc.source_url).await?;
    initialize_cluster(&state, "local-destination", "destination", &state.rpc.destination_url).await?;

    let operator = Router::new()
        .route("/clusters", get(clusters))
        .route("/routes", get(routes))
        .route("/messages", get(messages))
        .route("/messages/{id}", get(message))
        .route("/relayer/health", get(relayer_health))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_api_key));

    let app = Router::new()
        .route("/livez", get(livez))
        .route("/readyz", get(readyz))
        .nest("/api/v1", operator)
        .with_state(state)
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(TraceLayer::new_for_http());

    let port = env::var("PORT").unwrap_or_else(|_| "8080".into());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!(port, "RouterPulse API listening");
    axum::serve(listener, app).with_graceful_shutdown(shutdown()).await?;
    Ok(())
}

fn required_env(name: &str) -> anyhow::Result<String> {
    env::var(name).map_err(|_| anyhow::anyhow!("required environment variable {name} is missing"))
}

async fn shutdown() {
    let ctrl_c = async { let _ = tokio::signal::ctrl_c().await; };
    #[cfg(unix)]
    let terminate = async {
        let _ = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("register SIGTERM").recv().await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
}

async fn require_api_key(
    State(state): State<AppState>, headers: HeaderMap, request: axum::extract::Request,
    next: middleware::Next,
) -> Response {
    let supplied = headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or("").as_bytes();
    if supplied.len() != state.api_key.len() || !bool::from(supplied.ct_eq(state.api_key.as_slice())) {
        return (StatusCode::UNAUTHORIZED, Json(ApiError { code: "unauthorized", message: "A valid operator API key is required." })).into_response();
    }
    next.run(request).await
}

async fn livez() -> &'static str { "ok" }

async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&state.db).await {
        Ok(_) => (StatusCode::OK, Json(json!({"status":"ready","database":"ready"}))),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"status":"not_ready","database":"unavailable"}))),
    }
}

async fn clusters(State(state): State<AppState>) -> Json<Vec<ClusterStatus>> {
    let checks = [
        ("source", state.rpc.source_url.clone()),
        ("destination", state.rpc.destination_url.clone()),
    ];
    let results = join_all(checks.into_iter().map(|(name, url)| rpc_cluster(&state.client, name, url))).await;
    for cluster in &results {
        if let (Some(domain_hash), Some(slot)) = (&cluster.domain_hash, cluster.slot) {
            if let Ok(bytes) = hex::decode(domain_hash) {
                let _ = sqlx::query("UPDATE clusters SET last_observed_slot=$1,last_healthy_at=now() WHERE domain_hash=$2")
                    .bind(slot as i64).bind(bytes).execute(&state.db).await;
            }
        }
    }
    Json(results)
}

async fn rpc_cluster(client: &reqwest::Client, name: &'static str, rpc_url: String) -> ClusterStatus {
    let genesis = rpc_call(client, &rpc_url, "getGenesisHash", json!([])).await;
    let slot = rpc_call(client, &rpc_url, "getSlot", json!([{"commitment":"confirmed"}])).await;
    match (genesis, slot) {
        (Ok(g), Ok(s)) => {
            let genesis_hash = g.as_str().map(str::to_owned);
            let domain_hash = genesis_hash.as_deref().and_then(|g| cluster_domain(g, name).ok());
            ClusterStatus { name, rpc_url, genesis_hash, domain_hash, slot: s.as_u64(), healthy: true, error: None }
        },
        (g, s) => ClusterStatus {
            name, rpc_url, genesis_hash: g.ok().and_then(|v| v.as_str().map(str::to_owned)),
            domain_hash: None,
            slot: s.ok().and_then(|v| v.as_u64()), healthy: false,
            error: Some("RPC request failed or returned an invalid result".into()),
        },
    }
}

fn cluster_domain(genesis_hash: &str, label: &str) -> anyhow::Result<String> {
    let genesis = bs58::decode(genesis_hash).into_vec()?;
    anyhow::ensure!(genesis.len() == 32, "Solana genesis hash must be 32 bytes");
    let mut hasher = Sha256::new();
    hasher.update(b"ROUTERPULSE_CLUSTER_V1");
    hasher.update(genesis);
    hasher.update(b":");
    hasher.update(label.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

async fn initialize_cluster(state: &AppState, name: &str, label: &str, rpc_url: &str) -> anyhow::Result<()> {
    let genesis = rpc_call(&state.client, rpc_url, "getGenesisHash", json!([])).await?;
    let genesis_hash = genesis.as_str().ok_or_else(|| anyhow::anyhow!("getGenesisHash returned a non-string result"))?;
    let domain_hash = hex::decode(cluster_domain(genesis_hash, label)?)?;
    let ws_url = rpc_url.replacen("https://", "wss://", 1).replacen("http://", "ws://", 1);
    sqlx::query("INSERT INTO clusters(domain_hash,genesis_hash,name,rpc_url,websocket_url,commitment) VALUES($1,$2,$3,$4,$5,'confirmed') ON CONFLICT(domain_hash) DO UPDATE SET genesis_hash=EXCLUDED.genesis_hash,name=EXCLUDED.name,rpc_url=EXCLUDED.rpc_url,websocket_url=EXCLUDED.websocket_url,enabled=TRUE")
        .bind(domain_hash).bind(genesis_hash).bind(name).bind(rpc_url).bind(ws_url).execute(&state.db).await?;
    Ok(())
}

async fn rpc_call(client: &reqwest::Client, url: &str, method: &str, params: Value) -> anyhow::Result<Value> {
    let response: Value = client.post(url).json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
        .send().await?.error_for_status()?.json().await?;
    if let Some(error) = response.get("error") { anyhow::bail!("RPC error: {error}"); }
    response.get("result").cloned().ok_or_else(|| anyhow::anyhow!("JSON-RPC response missing result"))
}

async fn routes(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let rows = sqlx::query("SELECT r.id, sc.name AS source_name, dc.name AS destination_name, r.source_program, r.destination_program, r.source_confirmations, r.enabled, r.created_at FROM routes r JOIN clusters sc ON sc.domain_hash=r.source_domain_hash JOIN clusters dc ON dc.domain_hash=r.destination_domain_hash ORDER BY r.created_at DESC")
        .fetch_all(&state.db).await.map_err(|_| unavailable())?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(json!({
            "id": row.try_get::<uuid::Uuid, _>("id").map_err(|_| unavailable())?,
            "source_domain": row.try_get::<String, _>("source_name").map_err(|_| unavailable())?,
            "destination_domain": row.try_get::<String, _>("destination_name").map_err(|_| unavailable())?,
            "source_program": row.try_get::<String, _>("source_program").map_err(|_| unavailable())?,
            "destination_program": row.try_get::<String, _>("destination_program").map_err(|_| unavailable())?,
            "source_confirmations": row.try_get::<i32, _>("source_confirmations").map_err(|_| unavailable())?,
            "enabled": row.try_get::<bool, _>("enabled").map_err(|_| unavailable())?,
            "created_at": row.try_get::<DateTime<Utc>, _>("created_at").map_err(|_| unavailable())?,
        }));
    }
    Ok(Json(json!({"items":items,"total":items.len()})))
}

async fn messages(State(state): State<AppState>, Query(query): Query<ListQuery>) -> Result<Json<Value>, ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT encode(m.message_id,'hex') AS message_id, sc.name AS source_domain, dc.name AS destination_domain, m.source_program, m.destination_program, m.sender, m.receiver, m.sequence, encode(m.payload_hash,'hex') AS payload_hash, m.source_signature, m.source_slot, m.source_finalized_slot, m.destination_signature, m.status::text AS status, m.status_reason AS reason, m.created_at, m.updated_at, GREATEST(0, COALESCE(sc.last_observed_slot,0)-m.source_slot) AS confirmations, COALESCE(r.source_confirmations,0) AS required_confirmations FROM messages m JOIN clusters sc ON sc.domain_hash=m.source_domain_hash JOIN clusters dc ON dc.domain_hash=m.destination_domain_hash LEFT JOIN routes r ON r.source_domain_hash=m.source_domain_hash AND r.destination_domain_hash=m.destination_domain_hash AND r.source_program=m.source_program AND r.destination_program=m.destination_program WHERE ($1::text IS NULL OR m.status::text = $1) AND ($2::timestamptz IS NULL OR m.created_at < $2) ORDER BY m.created_at DESC LIMIT $3"
    ).bind(query.status).bind(query.before).bind(limit).fetch_all(&state.db).await.map_err(|_| unavailable())?;
    Ok(Json(json!({"items":rows,"total":rows.len(),"limit":limit})))
}

async fn message(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<MessageRow>, (StatusCode, Json<ApiError>)> {
    let bytes = hex::decode(id.strip_prefix("0x").unwrap_or(&id)).map_err(|_| (StatusCode::BAD_REQUEST, Json(ApiError { code:"invalid_message_id", message:"Message ID must be 32-byte hexadecimal." })))?;
    if bytes.len() != 32 { return Err((StatusCode::BAD_REQUEST, Json(ApiError { code:"invalid_message_id", message:"Message ID must be 32-byte hexadecimal." }))); }
    let row = sqlx::query_as::<_, MessageRow>(
        "SELECT encode(m.message_id,'hex') AS message_id, sc.name AS source_domain, dc.name AS destination_domain, m.source_program, m.destination_program, m.sender, m.receiver, m.sequence, encode(m.payload_hash,'hex') AS payload_hash, m.source_signature, m.source_slot, m.source_finalized_slot, m.destination_signature, m.status::text AS status, m.status_reason AS reason, m.created_at, m.updated_at, GREATEST(0, COALESCE(sc.last_observed_slot,0)-m.source_slot) AS confirmations, COALESCE(r.source_confirmations,0) AS required_confirmations FROM messages m JOIN clusters sc ON sc.domain_hash=m.source_domain_hash JOIN clusters dc ON dc.domain_hash=m.destination_domain_hash LEFT JOIN routes r ON r.source_domain_hash=m.source_domain_hash AND r.destination_domain_hash=m.destination_domain_hash AND r.source_program=m.source_program AND r.destination_program=m.destination_program WHERE m.message_id = $1"
    ).bind(bytes).fetch_optional(&state.db).await.map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, Json(unavailable())))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiError { code:"message_not_found", message:"Message was not found." })))?;
    Ok(Json(row))
}

async fn relayer_health(State(_state): State<AppState>) -> Json<Value> {
    Json(json!({"status":"scaffold","worker_enabled":false,"note":"Solana log scanner and submit worker are not yet active."}))
}

fn unavailable() -> ApiError { ApiError { code:"storage_unavailable", message:"The request could not be completed." } }

