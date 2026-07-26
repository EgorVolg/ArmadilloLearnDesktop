use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_DIR: &str = r"C:\Users\volge\Desktop\папака";

fn default_path(prefix: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let filename = format!("{}_{}.png", prefix, timestamp);
    let mut path = PathBuf::from(DEFAULT_DIR);
    path.push(&filename);
    path
}

/// Делает скриншот всего экрана и возвращает PNG-байты (без сохранения в файл)
pub fn capture_full_screen_bytes() -> Result<Vec<u8>, String> {
    let ps_script = r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$screen = [System.Windows.Forms.Screen]::PrimaryScreen
$bounds = $screen.Bounds
$bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($bounds.X, $bounds.Y, 0, 0, $bounds.Size)
$ms = New-Object System.IO.MemoryStream
$bitmap.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
[System.Convert]::ToBase64String($ms.ToArray())
$ms.Dispose()
"#;

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_script])
        .output()
        .map_err(|e| format!("Failed to execute PowerShell: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PowerShell error: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let base64_str = stdout.trim();

    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, base64_str)
        .map_err(|e| format!("Base64 decode error: {}", e))
}

/// Делает скриншот области экрана и возвращает PNG-байты (без сохранения в файл)
pub fn capture_area_bytes(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    let ps_script = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$bitmap = New-Object System.Drawing.Bitmap {}, {}
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen({}, {}, 0, 0, $bitmap.Size)
$ms = New-Object System.IO.MemoryStream
$bitmap.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
[System.Convert]::ToBase64String($ms.ToArray())
$ms.Dispose()
"#,
        width, height, x, y
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_script])
        .output()
        .map_err(|e| format!("Failed to execute PowerShell: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PowerShell error: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let base64_str = stdout.trim();

    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, base64_str)
        .map_err(|e| format!("Base64 decode error: {}", e))
}

/// Делает скриншот всего экрана и сохраняет в файл.
/// Если `output_path` не указан, сохраняет в `C:\Users\volge\Desktop\папака\screenshot_full_<timestamp>.png`
pub fn capture_full_screen(output_path: Option<&str>) -> Result<String, String> {
    let path = match output_path {
        Some(p) => PathBuf::from(p),
        None => default_path("screenshot_full"),
    };

    let path_str = path.to_string_lossy().to_string();

    // Используем PowerShell для скриншота через .NET
    let ps_script = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$screen = [System.Windows.Forms.Screen]::PrimaryScreen
$bounds = $screen.Bounds
$bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($bounds.X, $bounds.Y, 0, 0, $bounds.Size)
$bitmap.Save('{}', [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
"#,
        path_str.replace("'", "''")
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_script])
        .output()
        .map_err(|e| format!("Failed to execute PowerShell: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PowerShell error: {}", stderr));
    }

    Ok(path_str)
}

/// Делает скриншот области экрана с координатами (x, y) и заданными размерами.
/// Если `output_path` не указан, сохраняет в `C:\Users\volge\Desktop\папака\screenshot_area_<timestamp>.png`
pub fn capture_area(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    output_path: Option<&str>,
) -> Result<String, String> {
    let path = match output_path {
        Some(p) => PathBuf::from(p),
        None => default_path("screenshot_area"),
    };

    let path_str = path.to_string_lossy().to_string();

    // Используем PowerShell для скриншота области через .NET
    let ps_script = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$bitmap = New-Object System.Drawing.Bitmap {}, {}
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen({}, {}, 0, 0, $bitmap.Size)
$bitmap.Save('{}', [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
"#,
        width,
        height,
        x,
        y,
        path_str.replace("'", "''")
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_script])
        .output()
        .map_err(|e| format!("Failed to execute PowerShell: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PowerShell error: {}", stderr));
    }

    Ok(path_str)
}