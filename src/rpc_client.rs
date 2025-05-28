use anyhow::{anyhow, Result};
use jsonrpsee::{
    core::client::ClientT,
    http_client::{HttpClient, HttpClientBuilder},
    ws_client::{WsClient, WsClientBuilder},
};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use crate::config;

#[derive(Debug)]
pub enum RpcClientType {
    Http(HttpClient),
    WebSocket(WsClient),
}

#[derive(Debug, Clone)]
pub struct RpcClientConfig {
    pub url: String,
    pub timeout: u64,
    pub max_request_size: u32,
    pub max_response_size: u32,
    pub use_websocket: bool,
}

impl Default for RpcClientConfig {
    fn default() -> Self {
        Self {
            url: config::DEFAULT_RPC_URL.to_string(),
            timeout: 30,
            max_request_size: 10 * 1024 * 1024, // 10MB
            max_response_size: 10 * 1024 * 1024, // 10MB
            use_websocket: false,
        }
    }
}

#[derive(Debug)]
pub struct GenericRpcClient {
    client: RpcClientType,
    config: RpcClientConfig,
}

impl GenericRpcClient {
    pub async fn new(config: RpcClientConfig) -> Result<Self> {
        let client = if config.use_websocket {
            Self::create_ws_client(&config).await?
        } else {
            Self::create_http_client(&config)?
        };

        Ok(Self { client, config })
    }

    fn create_http_client(config: &RpcClientConfig) -> Result<RpcClientType> {
        let client = HttpClientBuilder::default()
            .request_timeout(Duration::from_secs(config.timeout))
            .max_request_size(config.max_request_size)
            .max_response_size(config.max_response_size)
            .build(&config.url)
            .map_err(|e| anyhow!("create http client failed: {}", e))?;

        info!("http rpc client created, connected to: {}", config.url);
        Ok(RpcClientType::Http(client))
    }

    async fn create_ws_client(config: &RpcClientConfig) -> Result<RpcClientType> {
        let client = WsClientBuilder::default()
            .request_timeout(Duration::from_secs(config.timeout))
            .max_request_size(config.max_request_size)
            .max_response_size(config.max_response_size)
            .build(&config.url)
            .await
            .map_err(|e| anyhow!("create websocket client failed: {}", e))?;

        info!("websocket rpc client created, connected to: {}", config.url);
        Ok(RpcClientType::WebSocket(client))
    }

    pub async fn call<T>(&self, method: &str) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.call_raw(method, Value::Array(vec![])).await
            .and_then(|value| serde_json::from_value(value).map_err(|e| anyhow!("deserialize failed: {}", e)))
    }

    pub async fn call_with_params<T>(&self, method: &str, params: Value) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.call_raw(method, params).await
            .and_then(|value| serde_json::from_value(value).map_err(|e| anyhow!("deserialize failed: {}", e)))
    }

    pub async fn call_raw(&self, method: &str, params: Value) -> Result<Value> {
        debug!("send raw rpc request: method={}, params={}", method, params);

        let params_array = match params {
            Value::Array(arr) => arr,
            other => vec![other],
        };

        let result = match &self.client {
            RpcClientType::Http(client) => {
                client.request(method, params_array).await
            }
            RpcClientType::WebSocket(client) => {
                client.request(method, params_array).await
            }
        };

        match result {
            Ok(response) => {
                debug!("raw rpc request success: method={}", method);
                Ok(response)
            }
            Err(e) => {
                error!("raw rpc request failed: method={}, error={}", method, e);
                Err(anyhow!("raw rpc request failed: {}", e))
            }
        }
    }

    pub async fn batch_call<T>(&self, requests: Vec<(&str, Value)>) -> Result<Vec<Result<T>>>
    where
        T: for<'de> Deserialize<'de>,
    {
        debug!("send batch rpc request, count: {}", requests.len());

        let mut results = Vec::new();
        
        for (method, params) in requests {
            let result = self.call_raw(method, params).await;
            match result {
                Ok(value) => {
                    match serde_json::from_value::<T>(value) {
                        Ok(typed_result) => results.push(Ok(typed_result)),
                        Err(e) => results.push(Err(anyhow!("deserialize failed: {}", e))),
                    }
                }
                Err(e) => results.push(Err(e)),
            }
        }

        Ok(results)
    }

    pub async fn health_check(&self) -> Result<bool> {
        match self.call_raw("rpc_methods", Value::Array(vec![])).await {
            Ok(_) => {
                info!("rpc client health check success");
                Ok(true)
            }
            Err(e) => {
                warn!("rpc client health check failed: {}", e);
                match self.call_raw("net_version", Value::Array(vec![])).await {
                    Ok(_) => {
                        info!("rpc client health check success");
                        Ok(true)
                    }
                    Err(_) => Ok(false),
                }
            }
        }
    }

    pub fn config(&self) -> &RpcClientConfig {
        &self.config
    }

    pub fn url(&self) -> &str {
        &self.config.url
    }

    pub fn is_websocket(&self) -> bool {
        matches!(self.client, RpcClientType::WebSocket(_))
    }
}

#[derive(Debug, Default)]
pub struct RpcClientBuilder {
    config: RpcClientConfig,
}

impl RpcClientBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn url<S: Into<String>>(mut self, url: S) -> Self {
        self.config.url = url.into();
        self
    }

    pub fn timeout(mut self, timeout: u64) -> Self {
        self.config.timeout = timeout;
        self
    }

    pub fn max_request_size(mut self, size: u32) -> Self {
        self.config.max_request_size = size;
        self
    }

    pub fn max_response_size(mut self, size: u32) -> Self {
        self.config.max_response_size = size;
        self
    }

    pub fn use_websocket(mut self, use_ws: bool) -> Self {
        self.config.use_websocket = use_ws;
        self
    }

    pub async fn build(self) -> Result<GenericRpcClient> {
        GenericRpcClient::new(self.config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rpc_client_builder() {
        let config = RpcClientConfig {
            url: config::DEFAULT_RPC_URL.to_string(),
            timeout: 30,
            max_request_size: 1024 * 1024,
            max_response_size: 1024 * 1024,
            use_websocket: false,
        };

        let builder = RpcClientBuilder::new()
            .url(config::DEFAULT_RPC_URL)
            .timeout(30)
            .max_request_size(1024 * 1024)
            .max_response_size(1024 * 1024)
            .use_websocket(false);

        assert_eq!(builder.config.url, config.url);
        assert_eq!(builder.config.timeout, config.timeout);
    }

    #[test]
    fn test_config_default() {
        let config = RpcClientConfig::default();
        assert_eq!(config.url, config::DEFAULT_RPC_URL);
        assert_eq!(config.timeout, 30);
        assert!(!config.use_websocket);
    }
}
