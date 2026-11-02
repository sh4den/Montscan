use std::path::Path;

use anyhow::{Context, anyhow};
use base64::Engine;
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings};

const DPI: f32 = 300.0;
const JPEG_QUALITY: u8 = 85;

pub async fn extract_first_page(pdf: &Path) -> anyhow::Result<String> {
    tracing::info!("Extracting first page from PDF: {}", pdf.display());
    let data = tokio::fs::read(pdf).await.context("failed to read PDF")?;
    let jpeg = tokio::task::spawn_blocking(move || render_first_page(data)).await??;
    tracing::info!("Extracted image: {} bytes", jpeg.len());
    Ok(base64::engine::general_purpose::STANDARD.encode(jpeg))
}

fn render_first_page(data: Vec<u8>) -> anyhow::Result<Vec<u8>> {
    let pdf = Pdf::new(data).map_err(|e| anyhow!("failed to parse PDF: {e:?}"))?;
    let page = pdf.pages().first().context("PDF has no pages")?;

    let (width, height) = page.render_dimensions();
    let scale = (DPI / 72.0).min(f32::from(u16::MAX) / width.max(height).max(1.0));
    let pixmap = hayro::render(
        page,
        &RenderCache::new(),
        &InterpreterSettings::default(),
        &RenderSettings::default(),
        &PixmapSettings {
            x_scale: scale,
            y_scale: scale,
            bg_color: WHITE,
        },
    );

    let rgb: Vec<u8> = pixmap
        .data_as_u8_slice()
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[r, g, b, _]| [r, g, b])
        .collect();
    let image =
        image::RgbImage::from_raw(u32::from(pixmap.width()), u32::from(pixmap.height()), rgb)
            .context("rendered page has inconsistent dimensions")?;

    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY)
        .encode_image(&image)
        .context("failed to encode JPEG")?;
    Ok(jpeg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pdf() -> Vec<u8> {
        let content = "0 0 0 rg 72 72 144 144 re f";
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R >>".to_string(),
            format!(
                "<< /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            ),
        ];
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (i, body) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend(format!("{} 0 obj\n{body}\nendobj\n", i + 1).bytes());
        }
        let xref = pdf.len();
        pdf.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
        for offset in offsets {
            pdf.extend(format!("{offset:010} 00000 n \n").bytes());
        }
        pdf.extend(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objects.len() + 1
            )
            .bytes(),
        );
        pdf
    }

    #[test]
    fn renders_first_page_at_300_dpi() {
        let jpeg = render_first_page(sample_pdf()).unwrap();
        let image = image::load_from_memory(&jpeg).unwrap().into_rgb8();
        let (width, height) = image.dimensions();
        assert_eq!(width, 2550);
        assert!((3299..=3300).contains(&height), "height {height}");

        assert!(image.get_pixel(10, 10).0.iter().all(|&c| c > 240));
        assert!(image.get_pixel(450, height - 450).0.iter().all(|&c| c < 15));
    }

    #[test]
    fn rejects_non_pdf_input() {
        assert!(render_first_page(b"not a pdf".to_vec()).is_err());
    }
}
