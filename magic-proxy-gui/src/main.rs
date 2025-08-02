mod app;

fn main() -> iced::Result {
    env_logger::init();
    
    iced::application("Magic Card Proxy Generator", app::update, app::view)
        .run_with(app::initialize)
}