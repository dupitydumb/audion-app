use std::path::PathBuf;
use std::time::Duration;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::BehaviorVersion;
use aws_sdk_s3::config::Credentials;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::config::retry::RetryConfig;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_config::timeout::TimeoutConfig;
use tracing::info;
use futures::StreamExt;

#[derive(Clone, Debug)]
pub enum StorageBackend {
    Local {
        data_dir: PathBuf,
    },
    S3 {
        client: Client,
        bucket: String,
        // endpoint_url is baked into the AWS SDK client at build time via config_builder.endpoint_url()
    },
    Azure {
        client: azure_storage_blobs::prelude::BlobServiceClient,
        container: String,
    },
}

impl StorageBackend {
    pub async fn put_object(&self, key: &str, data: Vec<u8>, content_type: &str) -> Result<(), String> {
        let key = key.replace("\\", "/");
        match self {
            Self::Local { data_dir } => {
                let full_path = data_dir.join(&key);
                if let Some(parent) = full_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::write(&full_path, data).map_err(|e| e.to_string())?;
                Ok(())
            }
            Self::S3 { client, bucket, .. } => {
                let body = aws_sdk_s3::primitives::ByteStream::from(data);
                client.put_object()
                    .bucket(bucket)
                    .key(&key)
                    .content_type(content_type)
                    .body(body)
                    .send()
                    .await
                    .map_err(|e| format!("S3 put_object error: {:?}", e))?;
                Ok(())
            }
            Self::Azure { client, container } => {
                let container_client = client.container_client(container);
                let blob_client = container_client.blob_client(&key);
                blob_client
                    .put_block_blob(data)
                    .content_type(content_type.to_string())
                    .into_future()
                    .await
                    .map_err(|e| format!("Azure put_object error: {}", e))?;
                Ok(())
            }
        }
    }
 
    pub async fn get_object(&self, key: &str) -> Result<Vec<u8>, String> {
        let key = key.replace("\\", "/");
        match self {
            Self::Local { data_dir } => {
                let full_path = data_dir.join(&key);
                std::fs::read(&full_path).map_err(|e| e.to_string())
            }
            Self::S3 { client, bucket, .. } => {
                let output = client.get_object()
                    .bucket(bucket)
                    .key(&key)
                    .send()
                    .await
                    .map_err(|e| format!("S3 get_object error: {:?}", e))?;
                let bytes = output.body.collect().await
                    .map_err(|e| format!("S3 body stream read error: {:?}", e))?
                    .into_bytes()
                    .to_vec();
                Ok(bytes)
            }
            Self::Azure { client, container } => {
                let container_client = client.container_client(container);
                let blob_client = container_client.blob_client(&key);
                let mut stream = blob_client.get().into_stream();
                let mut all_bytes: Vec<u8> = Vec::new();
                while let Some(chunk) = stream.next().await {
                    let chunk = chunk.map_err(|e| format!("Azure get_object error: {}", e))?;
                    let data = chunk.data.collect().await
                        .map_err(|e| format!("Azure body read error: {}", e))?;
                    all_bytes.extend_from_slice(data.as_ref());
                }
                Ok(all_bytes)
            }
        }
    }

    pub async fn get_presigned_url(&self, key: &str, expires_in_secs: u64) -> Result<String, String> {
        let key = key.replace("\\", "/");
        match self {
            Self::Local { .. } => {
                Err("Presigned URL is not supported for local storage".to_string())
            }
            Self::S3 { client, bucket, .. } => {
                let config = PresigningConfig::expires_in(Duration::from_secs(expires_in_secs))
                    .map_err(|e| format!("Presigning config error: {:?}", e))?;
                let req = client.get_object()
                    .bucket(bucket)
                    .key(&key)
                    .presigned(config)
                    .await
                    .map_err(|e| format!("S3 presign error: {:?}", e))?;
                Ok(req.uri().to_string())
            }
            Self::Azure { .. } => {
                Err("Presigned URL is not supported for Azure storage".to_string())
            }
        }
    }
 
    pub async fn delete_object(&self, key: &str) -> Result<(), String> {
        let key = key.replace("\\", "/");
        match self {
            Self::Local { data_dir } => {
                let full_path = data_dir.join(&key);
                if full_path.exists() {
                    std::fs::remove_file(full_path).map_err(|e| e.to_string())?;
                }
                Ok(())
            }
            Self::S3 { client, bucket, .. } => {
                client.delete_object()
                    .bucket(bucket)
                    .key(&key)
                    .send()
                    .await
                    .map_err(|e| format!("S3 delete_object error: {:?}", e))?;
                Ok(())
            }
            Self::Azure { client, container } => {
                let container_client = client.container_client(container);
                let blob_client = container_client.blob_client(&key);
                blob_client
                    .delete()
                    .into_future()
                    .await
                    .map_err(|e| format!("Azure delete_object error: {}", e))?;
                Ok(())
            }
        }
    }
}

pub fn build_s3_client(
    endpoint: &str,
    bucket: &str,
    access_key: &str,
    secret_key: &str,
    region: &str,
    force_path_style: bool,
) -> Result<Client, String> {
    let region = if region.trim().is_empty() {
        "us-east-1"
    } else {
        region
    };

    let credentials = Credentials::new(
        access_key.trim(),
        secret_key.trim(),
        None,
        None,
        "static-credentials"
    );

    let mut config_builder = aws_sdk_s3::config::Builder::new()
        .behavior_version_latest()
        .credentials_provider(credentials)
        .region(Region::new(region.to_string()))
        .timeout_config(
            TimeoutConfig::builder()
                .connect_timeout(Duration::from_secs(10))
                .read_timeout(Duration::from_secs(12))
                .build()
        );

    if !endpoint.trim().is_empty() {
        config_builder = config_builder.endpoint_url(endpoint.trim());
    }

    if force_path_style {
        config_builder = config_builder.force_path_style(true);
    }

    info!(
        "Building S3 Client: endpoint={}, bucket={}, region={}, force_path_style={}",
        endpoint, bucket, region, force_path_style
    );

    Ok(Client::from_conf(config_builder.build()))
}

/// S3 client for test-connection only: no retries, tight timeouts so we fail
/// fast instead of letting the reverse proxy 502 on us.
pub fn build_s3_client_no_retry(
    endpoint: &str,
    bucket: &str,
    access_key: &str,
    secret_key: &str,
    region: &str,
    force_path_style: bool,
) -> Result<Client, String> {
    let region = if region.trim().is_empty() {
        "us-east-1"
    } else {
        region
    };

    let credentials = Credentials::new(
        access_key.trim(),
        secret_key.trim(),
        None,
        None,
        "static-credentials"
    );

    let mut config_builder = aws_sdk_s3::config::Builder::new()
        .behavior_version_latest()
        .credentials_provider(credentials)
        .region(Region::new(region.to_string()))
        .retry_config(RetryConfig::disabled())
        .timeout_config(
            TimeoutConfig::builder()
                .connect_timeout(Duration::from_secs(8))
                .read_timeout(Duration::from_secs(8))
                .operation_timeout(Duration::from_secs(10))
                .build()
        );

    if !endpoint.trim().is_empty() {
        config_builder = config_builder.endpoint_url(endpoint.trim());
    }

    if force_path_style {
        config_builder = config_builder.force_path_style(true);
    }

    Ok(Client::from_conf(config_builder.build()))
}

pub fn build_azure_client(
    connection_string: &str,
) -> Result<azure_storage_blobs::prelude::BlobServiceClient, String> {
    use azure_storage::ConnectionString;
    use azure_storage_blobs::prelude::ClientBuilder;
    let conn = connection_string.trim();
    let cs = ConnectionString::new(conn)
        .map_err(|e| format!("Azure connection string parse error: {}", e))?;
    let account = cs.account_name
        .ok_or_else(|| "Azure connection string missing AccountName".to_string())?;
    let credentials = cs.storage_credentials()
        .map_err(|e| format!("Azure credentials error: {}", e))?;
    Ok(ClientBuilder::new(account, credentials).blob_service_client())
}

