use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::Json,
    routing::{get},
    Router,
};
use serde::{Deserialize, Serialize};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use std::env;

use rpc_client::RpcClientBuilder;
use serde_json::json;

mod subxt_client;
mod config;
mod rpc_client;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        .route("/api/nucleus", get(get_nucleus_list))
        .route("/api/nucleus/{id}", get(get_nucleus_by_id))
        .route("/api/nucleus/{id}/abi", get(get_nucleus_abi_by_id))
        .route("/api/node/detail", get(get_node_detail))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    let port = env::var("PORT").unwrap_or_else(|_| "4001".to_string());
    let addr = format!("0.0.0.0:{}", port);
    
    tracing::info!("🚀 Starting server...");
    tracing::info!("📡 Trying to bind to address: {}", addr);
    
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => {
            tracing::info!("✅ Server started at http://{}", addr);
            listener
        }
        Err(e) => {
            tracing::error!("❌ Failed to bind to address {}: {}", addr, e);
            tracing::error!("💡 Tip: Port may be in use, please try a different port");
            std::process::exit(1);
        }
    };
    
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> &'static str {
    "Nucleus Dashboard API v0.1.0"
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "service": "nucleus-dashboard-api"
    }))
}

#[derive(Serialize, Deserialize)]
struct NucleusQuery {
    limit: Option<u32>,
    offset: Option<u32>,
}

async fn get_node_detail() -> Result<Json<serde_json::Value>, StatusCode> {
    let rpc_url = env::var("NUCLEUS_RPC_URL").unwrap_or_else(|_| config::DEFAULT_RPC_URL.to_string());
    
    match subxt_client::NucleusClient::new(&rpc_url).await {
        Ok(client) => {
            match client.get_node_detail().await {
                Ok(detailed_info) => {
                    Ok(Json(json!({
                        "data": detailed_info,
                    })))
                }
                Err(e) => {
                    tracing::error!("Get Detailed Node Info Failed: {:?}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Err(e) => {
            tracing::error!("Create Nucleus Client Failed: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_nucleus_list(Query(params): Query<NucleusQuery>) -> Result<Json<serde_json::Value>, StatusCode> {
    let rpc_url = env::var("NUCLEUS_RPC_URL").unwrap_or_else(|_| config::DEFAULT_RPC_URL.to_string());
    let limit = params.limit;
    let offset = params.offset;
    
    match subxt_client::NucleusClient::new(&rpc_url).await {
        Ok(client) => {
            match client.get_nucleus_list(limit, offset).await {
                Ok(nucleus_list) => {
                    let count = nucleus_list.len();
                    Ok(Json(json!({
                        "data": nucleus_list,
                        "count": count,
                        "limit": limit.unwrap_or(10),
                        "offset": offset.unwrap_or(0)
                    })))
                }
                Err(e) => {
                    tracing::error!("Get Nucleus List Failed: {:?}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Err(e) => {
            tracing::error!("Create Nucleus Client Failed: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_nucleus_by_id(Path(id): Path<String>) -> Result<Json<serde_json::Value>, StatusCode> {
    let rpc_url = env::var("NUCLEUS_RPC_URL").unwrap_or_else(|_| config::DEFAULT_RPC_URL.to_string());
    
    match subxt_client::NucleusClient::new(&rpc_url).await {
        Ok(client) => {
            match client.get_nucleus_by_id(&id).await {
                Ok(Some(nucleus)) => {
                    Ok(Json(json!({
                        "data": nucleus
                    })))
                }
                Ok(None) => {
                    Err(StatusCode::NOT_FOUND)
                }
                Err(e) => {
                    tracing::error!("Get Nucleus Info Failed: {:?}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Err(e) => {
            tracing::error!("Create Nucleus Client Failed: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_nucleus_abi_by_id(Path(id): Path<String>) -> Result<Json<serde_json::Value>, StatusCode> {
    let rpc_url = env::var("NUCLEUS_RPC_URL").unwrap_or_else(|_| config::DEFAULT_RPC_URL.to_string());

    let host = rpc_url.split("://").nth(1).unwrap();
    let host = host.split(":").nth(0).unwrap();
    let rpc_url = format!("http://{}:9955/{}", host, id);

    tracing::info!("use rpc url for demo: {}", rpc_url);

    let client = match RpcClientBuilder::new()
        .url(&rpc_url)
        .timeout(30)
        .max_request_size(1024 * 1024)
        .max_response_size(1024 * 1024)
        .use_websocket(false)
        .build()
        .await
    {
        Ok(client) => client,
        Err(e) => {
            tracing::error!("create rpc client failed: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let health_status = client.health_check().await.unwrap_or(false);

    println!("health_status: {:?}", health_status);

    let result = match client.call_raw("abi", json!([ ])).await {
        Ok(result) => Some(result),
        Err(e) => {
            tracing::warn!("get abi failed: {}", e);
            None
        }
    };

    Ok(Json(json!({
        "status": "success",
        "data": result
    })))
}