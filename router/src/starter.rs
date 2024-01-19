use std::collections::HashMap;

use crate::consulr::{ConsulClient, ConsulError, VaultClient};
use crate::s3_download::S3Downloader;

pub(crate) async fn start_app(
    model_id: String,
    mut model_revision: String,
) -> Result<String, ConsulError> {
    let consul_client = ConsulClient::new();

    // Get kv from consul
    let mut consul_kv = match consul_client.kv().await {
        Ok(kv) => kv,
        Err(err) => {
            tracing::warn!("Could not get kv from consul: {:?}", err);
            HashMap::new()
        }
    };

    // Get vault instant
    match consul_client.get_service("vault".to_string()).await {
        Ok(vault) => {
            if vault.len() > 0 {
                let vault_split: Vec<&str> = vault[0].split(":").collect();
                let vault_host = vault_split[0].to_string();
                let vault_port = vault_split[1].parse::<u16>().unwrap();

                let vault_client = VaultClient::new(vault_host, vault_port);
                let vault_secret = match vault_client.secret().await {
                    Ok(secret) => secret,
                    Err(err) => {
                        tracing::warn!("Could not get secret from vault: {:?}", err);
                        HashMap::new()
                    }
                };
                // Set env vars
                for (key, value) in vault_secret {
                    consul_kv.insert(key, value);
                }
            }
        }
        Err(err) => {
            tracing::warn!("Could not get vault from consul: {:?}", err);
        }
    }

    let access_key = consul_kv
        .get("accessKeyID")
        .unwrap_or(&"".to_string())
        .to_string();
    let secret_key = consul_kv
        .get("secretAccessKey")
        .unwrap_or(&"".to_string())
        .to_string();
    let region = consul_kv
        .get("region")
        .unwrap_or(&"".to_string())
        .to_string();
    let bucket = consul_kv
        .get("bucket")
        .unwrap_or(&"".to_string())
        .to_string();
    if model_revision == "" {
        model_revision = "main".to_owned();
    }
    let sub_prefix = format!("{}/{}", model_id, model_revision);
    match S3Downloader::new(
        access_key,
        secret_key,
        region,
        bucket,
        "huggingface".to_string(),
        sub_prefix.clone(),
    )
    .await
    .download()
    .await
    {
        Ok(_) => {
            tracing::info!("Downloaded model from S3");
        }
        Err(err) => {
            tracing::warn!("Could not download model from S3: {:?}", err);
        }
    }

    consul_client.register().await;
    Ok(format!("/tmp/{}", sub_prefix))
}
