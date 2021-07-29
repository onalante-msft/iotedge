// Copyright (c) Microsoft. All rights reserved.

pub(crate) struct Route<M>
where
    M: edgelet_core::ModuleRuntime + Send + Sync,
{
    client: std::sync::Arc<futures_util::lock::Mutex<aziot_cert_client_async::Client>>,
    trust_bundle: String,
    _runtime: std::marker::PhantomData<M>,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct TrustBundleResponse {
    certificate: String,
}

#[async_trait::async_trait]
impl<M> http_common::server::Route for Route<M>
where
    M: edgelet_core::ModuleRuntime + Send + Sync,
{
    type ApiVersion = edgelet_http::ApiVersion;
    fn api_version() -> &'static dyn http_common::DynRangeBounds<Self::ApiVersion> {
        &((edgelet_http::ApiVersion::V2018_06_28)..)
    }

    type Service = crate::Service<M>;
    fn from_uri(
        service: &Self::Service,
        path: &str,
        _query: &[(std::borrow::Cow<'_, str>, std::borrow::Cow<'_, str>)],
        _extensions: &http::Extensions,
    ) -> Option<Self> {
        let trust_bundle = match path {
            "/trust-bundle" => service.config.trust_bundle.clone(),
            "/manifest-trust-bundle" => service.config.manifest_trust_bundle.clone(),
            _ => return None,
        };

        Some(Route {
            client: service.cert_client.clone(),
            trust_bundle,
            _runtime: std::marker::PhantomData,
        })
    }

    type GetResponse = TrustBundleResponse;
    async fn get(self) -> http_common::server::RouteResponse<Self::GetResponse> {
        let client = self.client.lock().await;

        let certificate = client.get_cert(&self.trust_bundle).await.map_err(|_| {
            edgelet_http::error::not_found(format!("certificate {} not found", self.trust_bundle))
        })?;

        let certificate = std::str::from_utf8(&certificate)
            .map_err(|err| {
                edgelet_http::error::server_error(format!("could not parse certificate: {}", err))
            })?
            .to_string();

        Ok((http::StatusCode::OK, TrustBundleResponse { certificate }))
    }

    type DeleteBody = serde::de::IgnoredAny;
    type DeleteResponse = ();

    type PostBody = serde::de::IgnoredAny;
    type PostResponse = ();

    type PutBody = serde::de::IgnoredAny;
    type PutResponse = ();
}