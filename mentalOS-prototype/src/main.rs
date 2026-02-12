fn main() {
    env_logger::init();
    match mentalOS::Config::load() {
        Ok(config) => {
            println!(
                "mentalOS prototype: config loaded. Provider={}, model={}",
                config.ai.provider, config.ai.model
            );
        }
        Err(err) => {
            eprintln!("mentalOS prototype: config not loaded: {err}");
        }
    }
}
