use napi_derive::napi;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use serde::{Serialize, Deserialize};
use tokio_postgres::{NoTls, Error as PgError, Client as PgClient, Connection as PgConnection};

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

fn get_postgres_error_message(error: &PgError) -> String {
    if let Some(db_error) = error.as_db_error() {
        db_error.message().to_string()
    } else {
        error.to_string()
    }
}

fn create_postgres_error_response(error: &PgError) -> napi::Result<PgResponse> {
    let (error_code, error_details) = if let Some(db_error) = error.as_db_error() {
        (
            db_error.code().code().to_string(),
            db_error.message().to_string()
        )
    } else {
        (
            "unknown".to_string(),
            error.to_string()
        )
    };

    let error_result = PgResponse {
        code: error_code,
        message: error_details
    };

    Err(napi::Error::new(
        napi::Status::GenericFailure,
        serde_json::to_string(&error_result).unwrap()
    ))
}

async fn create_postgres_connection(
    url: &str
) -> Result<(PgClient, PgConnection<tokio_postgres::Socket, tokio_postgres::tls::NoTlsStream>), PgError> {
    tokio_postgres::connect(url, NoTls).await
}

async fn handle_postgres_connection(
    conn: PgConnection<tokio_postgres::Socket, tokio_postgres::tls::NoTlsStream>
) {
    if let Err(error) = conn.await {
        eprintln!("Connection Error: {}", get_postgres_error_message(&error))
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
    let (client, connection) = match create_postgres_connection(&url).await {
        Ok((client, connection)) => (client, connection),
        Err(error) => {
            return create_postgres_error_response(&error);
        }
    };

    tokio::spawn(handle_postgres_connection(connection));

    match client.simple_query("SELECT 1").await {
        Ok(_) => Ok(PgResponse {
            code: "00000".to_string(),
            message: "Success".to_string()
        }),
        Err(error) => create_postgres_error_response(&error),
    }
}

#[napi]
pub async fn create_database(url: String, database: String) -> napi::Result<PgResponse> {
    let (client, connection) = match create_postgres_connection(&url).await {
        Ok((client, connection)) => (client, connection),
        Err(error) => {
            return create_postgres_error_response(&error);
        }
    };

    tokio::spawn(handle_postgres_connection(connection));

    let query = format!("CREATE DATABASE \"{}\"", database);
    match client.simple_query(&query).await {
        Ok(_) => Ok(PgResponse {
            code: "00000".to_string(),
            message: format!("Successfully created database '{}'", database)
        }),
        Err(error) => create_postgres_error_response(&error)
    }
}

#[napi]
pub async fn test_redis_parameters(
    host: String, 
    username: Option<String>,
    password: Option<String>
) -> napi::Result<String> {
    let url = match (username, password) {
        (Some(user), Some(pass)) => format!("redis://{}:{}@{}/", user, pass, host),
        (None, Some(pass)) => format!("redis://:{}@{}/", pass, host),
        _ => format!("redis://{}/", host),
    };

    let client = match redis::Client::open(url) {
        Ok(client) => client,
        Err(error) => {
            return Err(napi::Error::new(
                napi::Status::GenericFailure,
                error.to_string()
            ))
        }
    };

    let mut connection = match client.get_multiplexed_tokio_connection().await {
        Ok(conn) => conn,
        Err(error) => {
            return Err(napi::Error::new(
                napi::Status::GenericFailure,
                error.to_string()
            ))
        }
    };

    let ping: Result<String, _> = redis::cmd("PING").query_async(&mut connection).await;

    match ping {
        Ok(pong) => Ok(pong),
        Err(error) => {
            return Err(napi::Error::new(
                napi::Status::GenericFailure,
                error.to_string()
            ))
        }
    }
}

#[napi]
pub fn file_exists(file_path: String) -> bool {
    let filepath = Path::new(&file_path);
    fs::metadata(filepath).map(|metadata| metadata.is_file()).unwrap_or(false)
}

