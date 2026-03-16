use mental_os::openclaw::OpenClawClient;

#[tokio::test]
async fn live_ollama() {
    if std::env::var("MENTALOS_LIVE_OLLAMA").unwrap_or_default() != "1" {
        eprintln!("Skipping live_ollama test; set MENTALOS_LIVE_OLLAMA=1 to run.");
        return;
    }

    let mut config = mental_os::Config::load().expect("config missing");
    config.ai.provider = "ollama".to_string();
    let client = OpenClawClient::from_config(&config);
    let response = client
        .send_message("Say hello in one sentence.", &[])
        .await
        .expect("ollama request failed");

    assert!(!response.trim().is_empty());
}
