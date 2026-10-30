use crate::state;
use axum::response::sse::{Event, Sse};
use axum::{Router, response::IntoResponse, routing::get};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_stream::{StreamExt, wrappers::IntervalStream};

pub fn create_router(state: Arc<RwLock<state::RWatchState>>) -> Router {
    Router::new()
        .route("/api/processes", get(get_processes))
        .route("/api/processes/{pid}", get(get_process))
        .route("/api/system", get(get_system))
        .route("/api/network", get(get_network))
        .route("/api/cgroups", get(get_cgroups))
        .route("/api/events", get(events))
        .with_state(state)
}

pub async fn run_server(state: Arc<RwLock<state::RWatchState>>) {
    let app = create_router(state);

let listener = match tokio::net::TcpListener::bind("0.0.0.0:3000").await {
    Ok(listener) => listener,
    Err(error) => {
        tracing::error!(%error, "failed to bind HTTP API server");
        return;
    }
};

match listener.local_addr() {
    Ok(address) => {
        tracing::info!(%address, "HTTP API server listening");
    }
    Err(error) => {
        tracing::warn!(%error, "could not determine HTTP API server address");
    }
}

if let Err(error) = axum::serve(listener, app).await {
    tracing::error!(%error, "HTTP API server stopped with an error");
}

}


async fn get_processes(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<state::RWatchState>>>,
) -> axum::Json<serde_json::Value> {
    let state = state.read().await;

    axum::Json(serde_json::json!({
        "processes": state.processes,
    }))
}
async fn get_process(
    axum::extract::Path(pid): axum::extract::Path<u32>,
    axum::extract::State(state): axum::extract::State<Arc<RwLock<state::RWatchState>>>,
) -> axum::response::Response {
    let state = state.read().await;

    if let Some(process) = state
        .processes
        .as_ref()
        .and_then(|processes| processes.iter().find(|process| process.pid == pid))
    {
        axum::Json(process).into_response()
    } else {
        (
            axum::http::StatusCode::NOT_FOUND,
            format!("Process {} not found", pid),
        )
            .into_response()
    }
}
async fn get_system(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<state::RWatchState>>>,
) -> axum::Json<serde_json::Value> {
    let state = state.read().await;

    axum::Json(serde_json::json!({
        "cpu": state.cpu,
        "memory": state.memory,
        "disk": state.disk,
        "network": state.network,
        "cgroups": state.cgroups,
    }))
}

async fn get_network(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<state::RWatchState>>>,
) -> axum::Json<serde_json::Value> {
    let state = state.read().await;

    axum::Json(serde_json::json!({
        "network": state.network,
    }))
}

async fn get_cgroups(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<state::RWatchState>>>,
) -> axum::Json<serde_json::Value> {
    let state = state.read().await;

    axum::Json(serde_json::json!({
        "cgroups": state.cgroups,
    }))
}
async fn events(
    axum::extract::State(state): axum::extract::State<Arc<RwLock<state::RWatchState>>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let interval = tokio::time::interval(std::time::Duration::from_secs(1));

    let stream = IntervalStream::new(interval).then(move |_| {
        let state = state.clone();

        async move {
            let state = state.read().await;

            let data = serde_json::json!({
                "type": "snapshot",
                "data": {
                    "cpu": state.cpu,
                    "memory": state.memory,
                    "disk": state.disk,
                    "network": state.network,
                    "processes": state.processes,
                    "cgroups": state.cgroups,
                }
            });

            Ok(Event::default().event("snapshot").json_data(data).unwrap())
        }
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    async fn start_test_server(state: Arc<RwLock<state::RWatchState>>) -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = create_router(state);
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        addr
    }

    async fn http_get(addr: std::net::SocketAddr, path: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            path
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();

        let status_code = response
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .unwrap_or(0);

        let body = response.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        (status_code, body)
    }

    fn new_test_state() -> Arc<RwLock<state::RWatchState>> {
        Arc::new(RwLock::new(state::RWatchState {
            cpu: None,
            memory: None,
            disk: None,
            network: None,
            processes: None,
            cgroups: None,
            tcp: None,
            tcp_connections: None,
            udp: None,
        }))
    }

    #[tokio::test]
    async fn test_get_processes_empty() {
        let state = new_test_state();
        let addr = start_test_server(state).await;

        let (status, body) = http_get(addr, "/api/processes").await;
        assert_eq!(status, 200);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(parsed["processes"].is_null());
    }

    #[tokio::test]
    async fn test_get_process_found_and_not_found() {
        let state = new_test_state();
        let test_process = crate::process::ProcessInfo {
            pid: 42,
            name: "test_process".to_string(),
            start_time_ticks: 100,
            state: "S".to_string(),
            parent_pid: 1,
            memory_kb: 1024,
            threads: 1,
            utime: 10,
            stime: 20,
            cpu_usage: 5.0,
            namespaces: crate::namespace::ProcessNamespaces {
                pid: Ok(1),
                net: Ok(2),
                mnt: Ok(3),
                user: Ok(4),
            },
            limits: crate::limits::ResourceLimits {
                max_open_files: (Some(1024), Some(1024)),
                max_processes: (Some(500), Some(500)),
                max_locked_memory: (None, None),
                max_address_space: (None, None),
            },
        };
        state.write().await.processes = Some(vec![test_process]);
        let addr = start_test_server(state).await;

        // Test found
        let (status, body) = http_get(addr, "/api/processes/42").await;
        assert_eq!(status, 200);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["pid"], 42);
        assert_eq!(parsed["name"], "test_process");

        // Test not found
        let (status_404, body_404) = http_get(addr, "/api/processes/999").await;
        assert_eq!(status_404, 404);
        assert!(body_404.contains("Process 999 not found"));
    }

    #[tokio::test]
    async fn test_get_system_endpoint() {
        let state = new_test_state();
        state.write().await.cpu = Some(15.5);
        let addr = start_test_server(state).await;

        let (status, body) = http_get(addr, "/api/system").await;
        assert_eq!(status, 200);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["cpu"], 15.5);
        assert!(parsed.get("memory").is_some());
        assert!(parsed.get("disk").is_some());
        assert!(parsed.get("network").is_some());
        assert!(parsed.get("cgroups").is_some());
    }

    #[tokio::test]
    async fn test_get_network_endpoint() {
        let state = new_test_state();
        state.write().await.network = Some(vec![crate::network::NetworkRate {
            interface: "eth0".to_string(),
            download_rate: 1024.0,
            upload_rate: 512.0,
        }]);
        let addr = start_test_server(state).await;

        let (status, body) = http_get(addr, "/api/network").await;
        assert_eq!(status, 200);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["network"][0]["interface"], "eth0");
        assert_eq!(parsed["network"][0]["download_rate"], 1024.0);
    }

    #[tokio::test]
    async fn test_get_cgroups_endpoint() {
        let state = new_test_state();
        let addr = start_test_server(state).await;

        let (status, body) = http_get(addr, "/api/cgroups").await;
        assert_eq!(status, 200);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(parsed["cgroups"].is_null());
    }
}
