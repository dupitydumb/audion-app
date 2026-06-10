use std::path::PathBuf;
use std::time::Duration;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::Credentials;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::presigning::PresigningConfig;
use tracing::info;

#[derive(Clone, Debug)]
pub enum StorageBackend {
    Local {
        data_dir: PathBuf,
    },
    S3 {
        client: Client,
        bucket: String,
        endpoint_url: String,
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
        .credentials_provider(credentials)
        .region(Region::new(region.to_string()));

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
