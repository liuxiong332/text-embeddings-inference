use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_sdk_s3::config::Region;
use futures::future::BoxFuture;
use futures::FutureExt;
use std::fs::File;
use std::io::Write;

#[derive(Debug)]
pub struct S3Err {
    pub message: String,
}

pub struct S3Downloader {
    client: aws_sdk_s3::Client,
    bucket: String,
    root_prefix: String,
    sub_prefix: String,
}

impl S3Downloader {
    pub async fn new(
        access_key: String,
        secret_key: String,
        region: String,
        bucket: String,
        root_prefix: String,
        sub_prefix: String,
    ) -> Self {
        let mut root_prefix = root_prefix;
        if !root_prefix.ends_with('/') {
            root_prefix.push('/');
        }
        // let prefix = format!("{}{}", root_prefix, sub_prefix);
        let shared_config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(region))
            .credentials_provider(Credentials::new(
                access_key, secret_key, None, None, "Static",
            ))
            .load()
            .await;

        let client = aws_sdk_s3::Client::new(&shared_config);

        S3Downloader {
            client,
            bucket,
            root_prefix,
            sub_prefix,
        }
    }

    async fn download_file(self: &S3Downloader, key: String) -> Result<(), S3Err> {
        let get_resp = match self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(err) => {
                return Err(S3Err {
                    message: format!("Error getting object: {}", err),
                })
            }
        };

        let sub_path = key.replace(&self.root_prefix, "");
        let path = format!("/tmp/{}", sub_path);

        // Create the directory recursively if it doesn't exist
        let parent = match std::path::Path::new(&path).parent() {
            Some(parent) => parent,
            None => {
                return Err(S3Err {
                    message: format!("Error getting parent path: {}", path),
                })
            }
        };
        match std::fs::create_dir_all(parent) {
            Ok(_) => {}
            Err(err) => {
                return Err(S3Err {
                    message: format!("Error creating parent path: {}", err),
                })
            }
        }

        let mut file = match File::create(&path) {
            Ok(file) => file,
            Err(err) => {
                return Err(S3Err {
                    message: format!("Error creating file: {}", err),
                })
            }
        };

        let data = match get_resp.body.collect().await.map(|data| data.into_bytes()) {
            Ok(data) => data,
            Err(err) => {
                return Err(S3Err {
                    message: format!("Error collecting body: {}", err),
                })
            }
        };
        match file.write_all(&data) {
            Ok(_) => {}
            Err(err) => {
                return Err(S3Err {
                    message: format!("Error writing file: {}", err),
                })
            }
        }
        print!("Download file {} to {}\n", key, path);
        Ok(())
    }

    fn walk_s3_path<'a>(
        self: &'a S3Downloader,
        prefix: String,
    ) -> BoxFuture<'a, Result<(), S3Err>> {
        async {
            let resp = match self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(prefix)
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(err) => {
                    return Err(S3Err {
                        message: format!("Error listing objects: {}", err),
                    })
                }
            };

            if let Some(objects) = resp.contents {
                for object in objects {
                    if let Some(obj_key) = object.key {
                        if obj_key.ends_with("/") {
                            self.walk_s3_path(obj_key).await?;
                        } else {
                            self.download_file(obj_key).await?;
                        }
                    }
                }
            }

            Ok(())
        }
        .boxed()
    }

    pub async fn download(self: &S3Downloader) -> Result<(), S3Err> {
        let prefix = format!("{}{}", self.root_prefix, self.sub_prefix);
        self.walk_s3_path(prefix).await
    }
}

mod tests {
    use super::*;

    #[tokio::test]
    async fn test_s3_download() {
        let result = S3Downloader::new(
            "xx".to_string(),
            "xx".to_string(),
            "cn-north-1".to_string(),
            "pre-web-static".to_string(),
            "huggingface".to_string(),
            "intfloat/multilingual-e5-large/9f78368af0062735ba99812349c562316e29f719".to_string(),
        )
        .await
        .download()
        .await;

        match result {
            Ok(_) => {
                println!("download success");
            }
            Err(err) => {
                println!("download error: {:?}", err);
            }
        }
    }
}
