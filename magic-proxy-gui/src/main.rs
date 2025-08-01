use iced::{Application, Settings};

mod app;

fn main() -> iced::Result {
    env_logger::init();
    
    app::MagicProxyApp::run(Settings::default())
}