use ort::session::Session;

/// Тестирует загрузку ONNX-модели.
///
/// На этом этапе мы ещё не выполняем inference.
/// Наша задача — убедиться, что ONNX Runtime
/// может открыть модель и увидеть её входы/выходы.
pub fn test_model(path: &str) -> Result<(), String> {
    println!("=== ONNX MODEL TEST ===");
    println!("Loading: {}", path);

    let session = Session::builder()
        .map_err(|error| format!("Session builder error: {error}"))?
        .commit_from_file(path)
        .map_err(|error| format!("Failed to load ONNX model: {error}"))?;

    println!("Model loaded successfully.");

    println!("Inputs: {}", session.inputs().len());

    for (index, input) in session.inputs().iter().enumerate() {
        println!("INPUT #{index}:");
        println!("  name: {}", input.name());
        println!("  type: {:?}", input.dtype());
    }

    println!("Outputs: {}", session.outputs().len());

    for (index, output) in session.outputs().iter().enumerate() {
        println!("OUTPUT #{index}:");
        println!("  name: {}", output.name());
        println!("  type: {:?}", output.dtype());
    }

    println!("=== ONNX MODEL TEST END ===");

    Ok(())
}
