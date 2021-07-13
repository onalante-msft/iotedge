// Copyright (c) Microsoft. All rights reserved.

pub(crate) struct Route {
    module_id: String,
    gen_id: String,
    pid: libc::pid_t,
    api: super::CertApi,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ServerCertificateRequest {
    #[serde(rename = "commonName")]
    common_name: String,
}

#[async_trait::async_trait]
impl http_common::server::Route for Route {
    type ApiVersion = edgelet_http::ApiVersion;
    fn api_version() -> &'static dyn http_common::DynRangeBounds<Self::ApiVersion> {
        &((edgelet_http::ApiVersion::V2018_06_28)..)
    }

    type Service = crate::Service;
    fn from_uri(
        service: &Self::Service,
        path: &str,
        _query: &[(std::borrow::Cow<'_, str>, std::borrow::Cow<'_, str>)],
        extensions: &http::Extensions,
    ) -> Option<Self> {
        let uri_regex = regex::Regex::new(
            "^/modules/(?P<moduleId>[^/]+)/genid/(?P<genId>[^/]+)/certificate/server$",
        )
        .expect("hard-coded regex must compile");
        let captures = uri_regex.captures(path)?;

        let module_id = &captures["moduleId"];
        let module_id = percent_encoding::percent_decode_str(module_id)
            .decode_utf8()
            .ok()?;

        let gen_id = &captures["genId"];
        let gen_id = percent_encoding::percent_decode_str(gen_id)
            .decode_utf8()
            .ok()?;

        let pid = match extensions.get::<Option<libc::pid_t>>().cloned().flatten() {
            Some(pid) => pid,
            None => return None,
        };

        let api = super::CertApi::new(
            service.key_connector.clone(),
            service.key_client.clone(),
            service.cert_client.clone(),
            &service.config,
        );

        Some(Route {
            module_id: module_id.into_owned(),
            gen_id: gen_id.into_owned(),
            pid,
            api,
        })
    }

    type GetResponse = ();

    type DeleteBody = serde::de::IgnoredAny;
    type DeleteResponse = ();

    type PostBody = ServerCertificateRequest;
    type PostResponse = super::CertificateResponse;
    async fn post(
        self,
        body: Option<Self::PostBody>,
    ) -> http_common::server::RouteResponse<Option<Self::PostResponse>> {
        edgelet_http::auth_caller(&self.module_id, self.pid)?;

        let common_name = match body {
            Some(body) => body.common_name,
            None => return Err(edgelet_http::error::bad_request("missing request body")),
        };

        let cert_id = format!(
            "aziot-edged/module/{}:{}:server",
            &self.module_id, &self.gen_id
        );

        todo!()
    }

    type PutBody = serde::de::IgnoredAny;
    type PutResponse = ();
}
