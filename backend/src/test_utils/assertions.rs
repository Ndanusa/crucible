//! API TEST CLIENT APPROACH: Option A — builder pattern per request
//! Rationale: Dedicated assertion helpers keep tests extremely dense, readable, and consistent when dealing with standard payload shapes.

use crate::test_utils::client::TestResponse;
use http::StatusCode;
use serde::de::DeserializeOwned;

/// Represents a standard paginated response envelope.
#[derive(serde::Deserialize, Debug)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: i64,
}

/// Represents a standard validation error payload.
#[derive(serde::Deserialize, Debug)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

/// Represents a standard error envelope.
#[derive(serde::Deserialize, Debug)]
pub struct ErrorResponse {
    pub error: String,
    pub errors: Option<Vec<ValidationError>>,
}

/// Asserts that a 422 Unprocessable Entity response contains an error for the given field.
pub fn assert_validation_error(response: TestResponse, expected_field: &str) {
    let resp = response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    let payload: ErrorResponse = resp.json();

    let has_field = payload
        .errors
        .unwrap_or_default()
        .iter()
        .any(|e| e.field == expected_field);

    if !has_field {
        panic!(
            "Validation error for field '{}' was not found in response.",
            expected_field
        );
    }
}

/// Asserts a 401 Unauthorized response with the correct envelope.
pub fn assert_unauthorized(response: TestResponse) {
    let resp = response.assert_status(StatusCode::UNAUTHORIZED);
    let payload: ErrorResponse = resp.json();

    assert!(
        !payload.error.is_empty(),
        "Expected non-empty error message in 401 response"
    );
}

/// Asserts a 404 Not Found response.
pub fn assert_not_found(response: TestResponse) {
    response.assert_status(StatusCode::NOT_FOUND);
}

/// Deserializes and validates a paginated response envelope.
pub fn assert_paginated<T: DeserializeOwned>(response: TestResponse) -> PaginatedResponse<T> {
    let resp = response.assert_status(StatusCode::OK);
    resp.json::<PaginatedResponse<T>>()
}
