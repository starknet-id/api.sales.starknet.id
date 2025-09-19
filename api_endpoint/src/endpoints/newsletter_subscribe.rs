use std::sync::Arc;

use crate::{models::AppState, utils::get_error};
use axum::{extract::State, response::IntoResponse, Json};
use reqwest::StatusCode;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct AddNewsletterQuery {
    email: String,
    address: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct AddNewsletterRecord {
    email: String,
    address: Option<String>,
    source: String,
}

#[derive(Serialize)]
pub struct Output {
    success: bool,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(query): Json<AddNewsletterQuery>,
) -> impl IntoResponse {
    let collection = state.db.collection::<mongodb::bson::Document>("newsletter");

    // Check if email already exists
    let filter = mongodb::bson::doc! { "email": &query.email };
    let result = match collection.find_one(filter, None).await {
        Ok(res) => res,
        Err(err) => {
            state
                .logger
                .severe(format!("Failed to execute find_one: {}", err));
            return get_error("Internal server error".to_string());
        }
    };

    if let Some(_) = result {
        return get_error("Email already exists".to_string());
    }

    // Mailerlite API
    let base_url = state.conf.email.base_url.clone();
    let api_key = state.conf.email.api_key.clone();
    let ar_group_id = state.conf.email.ar_group_id.clone();

    let url = format!("{}/subscribers", base_url);
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({ "email": query.email, "groups": [ar_group_id] }))
        .send()
        .await;

    if let Err(err) = response {
        return get_error(format!("Failed to send request to Mailerlite: {}", err));
    }

    let bson_doc = match mongodb::bson::to_bson(&AddNewsletterRecord {
        email: query.email,
        address: query.address,
        source: "newsletter_subscription".to_string(),
    }) {
        Ok(bson) => bson,
        Err(err) => {
            state
                .logger
                .severe(format!("Failed to serialize to BSON: {}", err));
            return get_error("Internal server error".to_string());
        }
    };

    if let mongodb::bson::Bson::Document(document) = bson_doc {
        match collection.insert_one(document, None).await {
            Ok(_) => (),
            Err(err) => return get_error(format!("Failed to insert document: {}", err)),
        }
    } else {
        return get_error("Failed to create BSON document".to_string());
    }

    (StatusCode::OK, Json(Output { success: true })).into_response()
}
