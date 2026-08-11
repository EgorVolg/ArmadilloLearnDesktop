import { invoke } from "@tauri-apps/api/core";
import "./MainApp.css";

export const MainApp = () => {
  // Тест захвата экрана.
  const testCapture = async () => {
    console.log("CAPTURE TEST BUTTON");

    try {
      const result = await invoke<string>("test_capture");

      console.log("CAPTURE RESULT:", result);
    } catch (error) {
      console.error("CAPTURE ERROR:", error);
    }
  };

  // Тест захвата + crop.
  const testCrop = async () => {
    console.log("CROP TEST BUTTON");

    try {
      const result = await invoke<string>("test_crop");

      console.log("CROP RESULT:", result);
    } catch (error) {
      console.error("CROP ERROR:", error);
    }
  };

  // Тест полного recognition pipeline.
  const testOcr = async () => {
    console.log("OCR TEST BUTTON");

    try {
      const result = await invoke<string>("test_ocr");

      console.log("OCR RESULT:", result);
    } catch (error) {
      console.error("OCR ERROR:", error);
    }
  };

  const testOnnx = async () => {
    console.log("ONNX BUTTON CLICK");

    try {
      const result = await invoke<string>("test_onnx");

      console.log("ONNX RESULT:", result);
    } catch (error) {
      console.error("ONNX ERROR:", error);
    }
  };

  return (
    <div>
      <h1>Armadillo</h1>

      <p>Настройки приложения</p>

      <button
        onClick={testCapture}
        style={{
          zIndex: 100,
        }}
      >
        Test Capture
      </button>

      <button
        onClick={testCrop}
        style={{
          zIndex: 100,
        }}
      >
        Test Crop
      </button>

      <button
        onClick={testOcr}
        style={{
          zIndex: 100,
        }}
      >
        Test OCR
      </button>
      <button onClick={testOnnx}>
        Test ONNX
      </button>
    </div>
  );
};