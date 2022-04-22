// Copyright (c) Microsoft. All rights reserved.
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use anyhow::Context;
use futures::{future, Future, IntoFuture, Stream};
use hyper::{Body, Request, Response};

use cert_client::client::CertificateClient;
use edgelet_core::{CertificateProperties, CertificateType, WorkloadConfig};
use edgelet_http::route::{Handler, Parameters};
use edgelet_utils::{ensure_not_empty, prepare_dns_san_entries};
use workload::models::ServerCertificateRequest;

use super::refresh_cert;
use crate::error::{CertOperation, Error};
use crate::IntoResponse;

pub struct ServerCertHandler<W: WorkloadConfig> {
    cert_client: Arc<Mutex<CertificateClient>>,
    key_client: Arc<aziot_key_client::Client>,
    config: W,
}

impl<W: WorkloadConfig> ServerCertHandler<W> {
    pub fn new(
        key_client: Arc<aziot_key_client::Client>,
        cert_client: Arc<Mutex<CertificateClient>>,
        config: W,
    ) -> Self {
        ServerCertHandler {
            key_client,
            cert_client,
            config,
        }
    }
}
impl<W> Handler<Parameters> for ServerCertHandler<W>
where
    W: WorkloadConfig + Clone + Send + Sync + 'static,
{
    fn handle(
        &self,
        req: Request<Body>,
        params: Parameters,
    ) -> Box<dyn Future<Item = Response<Body>, Error = anyhow::Error> + Send> {
        let cert_client = self.cert_client.clone();
        let key_client = self.key_client.clone();
        let cfg = self.config.clone();

        let response = params
            .name("name")
            .context(Error::MissingRequiredParameter("name"))
            .and_then(|module_id| {
                let module_id = module_id.to_string();
                params
                    .name("genid")
                    .context(Error::MissingRequiredParameter("genid"))
                    .map(|gen_id| {
                        let alias = format!(
                            "aziot-edged/module/{}:{}:server",
                            module_id,
                            gen_id.to_string()
                        );
                        (module_id, alias)
                    })
            })
            .into_future()
            .and_then(|(module_id, alias)| {
                req.into_body().concat2().then(move |body| {
                    let body =
                        body.context(Error::CertOperation(CertOperation::GetServerCert))?;
                    Ok((alias, body, module_id))
                })
            })
            .and_then(move |(alias, body, module_id)| {
                let cert_req: ServerCertificateRequest =
                    serde_json::from_slice(&body).context(Error::MalformedRequestBody)?;

                let common_name = cert_req.common_name();
                ensure_not_empty(common_name)
                .context(Error::MalformedRequestBody)?;

                // add a DNS SAN entry in the server cert that uses the module identifier as
                // an alternative DNS name; we also need to add the common_name that we are using
                // as a DNS name since the presence of a DNS name SAN will take precedence over
                // the common name
                let mut dns: Vec<String> =
                    prepare_dns_san_entries([&*module_id].iter().copied()).collect();

                let mut ip: Vec<String> = Vec::new();

                if IpAddr::from_str(common_name).is_ok() {
                    ip.push(common_name.clone());
                } else {
                    dns.push(common_name.clone());
                };

                #[allow(clippy::cast_sign_loss)]
                let props = CertificateProperties::new(
                    common_name.to_string(),
                    CertificateType::Server,
                    alias.clone(),
                )
                .with_dns_san_entries(dns)
                .with_ip_entries(ip);

                Ok((alias, props, cfg))
            })
            .and_then(move |(alias, props, cfg)| {
                let response = refresh_cert(
                    key_client,
                    cert_client,
                    alias,
                    &props,
                    super::EdgeCaCertificate {
                        cert_id: cfg.edge_ca_cert().to_string(),
                        key_id: cfg.edge_ca_key().to_string(),
                        device_id: cfg.device_id().to_string(),
                    },
                    Error::CertOperation(CertOperation::GetServerCert)
                )
                .map_err(|_| anyhow::anyhow!(Error::CertOperation(CertOperation::GetServerCert)));
                Ok(response)
            })
            .flatten()
            .or_else(|e| future::ok(e.into_response()));

        Box::new(response)
    }
}
