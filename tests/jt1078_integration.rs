use wvp_gb28181_server::jt1078::Jt1078Server;

#[tokio::test]
async fn jt1078_init_smoke() {
    // Smoke test to ensure JT1078 server init API is callable
    let server = Jt1078Server::new();
    assert!(server.init().await.is_ok());
}
