use iced::{
    widget::{button, column, container, row, text, text_input, scrollable, progress_bar},
    Element, Length, Theme, Task,
};
use magic_proxy_core::{ProxyGenerator, Card, CardSearchResult, ProxyError};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Message {
    SearchInputChanged(String),
    SearchCard,
    CardSearchResult(Result<CardSearchResult, ProxyError>),
    AddCard(Card),
    RemoveCard(usize),
    GeneratePdf,
    PdfGenerated(Result<Vec<u8>, ProxyError>),
    SavePdf,
    GenerationProgress(usize, usize),
    ClearCards,
}

pub struct MagicProxyApp {
    generator: Arc<tokio::sync::Mutex<ProxyGenerator>>,
    search_input: String,
    search_results: Option<CardSearchResult>,
    is_searching: bool,
    is_generating: bool,
    generation_progress: Option<(usize, usize)>,
    generated_pdf: Option<Vec<u8>>,
    error_message: Option<String>,
}

impl iced::Application for MagicProxyApp {
    type Message = Message;
    type Theme = Theme;
    type Executor = iced::executor::Default;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let generator = match ProxyGenerator::new() {
            Ok(proxy_gen) => Arc::new(tokio::sync::Mutex::new(proxy_gen)),
            Err(e) => {
                log::error!("Failed to create ProxyGenerator: {}", e);
                return (
                    MagicProxyApp {
                        generator: Arc::new(tokio::sync::Mutex::new(ProxyGenerator::default())),
                        search_input: String::new(),
                        search_results: None,
                        is_searching: false,
                        is_generating: false,
                        generation_progress: None,
                        generated_pdf: None,
                        error_message: Some(format!("Failed to initialize: {}", e)),
                    },
                    Task::none(),
                );
            }
        };

        (
            MagicProxyApp {
                generator,
                search_input: String::new(),
                search_results: None,
                is_searching: false,
                is_generating: false,
                generation_progress: None,
                generated_pdf: None,
                error_message: None,
            },
            Task::none(),
        )
    }

    fn title(&self) -> String {
        "Magic Card Proxy Generator".to_string()
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::SearchInputChanged(value) => {
                self.search_input = value;
                Task::none()
            }
            Message::SearchCard => {
                if self.search_input.trim().is_empty() {
                    return Task::none();
                }

                self.is_searching = true;
                self.error_message = None;
                let search_term = self.search_input.clone();
                let generator = self.generator.clone();

                Task::perform(
                    async move {
                        let proxy_gen = generator.lock().await;
                        proxy_gen.search_card(&search_term).await
                    },
                    Message::CardSearchResult,
                )
            }
            Message::CardSearchResult(result) => {
                self.is_searching = false;
                match result {
                    Ok(search_result) => {
                        self.search_results = Some(search_result);
                        self.error_message = None;
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Search failed: {}", e));
                        self.search_results = None;
                    }
                }
                Task::none()
            }
            Message::AddCard(card) => {
                let generator = self.generator.clone();
                Task::perform(
                    async move {
                        let mut proxy_gen = generator.lock().await;
                        proxy_gen.add_card(card, 1);
                    },
                    |_| Message::ClearCards, // Dummy message to refresh UI
                )
            }
            Message::RemoveCard(index) => {
                let generator = self.generator.clone();
                Task::perform(
                    async move {
                        let mut proxy_gen = generator.lock().await;
                        proxy_gen.remove_card(index);
                    },
                    |_| Message::ClearCards, // Dummy message to refresh UI
                )
            }
            Message::GeneratePdf => {
                self.is_generating = true;
                self.generation_progress = Some((0, 1));
                self.error_message = None;
                let generator = self.generator.clone();

                Task::perform(
                    async move {
                        let mut proxy_gen = generator.lock().await;
                        proxy_gen.generate_pdf(magic_proxy_core::PdfOptions::default(), |current, total| {
                            // TODO: Send progress updates back to UI
                        }).await
                    },
                    Message::PdfGenerated,
                )
            }
            Message::PdfGenerated(result) => {
                self.is_generating = false;
                self.generation_progress = None;
                match result {
                    Ok(pdf_data) => {
                        self.generated_pdf = Some(pdf_data);
                        self.error_message = None;
                    }
                    Err(e) => {
                        self.error_message = Some(format!("PDF generation failed: {}", e));
                    }
                }
                Task::none()
            }
            Message::SavePdf => {
                if let Some(pdf_data) = &self.generated_pdf {
                    // TODO: Implement file save dialog
                    log::info!("Would save PDF with {} bytes", pdf_data.len());
                }
                Task::none()
            }
            Message::GenerationProgress(current, total) => {
                self.generation_progress = Some((current, total));
                Task::none()
            }
            Message::ClearCards => {
                let generator = self.generator.clone();
                Task::perform(
                    async move {
                        let mut proxy_gen = generator.lock().await;
                        proxy_gen.clear_cards();
                    },
                    |_| Message::ClearCards, // Dummy message
                )
            }
        }
    }

    fn view(&self) -> Element<Self::Message> {
        let search_section = column![
            text("Search for Magic Cards").size(24),
            row![
                text_input("Enter card name...", &self.search_input)
                    .on_input(Message::SearchInputChanged)
                    .on_submit(Message::SearchCard),
                button("Search")
                    .on_press_maybe(if self.is_searching { None } else { Some(Message::SearchCard) })
            ].spacing(10),
        ].spacing(10);

        let search_results_section = if let Some(results) = &self.search_results {
            let results_list = scrollable(
                column(
                    results.cards.iter().map(|card| {
                        button(format!("{} ({})", card.name, card.set))
                            .on_press(Message::AddCard(card.clone()))
                            .width(Length::Fill)
                            .into()
                    }).collect()
                ).spacing(5)
            ).height(Length::Fixed(200.0));
            
            column![
                text(format!("Found {} cards:", results.total_found)),
                results_list
            ].spacing(10)
        } else {
            column![]
        };

        let cards_section = {
            // TODO: Get actual cards from generator
            let cards_list = column![
                text("Cards to Generate:").size(18),
                text("(Cards will be shown here)")
            ].spacing(5);

            column![
                cards_list,
                row![
                    button("Generate PDF")
                        .on_press_maybe(if self.is_generating { None } else { Some(Message::GeneratePdf) }),
                    button("Clear All")
                        .on_press(Message::ClearCards)
                ].spacing(10)
            ].spacing(10)
        };

        let progress_section = if let Some((current, total)) = self.generation_progress {
            column![
                text(format!("Generating... {}/{}", current, total)),
                progress_bar(0.0..=1.0, current as f32 / total as f32)
            ].spacing(5)
        } else if let Some(_pdf_data) = &self.generated_pdf {
            column![
                text("PDF generated successfully!"),
                button("Save PDF").on_press(Message::SavePdf)
            ].spacing(5)
        } else {
            column![]
        };

        let error_section = if let Some(error) = &self.error_message {
            column![
                text("Error:").style(|_theme: &Theme| text::Style {
                    color: Some(iced::Color::from_rgb(0.8, 0.2, 0.2)),
                }),
                text(error)
            ].spacing(5)
        } else {
            column![]
        };

        let content = column![
            search_section,
            search_results_section,
            cards_section,
            progress_section,
            error_section
        ].spacing(20).padding(20);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}