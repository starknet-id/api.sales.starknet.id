use std::sync::Arc;

use crate::{
    models::AppState,
    utils::{get_error, to_hex},
};
use axum::{extract::State, response::IntoResponse, Json};
use reqwest::StatusCode;
use serde_derive::{Deserialize, Serialize};
use starknet::core::types::Felt;

#[derive(Deserialize)]
pub struct MailSubscribeQuery {
    tx_hash: Felt,
    groups: Vec<String>,
}

#[derive(Serialize)]
pub struct MailSubscribeDoc {
    #[serde(serialize_with = "field_element_to_hex")]
    tx_hash: Felt,
    group: String,
}

fn field_element_to_hex<S>(fe: &Felt, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&to_hex(*fe))
}

#[derive(Serialize)]
pub struct Output {
    success: bool,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(query): Json<MailSubscribeQuery>,
) -> impl IntoResponse {
    let emails_collection = state
        .db
        .collection::<mongodb::bson::Document>("email_groups");

    for group in query.groups {
        let bson_doc = match mongodb::bson::to_bson(&MailSubscribeDoc {
            tx_hash: query.tx_hash,
            group,
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
            match emails_collection.insert_one(document, None).await {
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
    }

    (StatusCode::OK, Json(Output { success: true })).into_response()
}
