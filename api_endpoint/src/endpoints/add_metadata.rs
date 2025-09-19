use std::sync::Arc;

use crate::{
    models::AppState,
    utils::{get_error, get_specific_error},
};
use axum::{extract::State, response::IntoResponse, Json};
use reqwest::StatusCode;
use serde_derive::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize)]
pub struct AddMetadata {
    meta_hash: String,
    email: String,
    tax_state: String,
    salt: String,
}

fn compute_metadata_hash(email: &str, tax_state: &str, salt: &str) -> String {
    let separator = "|";
    let data = format!(
        "{}{}{}{}{}",
        email,
        separator,
        tax_state.replace("|", ""),
        separator,
        salt
    );

    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    let result = hasher.finalize();
    let hash_hex = hex::encode(result);

    // Truncate the last two characters (8 bits) to make it a 248-bit hash so it fits in a felt
    let truncated_hash_hex = &hash_hex[0..hash_hex.len() - 2];
    truncated_hash_hex.to_string()
}

#[derive(Serialize)]
pub struct Output {
    success: bool,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(query): Json<AddMetadata>,
) -> impl IntoResponse {
    let computed_meta_hash = compute_metadata_hash(&query.email, &query.tax_state, &query.salt);
    if computed_meta_hash != query.meta_hash {
        return get_specific_error(StatusCode::BAD_REQUEST, "unable to verify hash".to_string());
    }

    let metadata_collection = state.db.collection::<mongodb::bson::Document>("metadata");

    let bson_doc = match mongodb::bson::to_bson(&query) {
        Ok(bson) => bson,
        Err(err) => {
            state
                .logger
                .severe(format!("Failed to serialize to BSON: {}", err));
            return get_error("Internal server error".to_string());
        }
    };

    if let mongodb::bson::Bson::Document(document) = bson_doc {
        match metadata_collection.insert_one(document, None).await {
            Ok(_) => (),
            Err(err) => {
                state
                    .logger
                    .severe(format!("Failed to insert document: {}", err));
                return get_error("Internal server error".to_string());
            }
        }
    } else {
        state
            .logger
            .severe("Failed to create BSON document".to_string());
        return get_error("Internal server error".to_string());
    }

    (StatusCode::OK, Json(Output { success: true })).into_response()
}
