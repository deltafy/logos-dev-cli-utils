use napi_derive::napi;
use serde::{Serialize, Deserialize};
use tokio_postgres::{Client, Error, Connection, NoTls, Socket};
use tokio_postgres::tls::NoTlsStream;

#[napi(object)]
#[derive(Serialize, Deserialize)]
pub struct PgResponse {
    pub code: String,           // convert from SqlState
    pub message: String,
}

pub fn get_postgres_error_message(error: &Error) -> String {
    if let Some(db_error) = error.as_db_error() {
        db_error.message().to_string()
    } else {
        error.to_string()
    }
}

pub fn create_postgres_error_response(error: &Error) -> napi::Result<PgResponse> {
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

pub async fn create_postgres_connection(url: &str) -> Result<(Client, Connection<Socket, NoTlsStream>), Error> {
    tokio_postgres::connect(url, NoTls).await
}

pub async fn handle_postgres_connection(conn: Connection<Socket, NoTlsStream>) {
    if let Err(error) = conn.await {
        eprintln!("Connection Error: {}", get_postgres_error_message(&error))
    }
}

