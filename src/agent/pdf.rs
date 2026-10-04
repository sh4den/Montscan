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
