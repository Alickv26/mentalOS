use mental_os::providers::ollama::OllamaProvider;
use mental_os::providers::AiProvider;

#[tokio::test]
async fn live_ollama() {
    if std::env::var("MENTALOS_LIVE_OLLAMA").unwrap_or_default() != "1" {
        eprintln!("Skipping live_ollama test; set MENTALOS_LIVE_OLLAMA=1 to run.");
        return;
    }

    let config = mental_os::Config::load().expect("config missing");
    let client = OllamaProvider::from_config(&config).expect("create ollama provider");

    let response = client
        .send_message("Say hello in one sentence.", &[])
        .expect("ollama request failed");

    assert!(!response.trim().is_empty());
}
