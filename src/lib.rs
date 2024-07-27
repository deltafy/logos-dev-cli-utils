use napi_derive::napi;
use std::env;
use std::process::Command;

#[napi]
pub async fn run_npm_script(script: String) -> napi::Result<String> {
    let home_dir = env::var("HOME").or_else(|_| env::var("USERPROFILE")).unwrap_or_default();
    let npm_path = if cfg!(target_os = "windows") {
        format!("{}\\AppData\\Roaming\\npm", home_dir)
    } else {
        format!("{}/.npm-global/bin", home_dir)
    };

    let mut path = env::var("PATH").unwrap_or_default();

    path.push(if cfg!(target_os = "windows") { ';' } else { ':' });
    path.push_str(&npm_path);

    let (command, args) = if cfg!(target_os = "windows") {
        ("cmd", vec!["/C", &script])
    } else {
        ("sh", vec!["-c", &script])
    };

    let output = Command::new(command)
        .args(&args)
        .env("PATH", path)
        .output()
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(napi::Error::from_reason(
            String::from_utf8_lossy(&output.stderr).into_owned()
        ))
    }
}
