use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use sdkwork_utils_rust::{
    offset_list_page_info, validated_offset_list_params, OffsetListPageParams, SdkWorkApiResponse,
    SdkWorkCommandData, SdkWorkPageData, SdkWorkProblemDetail, SdkWorkResourceData,
    SdkWorkResultCode, MAX_LIST_PAGE_SIZE,
};
use sdkwork_web_core::WebRequestContext;

fn trace_id(context: Option<&WebRequestContext>) -> String {
    context
        .and_then(|value| value.trace_id.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(sdkwork_utils_rust::uuid)
}

pub(crate) fn success_item<T: serde::Serialize>(
    context: Option<&WebRequestContext>,
    item: T,
) -> Response {
    let trace_id = trace_id(context);
    let envelope = SdkWorkApiResponse::success(SdkWorkResourceData { item }, trace_id.clone());
    attach_trace((StatusCode::OK, Json(envelope)).into_response(), &trace_id)
}

pub(crate) fn success_created<T: serde::Serialize>(
    context: Option<&WebRequestContext>,
    item: T,
) -> Response {
    let trace_id = trace_id(context);
    let envelope = SdkWorkApiResponse::success(SdkWorkResourceData { item }, trace_id.clone());
    attach_trace(
        (StatusCode::CREATED, Json(envelope)).into_response(),
        &trace_id,
    )
}

pub(crate) fn no_content(context: Option<&WebRequestContext>) -> Response {
    let trace_id = trace_id(context);
    attach_trace(StatusCode::NO_CONTENT.into_response(), &trace_id)
}

pub(crate) fn success_items<T: serde::Serialize>(
    context: Option<&WebRequestContext>,
    items: Vec<T>,
    total_items: i64,
    params: OffsetListPageParams,
) -> Response {
    let trace_id = trace_id(context);
    let envelope = SdkWorkApiResponse::success(
        SdkWorkPageData {
            items,
            page_info: offset_list_page_info(total_items, params),
        },
        trace_id.clone(),
    );
    attach_trace((StatusCode::OK, Json(envelope)).into_response(), &trace_id)
}

pub(crate) fn success_command(
    context: Option<&WebRequestContext>,
    resource_id: String,
    status: String,
) -> Response {
    let trace_id = trace_id(context);
    let envelope = SdkWorkApiResponse::success(
        SdkWorkCommandData {
            accepted: true,
            resource_id: Some(resource_id),
            status: Some(status),
        },
        trace_id.clone(),
    );
    attach_trace((StatusCode::OK, Json(envelope)).into_response(), &trace_id)
}

pub(crate) fn parse_page(
    context: Option<&WebRequestContext>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<OffsetListPageParams, Box<Response>> {
    validated_offset_list_params(page, page_size).map_err(|_| {
        Box::new(validation(
            context,
            format!("page must be >= 1 and page_size must be between 1 and {MAX_LIST_PAGE_SIZE}"),
        ))
    })
}

pub(crate) fn unauthorized(
    context: Option<&WebRequestContext>,
    detail: impl Into<String>,
) -> Response {
    problem(
        context,
        StatusCode::UNAUTHORIZED,
        SdkWorkResultCode::AuthenticationRequired,
        detail,
    )
}

pub(crate) fn forbidden(
    context: Option<&WebRequestContext>,
    detail: impl Into<String>,
) -> Response {
    problem(
        context,
        StatusCode::FORBIDDEN,
        SdkWorkResultCode::PermissionRequired,
        detail,
    )
}

pub(crate) fn validation(
    context: Option<&WebRequestContext>,
    detail: impl Into<String>,
) -> Response {
    problem(
        context,
        StatusCode::BAD_REQUEST,
        SdkWorkResultCode::ValidationError,
        detail,
    )
}

pub(crate) fn not_found(
    context: Option<&WebRequestContext>,
    detail: impl Into<String>,
) -> Response {
    problem(
        context,
        StatusCode::NOT_FOUND,
        SdkWorkResultCode::NotFound,
        detail,
    )
}

pub(crate) fn internal_error(
    context: Option<&WebRequestContext>,
    detail: impl Into<String>,
) -> Response {
    problem(
        context,
        StatusCode::INTERNAL_SERVER_ERROR,
        SdkWorkResultCode::InternalError,
        detail,
    )
}

/// `409` for a domain conflict: stale optimistic-concurrency version, an
/// in-use resource, or a state that contradicts the request.
pub(crate) fn conflict(
    context: Option<&WebRequestContext>,
    detail: impl Into<String>,
) -> Response {
    problem(
        context,
        StatusCode::CONFLICT,
        SdkWorkResultCode::Conflict,
        detail,
    )
}

/// `422` for a well-formed request that cannot be applied to the current state.
pub(crate) fn invalid_state(
    context: Option<&WebRequestContext>,
    detail: impl Into<String>,
) -> Response {
    problem(
        context,
        StatusCode::UNPROCESSABLE_ENTITY,
        SdkWorkResultCode::UnprocessableEntity,
        detail,
    )
}

fn problem(
    context: Option<&WebRequestContext>,
    status: StatusCode,
    code: SdkWorkResultCode,
    detail: impl Into<String>,
) -> Response {
    let trace_id = trace_id(context);
    let body = SdkWorkProblemDetail::platform(code, detail, trace_id.clone());
    attach_trace(
        (
            status,
            [(axum::http::header::CONTENT_TYPE, "application/problem+json")],
            Json(body),
        )
            .into_response(),
        &trace_id,
    )
}

fn attach_trace(mut response: Response, trace_id: &str) -> Response {
    if let Ok(value) = HeaderValue::from_str(trace_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-sdkwork-trace-id"), value);
    }
    response
}
