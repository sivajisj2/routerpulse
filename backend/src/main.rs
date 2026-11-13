use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, net::SocketAddr, sync::{Arc, RwLock}};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

#[derive(Clone)]
struct AppState {
    api_key: String,
    messages: Arc<RwLock<HashMap<String, Message>>>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Message {
    message_id: String,
    source_chain_id: u64,
    destination_chain_id: u64,
    sender: String,
    receiver: String,
    status: String,
    reason: String,
    source_tx_hash: Option<String>,
    destination_tx_hash: Option<String>,
    confirmations: u64,
    required_confirmations: u64,
}

#[derive(Deserialize)]
struct CreateMessage {
    message_id: String,
    source_chain_id: u64,
    destination_chain_id: u64,
    sender: String,
    receiver: String,
}

async fn livez() -> &'static str { "ok" }

async fn readyz() -> impl IntoResponse { (StatusCode::OK, Json(serde_json::json!({"status":"ready"}))) }

async fn chains(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if !authorized(&state, &headers) { return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"unauthorized"}))); }
    (StatusCode::OK, Json(serde_json::json!([
        {"chain_id":31337,"name":"Source Anvil","rpc":"http://localhost:8545","environment":"LOCAL"},
        {"chain_id":31338,"name":"Destination Anvil","rpc":"http://localhost:8546","environment":"LOCAL"}
    ])))
}

async fn list_messages(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if !authorized(&state, &headers) { return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"unauthorized"}))); }
    let messages = state.messages.read().expect("message lock poisoned").values().cloned().collect::<Vec<_>>();
    (StatusCode::OK, Json(serde_json::json!({"items":messages,"total":messages.len()})))
}

async fn get_message(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> impl IntoResponse {
    if !authorized(&state, &headers) { return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"unauthorized"}))); }
    match state.messages.read().expect("message lock poisoned").get(&id).cloned() {
        Some(message) => (StatusCode::OK, Json(serde_json::json!(message))),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"message_not_found"}))),
    }
}

async fn create_message(State(state): State<AppState>, headers: HeaderMap, Json(input): Json<CreateMessage>) -> impl IntoResponse {
    if !authorized(&state, &headers) { return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"unauthorized"}))); }
    if input.message_id.trim().is_empty() || input.source_chain_id == input.destination_chain_id || input.receiver.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"invalid_message"})));
    }
    let mut messages = state.messages.write().expect("message lock poisoned");
    if let Some(existing) = messages.get(&input.message_id) {
        return (StatusCode::OK, Json(serde_json::json!({"message":existing,"replayed":true})));
    }
    let message = Message {
        message_id: input.message_id.clone(), source_chain_id: input.source_chain_id,
        destination_chain_id: input.destination_chain_id, sender: input.sender, receiver: input.receiver,
        status: "source_confirming".into(), reason: "Awaiting configured source confirmations".into(),
        source_tx_hash: None, destination_tx_hash: None, confirmations: 0, required_confirmations: 1,
    };
    messages.insert(input.message_id, message.clone());
    (StatusCode::CREATED, Json(serde_json::json!({"message":message,"replayed":false})))
}

fn authorized(state: &AppState, headers: &HeaderMap) -> bool {
    headers.get("x-api-key").and_then(|v| v.to_str().ok()) == Some(state.api_key.as_str())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(
        env::var("RUST_LOG").unwrap_or_else(|_| "routerpulse_api=info,tower_http=info".into())
    ).init();
    let state = AppState { api_key: env::var("API_KEY").unwrap_or_else(|_| "local-demo-key-change-me".into()), messages: Arc::new(RwLock::new(HashMap::new())) };
    let app = Router::new()
        .route("/livez", get(livez)).route("/readyz", get(readyz))
        .route("/api/v1/chains", get(chains))
        .route("/api/v1/messages", get(list_messages).post(create_message))
        .route("/api/v1/messages/{id}", get(get_message))
        .layer(CorsLayer::permissive()).layer(TraceLayer::new_for_http()).with_state(state);
    let addr = SocketAddr::from(([0,0,0,0], 8080));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind API listener");
    tracing::info!(%addr, "RouterPulse API started");
    axum::serve(listener, app).await.expect("serve API");
}

