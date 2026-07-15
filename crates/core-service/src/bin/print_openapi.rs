//! Утилита для выгрузки OpenAPI-спецификации в файл/stdout без поднятия сервера и БД.
//! Пример: `cargo run --bin print_openapi > openapi.json`

use core_service::presentation::api::openapi::ApiDoc;
use utoipa::OpenApi;

fn main() {
    let doc = ApiDoc::openapi();
    println!("{}", doc.to_pretty_json().expect("failed to serialize OpenAPI spec"));
}
