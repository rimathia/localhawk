use printpdf::image_crate::DynamicImage;
use printpdf::{Image, ImageTransform, Mm, PdfDocument};
use crate::error::ProxyError;
use crate::DoubleFaceMode;

// Constants from MagicHawk
pub const IMAGE_WIDTH: u32 = 480;
pub const IMAGE_HEIGHT: u32 = 680;
pub const PAGE_WIDTH: u32 = 3 * IMAGE_WIDTH;
pub const PAGE_HEIGHT: u32 = 3 * IMAGE_HEIGHT;
pub const IMAGE_HEIGHT_CM: f32 = 8.7;
pub const IMAGE_WIDTH_CM: f32 = IMAGE_HEIGHT_CM * IMAGE_WIDTH as f32 / IMAGE_HEIGHT as f32;

const A4_WIDTH: Mm = Mm(210.0);
const A4_HEIGHT: Mm = Mm(297.0);
const INCH_DIV_CM: f32 = 2.54;
const DPI: f32 = 300.0;
const DPCM: f32 = DPI / INCH_DIV_CM;

#[derive(Debug, Clone)]
pub struct PdfOptions {
    pub page_size: PageSize,
    pub cards_per_row: u32,
    pub cards_per_column: u32,
    pub margin: f32,
    pub double_face_mode: DoubleFaceMode,
}


#[derive(Debug, Clone)]
pub enum PageSize {
    A4,
    Letter,
    Custom { width_mm: f32, height_mm: f32 },
}

impl Default for PdfOptions {
    fn default() -> Self {
        PdfOptions {
            page_size: PageSize::A4,
            cards_per_row: 3,
            cards_per_column: 3,
            margin: 3.0,
            double_face_mode: DoubleFaceMode::BothSides, // Keep current behavior as default
        }
    }
}

pub fn generate_pdf<I>(images: I, options: PdfOptions) -> Result<Vec<u8>, ProxyError>
where
    I: Iterator<Item = DynamicImage>,
{
    let (page_width, page_height) = match options.page_size {
        PageSize::A4 => (A4_WIDTH, A4_HEIGHT),
        PageSize::Letter => (Mm(215.9), Mm(279.4)),
        PageSize::Custom { width_mm, height_mm } => (Mm(width_mm as f64), Mm(height_mm as f64)),
    };

    let (doc, page1, layer1) = PdfDocument::new("Magic Card Proxies", page_width, page_height, "Layer 1");

    let transform = ImageTransform {
        dpi: Some(DPI as f64),
        translate_x: Some((page_width - Mm((options.cards_per_row as f32 * IMAGE_WIDTH_CM * 10.0) as f64)) / 2.0),
        translate_y: Some((page_height - Mm((options.cards_per_column as f32 * IMAGE_HEIGHT_CM * 10.0) as f64)) / 2.0),
        scale_x: Some((IMAGE_WIDTH_CM / (IMAGE_WIDTH as f32) * DPCM) as f64),
        scale_y: Some((IMAGE_HEIGHT_CM / (IMAGE_HEIGHT as f32) * DPCM) as f64),
        rotate: None,
    };

    let pages_iter = images_to_pages(images, options.cards_per_row * options.cards_per_column);

    for (page_index, page_images) in pages_iter.enumerate() {
        let (current_page, current_layer) = if page_index == 0 {
            (page1, layer1)
        } else {
            doc.add_page(page_width, page_height, "Layer 1")
        };

        let layer = doc.get_page(current_page).get_layer(current_layer);
        
        for (card_index, image) in page_images.into_iter().enumerate() {
            let row = card_index as u32 / options.cards_per_row;
            let col = card_index as u32 % options.cards_per_row;
            
            let x_offset = col as f32 * IMAGE_WIDTH_CM * 10.0;
            let y_offset = (options.cards_per_column - 1 - row) as f32 * IMAGE_HEIGHT_CM * 10.0;
            
            let card_transform = ImageTransform {
                translate_x: Some(transform.translate_x.unwrap() + Mm(x_offset as f64)),
                translate_y: Some(transform.translate_y.unwrap() + Mm(y_offset as f64)),
                ..transform
            };
            
            Image::from_dynamic_image(&image).add_to_layer(layer.clone(), card_transform);
        }
    }

    doc.save_to_bytes()
        .map_err(|e| ProxyError::Pdf(format!("Failed to save PDF: {}", e)))
}

fn images_to_pages<I>(
    images: I,
    cards_per_page: u32,
) -> impl Iterator<Item = Vec<DynamicImage>>
where
    I: Iterator<Item = DynamicImage>,
{
    let mut current_page = Vec::new();
    let mut pages = Vec::new();
    
    for image in images {
        current_page.push(image);
        
        if current_page.len() == cards_per_page as usize {
            pages.push(current_page);
            current_page = Vec::new();
        }
    }
    
    // Add the last page if it has any cards
    if !current_page.is_empty() {
        pages.push(current_page);
    }
    
    pages.into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;
    use printpdf::image_crate::{DynamicImage, RgbImage};

    fn create_test_image() -> DynamicImage {
        let img = RgbImage::new(IMAGE_WIDTH, IMAGE_HEIGHT);
        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn test_pdf_options_default() {
        let options = PdfOptions::default();
        assert_eq!(options.cards_per_row, 3);
        assert_eq!(options.cards_per_column, 3);
        assert_eq!(options.margin, 3.0);
        matches!(options.page_size, PageSize::A4);
    }

    #[test]
    fn test_custom_page_size() {
        let options = PdfOptions {
            page_size: PageSize::Custom { width_mm: 200.0, height_mm: 250.0 },
            ..Default::default()
        };
        
        matches!(options.page_size, PageSize::Custom { width_mm: 200.0, height_mm: 250.0 });
    }

    #[test]
    fn test_images_to_pages_iterator() {
        let images = vec![
            create_test_image(),
            create_test_image(),
            create_test_image(),
            create_test_image(),
            create_test_image(),
        ];
        
        let pages: Vec<Vec<DynamicImage>> = images_to_pages(images.into_iter(), 3).collect();
        
        // Should create 2 pages: first with 3 images, second with 2 images
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].len(), 3);
        assert_eq!(pages[1].len(), 2);
    }

    #[test]
    fn test_generate_pdf_basic() {
        let images = vec![create_test_image()];
        let options = PdfOptions::default();
        
        let result = generate_pdf(images.into_iter(), options);
        assert!(result.is_ok());
        
        let pdf_data = result.unwrap();
        assert!(pdf_data.len() > 1000); // PDF should have reasonable size
        
        // Check PDF header
        assert_eq!(&pdf_data[0..4], b"%PDF");
    }

    #[test]
    fn test_generate_pdf_empty_images() {
        let images: Vec<DynamicImage> = vec![];
        let options = PdfOptions::default();
        
        let result = generate_pdf(images.into_iter(), options);
        assert!(result.is_ok()); // Should handle empty case gracefully
    }

    #[test]
    fn test_page_size_variants() {
        let image = create_test_image();
        
        // Test A4
        let result = generate_pdf(vec![image.clone()].into_iter(), PdfOptions {
            page_size: PageSize::A4,
            ..Default::default()
        });
        assert!(result.is_ok());
        
        // Test Letter
        let result = generate_pdf(vec![image.clone()].into_iter(), PdfOptions {
            page_size: PageSize::Letter,
            ..Default::default()
        });
        assert!(result.is_ok());
        
        // Test Custom
        let result = generate_pdf(vec![image].into_iter(), PdfOptions {
            page_size: PageSize::Custom { width_mm: 200.0, height_mm: 280.0 },
            ..Default::default()
        });
        assert!(result.is_ok());
    }
}