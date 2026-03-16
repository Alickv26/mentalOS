use mental_os::openclaw::OpenClawClient;

#[tokio::test]
async fn live_openclaw() {
    if std::env::var("MENTALOS_LIVE_OPENCLAW").unwrap_or_default() != "1" {
        eprintln!("Skipping live_openclaw test; set MENTALOS_LIVE_OPENCLAW=1 to run.");
        return;
    }

    let config = mental_os::Config::load().expect("config missing");
    let client = OpenClawClient::from_config(&config);
    let response = client
        .send_message("Say hello in one sentence.", &[])
        .await
        .expect("openclaw request failed");

    assert!(!response.trim().is_empty());
}
