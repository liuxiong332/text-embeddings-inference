use serde::{Deserialize, Serialize};
use std::env;

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
