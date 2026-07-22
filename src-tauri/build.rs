fn main() {
    tauri_build::build(); 
    let signtool_path = r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\signtool.exe";
    let binary_path = r"D:\armadillo-learn-desktop\src-tauri\target\debug\armadillo-learn-desktop.exe";

    if std::path::Path::new(signtool_path).exists() {
        let status = std::process::Command::new(signtool_path)
            .args(["sign", "/fd", "SHA256", "/a", binary_path])
            .status()
            .expect("Failed to sign binary");

        if status.success() {
            println!("cargo:warning=Binary signed successfully!");
        } else {
            println!("cargo:warning=Failed to sign binary!");
        }
    } else {
        println!("cargo:warning=Signtool not found at: {}", signtool_path);
    }
}
