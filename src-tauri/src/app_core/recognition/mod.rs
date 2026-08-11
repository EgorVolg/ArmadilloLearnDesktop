// ┌─────────────┬──────────────────────────────────────────────┐
// │ Файл        │ Что делает                                   │
// ├─────────────┼──────────────────────────────────────────────┤
// │ capture.rs  │ Делает screenshot                            │
// │ crop.rs     │ Обрезает нужную область                      │
// │ image.rs    │ Наш внутренний тип изображения               │
// │ region.rs   │ Координаты областей                          │
// │ types.rs    │ OcrResult, TextRegion и т.д.                 │
// │ ocr.rs      │ OcrEngine trait                              │
// │ paddle/     │ Реализация OcrEngine через PP-OCRv5 + ONNX   │
// │ service.rs  │ Более высокий уровень работы с OCR           │
// └─────────────┴──────────────────────────────────────────────┘

pub mod capture;
pub mod crop;
pub mod image;
pub mod ocr;
pub mod region;
pub mod service;
pub mod types;

pub mod paddle;

pub mod onnx_test;
