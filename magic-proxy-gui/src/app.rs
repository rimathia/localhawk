use iced::widget::{button, column, container, row, text, text_input};
use iced::{Element, Length, Task};

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    ButtonPressed,
    ClearText,
}

#[derive(Debug, Default)]
pub struct AppState {
    input_text: String,
    display_text: String,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            input_text: String::new(),
            display_text: "Welcome to Magic Card Proxy Generator!".to_string(),
        }
    }
}

pub fn initialize() -> (AppState, Task<Message>) {
    (AppState::new(), Task::none())
}

pub fn update(state: &mut AppState, message: Message) -> Task<Message> {
    match message {
        Message::InputChanged(value) => {
            state.input_text = value;
        }
        Message::ButtonPressed => {
            if !state.input_text.trim().is_empty() {
                state.display_text = format!("You entered: {}", state.input_text);
            } else {
                state.display_text = "Please enter some text!".to_string();
            }
        }
        Message::ClearText => {
            state.input_text.clear();
            state.display_text = "Text cleared!".to_string();
        }
    }
    Task::none()
}

pub fn view(state: &AppState) -> Element<Message> {
    let input_section = column![
        text("Enter card name:").size(18),
        text_input("Type here...", &state.input_text)
            .on_input(Message::InputChanged)
            .padding(10),
    ]
    .spacing(10);

    let button_section = row![
        button("Search Card")
            .on_press(Message::ButtonPressed)
            .padding(10),
        button("Clear")
            .on_press(Message::ClearText)
            .padding(10),
    ]
    .spacing(10);

    let display_section = column![
        text(&state.display_text).size(16),
    ]
    .spacing(10);

    let content = column![
        input_section,
        button_section,
        display_section,
    ]
    .spacing(20)
    .padding(20);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}