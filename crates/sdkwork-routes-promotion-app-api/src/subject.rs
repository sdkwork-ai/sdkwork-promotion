use axum::Extension;
use sdkwork_iam_context_service::IamAppContext;
use sdkwork_web_core::WebRequestContext;

#[derive(Debug, Clone)]
pub(crate) struct AppRuntimeSubject {
    pub tenant_id: String,
    pub organization_id: Option<String>,
    pub user_id: String,
}

impl AppRuntimeSubject {
    /// Catalogue subject for an anonymous storefront visitor: the platform
    /// public tenant owns the shared offer catalogue, and there is no user.
    fn public_catalog(tenant_id: String) -> Self {
        Self {
            tenant_id,
            organization_id: None,
            user_id: String::new(),
        }
    }
}

pub(crate) fn app_runtime_subject_from_extension(
    context: Option<Extension<IamAppContext>>,
) -> Result<AppRuntimeSubject, String> {
    let Some(Extension(context)) = context else {
        return Err("authenticated runtime context is required".to_owned());
    };
    app_runtime_subject_from_iam(&context)
}

pub(crate) fn app_runtime_subject_from_iam(
    context: &IamAppContext,
) -> Result<AppRuntimeSubject, String> {
    let tenant_id = required_context_text(&context.tenant_id, "tenant_id")?;
    let user_id = required_context_text(&context.user_id, "user_id")?;
    let organization_id = context
        .organization_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);

    Ok(AppRuntimeSubject {
        tenant_id,
        organization_id,
        user_id,
    })
}

/// Tenant id owning the public storefront catalogue, used when an
/// optional-credential route is called without a signed-in principal
/// (override via `PROMOTION_PUBLIC_TENANT_ID`).
pub(crate) fn public_tenant_id() -> String {
    std::env::var("PROMOTION_PUBLIC_TENANT_ID").unwrap_or_else(|_| "100001".to_owned())
}

/// Resolve the subject for optional-credential (`dual-token-or-anonymous`)
/// catalogue reads: the signed-in principal when credentials were presented,
/// otherwise the platform public tenant.
pub(crate) fn catalog_runtime_subject(context: Option<&WebRequestContext>) -> AppRuntimeSubject {
    let Some(principal) = context.and_then(|context| context.principal()) else {
        return AppRuntimeSubject::public_catalog(public_tenant_id());
    };

    AppRuntimeSubject {
        tenant_id: principal.tenant_id().to_owned(),
        organization_id: principal
            .organization_id()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
        user_id: principal.user_id().to_owned(),
    }
}

fn required_context_text(value: &str, field_name: &'static str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!(
            "authenticated runtime context {field_name} is required"
        ));
    }
    Ok(value.to_owned())
}
