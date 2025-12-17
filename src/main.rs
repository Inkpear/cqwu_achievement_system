use cqwu_achievement_system::{
    configuration::get_configuration,
    startup::Application,
    telemetry::{get_subscriber, init_subscriber},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let configuration = get_configuration().expect("Failed to load configuration");
    
    let (subscriber, _guard) = get_subscriber(
        "cqwu_achievement_system".into(),
        "info".into(),
        std::io::stdout,
        Some(configuration.log.clone())
    );
    init_subscriber(subscriber);


    let app = Application::build(configuration).await?;

    app.run_until_stopped().await?;

    Ok(())
}
