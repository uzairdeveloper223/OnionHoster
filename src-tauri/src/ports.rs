use serde::Serialize;
use std::{net::Ipv4Addr, time::Duration};
use tokio::{net::TcpStream, time::timeout};

const COMMON_PORTS: [u16; 16] = [
    80, 443, 3000, 3001, 4000, 4200, 5000, 5173, 5174, 8000, 8080, 8081, 8443, 8888, 9000, 9090,
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedPort {
    pub port: u16,
    pub scheme: &'static str,
}

pub fn scheme_for_port(port: u16) -> &'static str {
    if matches!(port, 443 | 8443) {
        "https"
    } else {
        "http"
    }
}

#[tauri::command]
pub async fn detect_local_ports() -> Vec<DetectedPort> {
    let mut tasks = Vec::with_capacity(COMMON_PORTS.len());
    for port in COMMON_PORTS {
        tasks.push(tokio::spawn(async move {
            timeout(
                Duration::from_millis(350),
                TcpStream::connect((Ipv4Addr::LOCALHOST, port)),
            )
            .await
            .ok()
            .and_then(Result::ok)
            .map(|_| port)
        }));
    }

    let mut ports = Vec::new();
    for task in tasks {
        if let Ok(Some(port)) = task.await {
            ports.push(DetectedPort {
                port,
                scheme: scheme_for_port(port),
            });
        }
    }
    ports.sort_by_key(|port| port.port);
    ports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guesses_common_tls_ports() {
        assert_eq!(scheme_for_port(443), "https");
        assert_eq!(scheme_for_port(8443), "https");
        assert_eq!(scheme_for_port(3000), "http");
    }
}
