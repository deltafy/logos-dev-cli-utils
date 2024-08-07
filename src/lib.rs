use napi_derive::napi;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use serde::{Serialize, Deserialize};

mod postgres;
mod file;

use postgres::PgResponse;

// map std::process::Output so we could mimic the callback
// parameters of Node's child_process.exec()
#[napi(object)]
#[derive(Serialize, Deserialize)]
pub struct ProcessOutput {
    pub status: i32,            // convert from ExitStatus
    pub stdout: String,         // convert from Vec<u8>
    pub stderr: String,         // convert from Vec<u8>
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
    let (client, connection) = match postgres::create_postgres_connection(&url).await {
        Ok((client, connection)) => (client, connection),
        Err(error) => {
            return postgres::create_postgres_error_response(&error);
        }
    };

    tokio::spawn(postgres::handle_postgres_connection(connection));

    match client.simple_query("SELECT 1").await {
        Ok(_) => Ok(PgResponse {
            code: "00000".to_string(),
            message: "Success".to_string()
        }),
        Err(error) => postgres::create_postgres_error_response(&error),
    }
}

#[napi]
pub async fn create_database(url: String, database: String) -> napi::Result<PgResponse> {
    let (client, connection) = match postgres::create_postgres_connection(&url).await {
        Ok((client, connection)) => (client, connection),
        Err(error) => {
            return postgres::create_postgres_error_response(&error);
        }
    };

    tokio::spawn(postgres::handle_postgres_connection(connection));

    let query = format!("CREATE DATABASE \"{}\"", database);
    match client.simple_query(&query).await {
        Ok(_) => Ok(PgResponse {
            code: "00000".to_string(),
            message: format!("Successfully created database '{}'", database)
        }),
        Err(error) => postgres::create_postgres_error_response(&error)
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

#[napi]
pub async fn rename_database(url: String, database: String, new_database_name: String) -> napi::Result<PgResponse> {
    let (client, connection) = match postgres::create_postgres_connection(&url).await {
        Ok((client, connection)) => (client, connection),
        Err(error) => {
            return postgres::create_postgres_error_response(&error);
        }
    };

    tokio::spawn(postgres::handle_postgres_connection(connection));

    let query = format!("ALTER DATABASE \"{}\" RENAME TO \"{}\";", database, new_database_name);
    match client.batch_execute(&query).await {
        Ok(_) => Ok(PgResponse {
            code: "00000".to_string(),
            message: format!("Database {} renamed to {}", database, new_database_name)
        }),
        Err(error) => postgres::create_postgres_error_response(&error)
    }
}

#[napi]
pub fn find_nonexistent_files(paths: Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .filter(|path| !fs::metadata(path).is_ok())
        .collect()
}

#[napi]
pub fn copy_file(
    source: String, 
    destination: String, 
    create_dest_if_not_exists: Option<bool>
) -> napi::Result<()> {
    let mut source_file = file::read_file(&source)?;

    let mut destination_file = if create_dest_if_not_exists.unwrap_or(false) {
        file::create_file(&destination)?
    } else {
        file::read_file(&destination)?
    };

    io::copy(&mut source_file, &mut destination_file)
        .map_err(|error| napi::Error::new(
            napi::Status::GenericFailure,
            format!("Failed to write to {}: {}", &destination, error)
        ))?;

    Ok(())
}

#[napi]
pub fn env_to_json_string(env_path: String) -> napi::Result<String> {
    let env_file_path = Path::new(&env_path);

    let env_data = fs::read_to_string(env_file_path)
        .map_err(|error| napi::Error::new(
            napi::Status::GenericFailure,
            format!("Failed to read .env: {}", error).to_string()
        ))?;

    let mut json_map = serde_json::Map::new();

    for line in env_data.lines() {
        if let Some((key, value)) = line.split_once('=') {
            json_map.insert(
                key.to_string(), 
                serde_json::Value::String(value.to_string())
            );
        }
    }

    match serde_json::to_string(&json_map) {
        Ok(json_str) => Ok(json_str),
        Err(error) => {
            return Err(napi::Error::new(
                napi::Status::GenericFailure,
                format!("Failed to create JSON string: {}", error).to_string()
            ))
        }
    }
}

#[napi]
pub fn json_string_to_env(json_str: String, env_path: String) -> napi::Result<()> {
    let env_file_path = Path::new(&env_path);
    
    let data: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|error| napi::Error::new(
            napi::Status::GenericFailure,
            format!("Failed to parse to JSON: {}", error).to_string()
        ))?;

    let json_map = data.as_object().ok_or_else(|| {
        napi::Error::new(
            napi::Status::GenericFailure,
            "Cannot convert JSON string to object".to_string()
        )
    })?;

    let mut file = fs::File::create(env_file_path)
        .map_err(|error| napi::Error::new(
            napi::Status::GenericFailure,
            format!("Failed to create output file: {}", error).to_string()
        ))?;

    for (key, value) in json_map {
        let val = match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => "".to_string(),
            _ => continue,
        };

        writeln!(file, "{}={}", key, val)
            .map_err(|error| napi::Error::new(
                napi::Status::GenericFailure,
                format!("Failed to write to output file: {}", error).to_string()
            ))?;
    }

    Ok(())
}

