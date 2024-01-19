use aws_smithy_types::base64;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env};

pub struct ConsulClient {
    consul_host: String,
    consul_port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct AgentServiceCheck {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Interval")]
    interval: String,
    #[serde(rename = "Timeout")]
    timeout: String,
    #[serde(rename = "TTL")]
    ttl: String,
    #[serde(rename = "DeregisterCriticalServiceAfter")]
    deregister_critical_service_after: String,
    #[serde(rename = "HTTP")]
    http: Option<String>,
    #[serde(rename = "TCP")]
    tcp: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConsulKV {
    #[serde(rename = "Key")]
    key: String,
    #[serde(rename = "Value")]
    value: String,
}

#[derive(Debug)]
pub struct ConsulError {
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServiceEntry {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "Service")]
    service: String,
    #[serde(rename = "Tags")]
    tags: Vec<String>,
    #[serde(rename = "Address")]
    address: String,
    #[serde(rename = "Port")]
    port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct HealthService {
    #[serde(rename = "Service")]
    service: ServiceEntry,
}

impl ConsulClient {
    pub fn new() -> ConsulClient {
        let consul_addr = env::var("APP_CONSUL_ADDRESS").unwrap_or("127.0.0.1:8500".to_string());
        let (host, port) = consul_addr
            .split_once(":")
            .expect("Invalid CONSUL_ADDRESS format");
        let port_int: i16 = port.parse().expect("Invalid port");

        ConsulClient {
            consul_host: host.to_string(),
            consul_port: port_int as u16,
        }
    }

    fn gen_check(&self) -> AgentServiceCheck {
        let pod_ip = env::var("POD_IP").unwrap_or("127.0.0.1".to_string());
        let server_name: String =
            env::var("SERVER_NAME").unwrap_or("text-embeddings-inference-server".to_string());
        let port = env::var("PORT").unwrap_or("8080".to_string());
        let service_id = format!("{}_{}", server_name, pod_ip);

        return AgentServiceCheck {
            id: service_id,
            name: server_name.to_string(),
            interval: "5s".to_string(),
            timeout: "5s".to_string(),
            ttl: "10s".to_string(),
            deregister_critical_service_after: "20s".to_string(),
            http: None,
            tcp: Some(format!("{}:{}", pod_ip, port)),
        };
    }

    pub async fn register(&self) {
        let check = self.gen_check();
        let req_url = format!(
            "https://{}.{}/v1/agent/check/register",
            self.consul_host, self.consul_port
        );
        let resp = reqwest::Client::new()
            .put(&req_url)
            .json(&check)
            .send()
            .await;
        match resp {
            Ok(_) => {
                println!("Consul registration {} successfully!", check.id);
            }
            Err(e) => {
                println!("Consul registration error: {:?}", e);
            }
        }
    }

    pub async fn kv(&self) -> Result<HashMap<String, String>, ConsulError> {
        // http://10.70.2.40:8500/v1/kv/config/ai-studio/?recurse=true
        let server_name =
            env::var("SERVER_NAME").unwrap_or("text-embeddings-inference-server".to_string());
        let req_url = format!(
            "https://{}.{}/v1/kv/config/{}?recurse=true",
            self.consul_host, self.consul_port, server_name
        );
        let resp = match reqwest::Client::new().get(&req_url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                return Err(ConsulError {
                    message: format!("Error getting object: {}", e),
                })
            }
        };

        match resp.json::<Vec<ConsulKV>>().await {
            Ok(kvs) => {
                let mut map = HashMap::new();
                for kv in kvs {
                    let key = kv.key.replace(&format!("config/{}/", server_name), "");
                    let byte_val = match base64::decode(kv.value) {
                        Ok(value) => value,
                        Err(e) => {
                            return Err(ConsulError {
                                message: format!("Error decoding base64: {}", e),
                            });
                        }
                    };
                    let value = match String::from_utf8(byte_val) {
                        Ok(value) => value,
                        Err(e) => {
                            return Err(ConsulError {
                                message: format!("Error decoding utf8: {}", e),
                            });
                        }
                    };
                    map.insert(key, value);
                }
                Ok(map)
            }
            Err(e) => Err(ConsulError {
                message: format!("Error parsing json: {}", e),
            }),
        }
    }

    pub async fn get_service(&self, service_name: String) -> Result<Vec<String>, ConsulError> {
        // http://10.70.2.40:8500/v1/health/checks/vault
        let req_url = format!(
            "https://{}.{}/v1/health/checks/{}",
            self.consul_host, self.consul_port, service_name
        );
        let resp = match reqwest::Client::new().get(&req_url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                return Err(ConsulError {
                    message: format!("Error getting object: {}", e),
                });
            }
        };

        match resp.json::<Vec<HealthService>>().await {
            Ok(services) => {
                let mut addrs = Vec::new();
                for service in services {
                    let addr = format!("{}:{}", service.service.address, service.service.port);
                    addrs.push(addr);
                }
                Ok(addrs)
            }
            Err(e) => Err(ConsulError {
                message: format!("Error parsing json: {}", e),
            }),
        }
    }
}

pub struct VaultClient {
    vault_host: String,
    vault_port: u16,
    token: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct VaultSecretRes {
    data: HashMap<String, String>,
}

impl VaultClient {
    pub fn new(host: String, port: u16) -> VaultClient {
        VaultClient {
            vault_host: host,
            vault_port: port,
            token: env::var("APP_VAULT_TOKEN").unwrap_or("".to_string()),
        }
    }

    pub async fn secret(&self) -> Result<HashMap<String, String>, ConsulError> {
        // http://10.70.2.40:8200/v1/secret/ai-studio
        let server_name =
            env::var("SERVER_NAME").unwrap_or("text-embeddings-inference-server".to_string());
        let req_url = format!(
            "https://{}.{}/v1/secret/{}",
            self.vault_host, self.vault_port, server_name
        );
        let mut headers = reqwest::header::HeaderMap::new();
        let token_h = match reqwest::header::HeaderValue::from_str(&self.token) {
            Ok(h) => h,
            Err(e) => {
                return Err(ConsulError {
                    message: format!("Error parsing token: {}", e),
                })
            }
        };
        headers.append("", token_h);
        let resp = match reqwest::Client::new()
            .get(&req_url)
            .headers(headers)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                return Err(ConsulError {
                    message: format!("Error getting object: {}", e),
                })
            }
        };

        // Get data from resp json
        match resp.json::<VaultSecretRes>().await {
            Ok(res) => Ok(res.data),
            Err(e) => Err(ConsulError {
                message: format!("Error parsing json: {}", e),
            }),
        }
    }
}

mod tests {
    use super::ConsulClient;
    use std::env;

    #[test]
    fn test_gen_check() {
        env::set_var("APP_CONSUL_ADDRESS", "192.168.40.73:8500");

        let client = ConsulClient::new();
        let check = client.gen_check();
        assert_eq!(check.id, "text-embeddings-inference-server_127.0.0.1");
        assert_eq!(check.name, "text-embeddings-inference-server");
        assert_eq!(check.interval, "5s");
        assert_eq!(check.timeout, "5s");
        assert_eq!(check.ttl, "10s");
        assert_eq!(check.deregister_critical_service_after, "20s");
        assert_eq!(check.http, None);
        assert_eq!(check.tcp, Some("127.0.0.1:8080".to_string()));
        let body = serde_json::to_string(&check).unwrap();
        print!("{}", body);
    }
}
