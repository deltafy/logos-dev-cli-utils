use napi_derive::napi;
use std::env;
use std::process::Command;
use serde::{Serialize, Deserialize};
use tokio_postgres::{NoTls, Error as PgError};

// map std::process::Output so we could mimic the callback
// parameters of Node's child_process.exec()
#[napi(object)]
#[derive(Serialize, Deserialize)]
pub struct ProcessOutput {
    pub status: i32,            // convert from ExitStatus
    pub stdout: String,         // convert from Vec<u8>
    pub stderr: String,         // convert from Vec<u8>
}

#[napi(object)]
#[derive(Serialize, Deserialize)]
pub struct PgResponse {
    pub code: String,           // convert from SqlState
    pub message: String,
}

fn get_postgres_error_code(error: &PgError) -> String {
    if let Some(db_error) = error.as_db_error() {
        db_error.code().code().to_string()
    } else {
        "unknown".to_string()
    }
}

fn get_postgres_error_message(error: &PgError) -> String {
    if let Some(db_error) = error.as_db_error() {
        db_error.message().to_string()
    } else {
        error.to_string()
    }
}

#[napi]
pub async fn run_npm_script(script: String) -> napi::Result<ProcessOutput> {
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

    Ok(ProcessOutput {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

#[napi]
pub async fn test_postgres_url(url: String) -> napi::Result<PgResponse> {
    let (client, connection) = match tokio_postgres::connect(&url, NoTls).await {
        Ok((client, connection)) => (client, connection),
        Err(error) => {
            let error_msg = PgResponse {
                code: get_postgres_error_code(&error),
                message: get_postgres_error_message(&error)
            };

            return Err(napi::Error::new(
                napi::Status::GenericFailure,
                serde_json::to_string(&error_msg).unwrap(),
            ));
        }
    };

    tokio::spawn(async move {
        if let Err(error) = connection.await {
            eprintln!("Connection Error: {}", get_postgres_error_message(&error))
        }
    });

    match client.simple_query("SELECT 1").await {
        Ok(_) => Ok(PgResponse {
            code: "00000".to_string(),
            message: "Success".to_string()
        }),
        Err(error) => {
            let error_msg = PgResponse {
                code: get_postgres_error_code(&error),
                message: get_postgres_error_message(&error)
            };

            Err(napi::Error::new(
                napi::Status::GenericFailure,
                serde_json::to_string(&error_msg).unwrap(),
            ))
        }
    }
}
