use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use subxt::{OnlineClient, PolkadotConfig};
use hex;
use parity_scale_codec::{Decode, Encode};
use sp_core::crypto::Ss58Codec;

use crate::config;

#[subxt::subxt(runtime_metadata_path = "artifacts/polkadot_metadata_full.scale")]
pub mod polkadot {}

#[derive(Debug, Serialize, Deserialize)]
pub struct NucleusInfo {
    pub id: String,
    pub name: String,
    pub manager: String,
    pub wasm_hash: String,
    pub wasm_version: u32,
    pub wasm_location: String,
    pub energy: u64,
    pub current_event: u64,
    pub root_state: String,
    pub capacity: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountInfo {
    pub address: String,
    pub balance: String,
    pub nonce: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockInfo {
    pub number: u32,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    pub spec_name: String,
    pub spec_version: String,
    pub best_block: u32,
    pub finalized_block: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkStats {
    pub total_accounts: u64,
    pub total_nucleus: u64,
    pub total_validators: u64,
    pub active_validators: u64,
    pub total_issuance: String,
    pub staking_ratio: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeDetail {
    pub node_info: NodeInfo,
    pub network_stats: NetworkStats,
    pub endpoint: String,
}

pub struct NucleusClient {
    client: OnlineClient<PolkadotConfig>,
    endpoint: String,
}

impl NucleusClient {
    pub async fn new(endpoint: &str) -> Result<Self> {
        let client = OnlineClient::<PolkadotConfig>::from_url(endpoint).await?;
        
        Ok(Self { 
            client,
            endpoint: endpoint.to_string(),
        })
    }

    pub async fn get_account_info(&self, account_id: &str) -> Result<AccountInfo> {
        let account = subxt::utils::AccountId32::from_str(account_id)?;
        
        let account_info_query = polkadot::storage().system().account(&account);
        let account_info = self.client
            .storage()
            .at_latest()
            .await?
            .fetch(&account_info_query)
            .await?;

        if let Some(info) = account_info {
            Ok(AccountInfo {
                address: account_id.to_string(),
                balance: info.data.free.to_string(),
                nonce: info.nonce,
            })
        } else {
            Ok(AccountInfo {
                address: account_id.to_string(),
                balance: "0".to_string(),
                nonce: 0,
            })
        }
    }

    pub async fn get_latest_block(&self) -> Result<BlockInfo> {
        let block = self.client.blocks().at_latest().await?;

        Ok(BlockInfo {
            number: block.number(),
            hash: format!("{:?}", block.hash()),
            parent_hash: format!("{:?}", block.header().parent_hash),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        })
    }

    pub async fn get_nucleus_list(&self, limit: Option<u32>, offset: Option<u32>) -> Result<Vec<NucleusInfo>> {
        let mut nucleus_list = Vec::new();
        let limit = limit.unwrap_or(10) as usize;
        let offset = offset.unwrap_or(0) as usize;
        let mut current_index = 0;

        let storage = self.client.storage().at_latest().await?;
        
        let query = polkadot::storage().nucleus().nuclei_iter();
        let mut iter = storage.iter(query).await?;
        
        while let Some(Ok(kv)) = iter.next().await {
            if current_index < offset {
                current_index += 1;
                continue;
            }
            
            if nucleus_list.len() >= limit {
                break;
            }
            
            let nucleus_data = &kv.value;
            
            let nucleus_id = if kv.key_bytes.len() >= 32 {
                let id_bytes: [u8; 32] = kv.key_bytes[kv.key_bytes.len()-32..].try_into().unwrap_or([0u8; 32]);
                let id = sp_core::crypto::AccountId32::from(id_bytes);
                id.to_ss58check_with_version(sp_core::crypto::Ss58AddressFormat::custom(config::VERISENSE_PREFIX))
            } else {
                format!("{:?}", kv.key_bytes)
            };
            
            nucleus_list.push(NucleusInfo {
                id: nucleus_id,
                name: String::from_utf8(nucleus_data.name.clone()).unwrap_or_else(|_| 
                    format!("Invalid UTF-8: {:?}", nucleus_data.name)
                ),
                manager: {
                    let account_bytes: [u8; 32] = nucleus_data.manager.0;
                    let account_id = sp_core::crypto::AccountId32::from(account_bytes);
                    account_id.to_ss58check_with_version(sp_core::crypto::Ss58AddressFormat::custom(config::VERISENSE_PREFIX))
                },
                wasm_hash: format!("0x{}", hex::encode(&nucleus_data.wasm_hash.0)),
                wasm_version: nucleus_data.wasm_version,
                wasm_location: nucleus_data.wasm_location
                    .as_ref()
                    .map(|loc| format!("{}", hex::encode(&loc.0)))
                    .unwrap_or_else(|| "".to_string()),
                energy: nucleus_data.energy as u64,
                current_event: nucleus_data.current_event as u64,
                root_state: format!("0x{}", hex::encode(&nucleus_data.root_state.0)),
                capacity: nucleus_data.capacity as u32,
            });
            
            current_index += 1;
        }

        Ok(nucleus_list)
    }

    pub async fn get_nucleus_by_id(&self, nucleus_id: &str) -> Result<Option<NucleusInfo>> {
        let storage = self.client.storage().at_latest().await?;
        let account = subxt::utils::AccountId32::from_str(nucleus_id)?;
        
        let nucleus_query = polkadot::storage().nucleus().nuclei(&account);
        let nucleus_data = storage.fetch(&nucleus_query).await?;
        
        if let Some(data) = nucleus_data {
            Ok(Some(NucleusInfo {
                id: nucleus_id.to_string(),
                name: String::from_utf8(data.name.clone()).unwrap_or_else(|_| 
                    format!("Invalid UTF-8: {:?}", data.name)
                ),
                manager: {
                    let account_bytes: [u8; 32] = data.manager.0;
                    let account_id = sp_core::crypto::AccountId32::from(account_bytes);
                    account_id.to_ss58check_with_version(sp_core::crypto::Ss58AddressFormat::custom(config::VERISENSE_PREFIX))
                },
                wasm_hash: format!("0x{}", hex::encode(&data.wasm_hash.0)),
                wasm_version: data.wasm_version,
                wasm_location: data.wasm_location
                    .as_ref()
                    .map(|loc| format!("{}", hex::encode(&loc.0)))
                    .unwrap_or_else(|| "".to_string()),
                energy: data.energy as u64,
                current_event: data.current_event as u64,
                root_state: format!("0x{}", hex::encode(&data.root_state.0)),
                capacity: data.capacity as u32
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_node_detail(&self) -> Result<NodeDetail> {
        let node_info: NodeInfo = self.get_node_info().await?;
        let network_stats = self.get_network_stats().await?;
        
        Ok(NodeDetail {
            node_info,
            network_stats,
            endpoint: self.endpoint.clone(),
        })
    }

    pub async fn get_node_info(&self) -> Result<NodeInfo> {
        let constants = self.client.constants();
        let latest_block = self.client.blocks().at_latest().await?;
        let finalized_block = self.client.blocks().at_latest().await?;
        
        let version_addr = polkadot::constants().system().version();
        let version_value = constants.at(&version_addr);
        let version_result = version_value.as_ref().unwrap();
        let spec_name = version_result.spec_name.to_string();
        let spec_version = version_result.spec_version.to_string();
        
        Ok(NodeInfo {
            spec_name,
            spec_version,
            best_block: latest_block.number(),
            finalized_block: finalized_block.number(),
        })
    }

    pub async fn get_network_stats(&self) -> Result<NetworkStats> {
        let total_nucleus = self.count_total_nucleus().await?;
        let (total_validators, active_validators) = self.get_validator_counts().await?;
        
        let total_issuance = self.get_total_issuance().await?;
        
        let total_accounts = self.count_total_accounts().await?;
        
        let staking_ratio = self.calculate_staking_ratio().await?;
        
        Ok(NetworkStats {
            total_accounts,
            total_nucleus,
            total_validators,
            active_validators,
            total_issuance,
            staking_ratio,
        })
    }

    pub async fn count_total_nucleus(&self) -> Result<u64> {
        let storage = self.client.storage().at_latest().await?;
        let query = polkadot::storage().nucleus().nuclei_iter();
        let mut iter = storage.iter(query).await?;
        
        let mut count = 0u64;
        while let Some(Ok(_)) = iter.next().await {
            count += 1;
        }
        
        Ok(count)
    }

    pub async fn get_validator_counts(&self) -> Result<(u64, u64)> {
        let storage = self.client.storage().at_latest().await?;
        
        let validators_query = polkadot::storage().session().validators();
        let validators = storage.fetch(&validators_query).await?;
        
        let total_validators = if let Some(validator_list) = validators {
            validator_list.len() as u64
        } else {
            0
        };
        
        let active_validators = total_validators;
        
        Ok((total_validators, active_validators))
    }

    pub async fn get_total_issuance(&self) -> Result<String> {
        let storage = self.client.storage().at_latest().await?;
        
        let total_issuance_query = polkadot::storage().balances().total_issuance();
        let total_issuance = storage.fetch(&total_issuance_query).await?;
        
        if let Some(issuance) = total_issuance {
            Ok(issuance.to_string())
        } else {
            Ok("0".to_string())
        }
    }

    pub async fn count_total_accounts(&self) -> Result<u64> {
        let storage = self.client.storage().at_latest().await?;
        
        let accounts_query = polkadot::storage().system().account_iter();
        let mut iter = storage.iter(accounts_query).await?;
        
        let mut count = 0u64;
        while let Some(Ok(_)) = iter.next().await {
            count += 1;
            if count >= 10000 {
                break;
            }
        }
        
        Ok(count)
    }

    pub async fn calculate_staking_ratio(&self) -> Result<f64> {
        let storage = self.client.storage().at_latest().await?;
        
        let total_issuance_query = polkadot::storage().balances().total_issuance();
        let total_issuance = storage.fetch(&total_issuance_query).await?;
        
        if let Some(_total) = total_issuance {
            Ok(0.0)
        } else {
            Ok(0.0)
        }
    }

    pub async fn subscribe_new_blocks(&self) -> Result<()> {
        let mut blocks_sub = self.client.blocks().subscribe_finalized().await?;

        while let Some(block) = blocks_sub.next().await {
            match block {
                Ok(block) => {
                    tracing::info!("New Block: #{} - {:?}", block.number(), block.hash());
                }
                Err(e) => {
                    tracing::error!("Subscribe Block Error: {:?}", e);
                    break;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const TEST_RPC_LINK: &str = "wss://rpc.beta.verisense.network";

    #[tokio::test]
    async fn test_nucleus_client() {
        let client = NucleusClient::new(TEST_RPC_LINK)
            .await
            .expect("Failed to create Nucleus client");

        let node_info = client.get_node_detail().await;
        println!("node_info: {:?}", node_info);
        assert!(node_info.is_ok());
    }

    #[tokio::test]
    async fn test_get_nucleus_list() {
        let client = NucleusClient::new(TEST_RPC_LINK)
            .await
            .expect("Failed to create Nucleus client");

        let nucleus_list = client.get_nucleus_list(Some(5), Some(0)).await;
        println!("nucleus_list: {:#?}", nucleus_list);
        assert!(nucleus_list.is_ok());

        let list = nucleus_list.unwrap();
        assert!(list.len() <= 5);
    }

    #[tokio::test]
    async fn test_get_node_detail() {
        let client = NucleusClient::new(TEST_RPC_LINK)
            .await
            .expect("Failed to create Nucleus client");

        let detailed_info = client.get_node_detail().await;
        println!("detailed_node_info: {:#?}", detailed_info);
        assert!(detailed_info.is_ok());

        let info = detailed_info.unwrap();
        assert!(!info.node_info.spec_name.is_empty());
        assert!(info.network_stats.total_nucleus >= 0);
        assert!(info.network_stats.total_validators >= 0);
    }

}
