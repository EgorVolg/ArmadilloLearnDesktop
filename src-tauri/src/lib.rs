mod app;
mod app_core;

use app::runtime::AppRuntime;
use tauri::Manager;

use crate::app_core::pipeline::recognition_pipeline::RecognitionPipeline;
use crate::app_core::recognition::{ capture::capture_screen, crop::crop, region::Region };

#[tauri::command]
fn test_crop() -> Result<String, String> {
    // Захватываем весь экран.
    let image = capture_screen().map_err(|error| format!("{error:?}"))?;

    // Для теста берём прямоугольник
    // в левом верхнем углу экрана.
    let region = Region::new(
        100, // X
        100, // Y
        800, // width
        400 // height
    );

    // Обрезаем screenshot.
    let cropped = crop(&image, region).map_err(|error| format!("{error:?}"))?;

    println!("Original: {}x{}", image.width, image.height);

    println!("Cropped: {}x{}", cropped.width, cropped.height);

    println!("Cropped RGB buffer: {} bytes", cropped.data.len());

    Ok(format!("{}x{}", cropped.width, cropped.height))
}

#[tauri::command]
fn test_ocr() -> Result<String, String> {
    println!("=== TEST OCR COMMAND ===");

    // Создаём pipeline распознавания.
    let pipeline = RecognitionPipeline::new();

    // Пока используем фиксированную область
    // только для тестирования.
    let region = Region::new(
        100, // X
        100, // Y
        800, // width
        400 // height
    );

    // Запускаем полный recognition pipeline.
    let result = pipeline.run(region)?;

    println!("OCR regions: {}", result.regions.len());

    Ok(format!("OCR completed: {} regions", result.regions.len()))
}

#[tauri::command]
fn test_capture() -> Result<String, String> {
    let image = capture_screen().map_err(|error| format!("{error:?}"))?;

    println!("Captured screen: {}x{}", image.width, image.height);

    println!("RGB buffer: {} bytes", image.data.len());

    Ok(format!("{}x{}", image.width, image.height))
}

pub fn run() {
    dotenv::dotenv().ok();

    tauri::Builder
        ::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![test_crop, test_ocr, test_capture])
        .setup(|app| {
            let runtime = AppRuntime::new(app.handle().clone());

            app.manage(runtime);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
