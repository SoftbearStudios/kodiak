// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::{ArenaService, DomainDto, NonZeroUnixMillis, UnixTime};
use log::warn;
use rustls::server::{ClientHello, ServerConfig};
use rustls::sign::CertifiedKey;
use rustls::InconsistentKeys;
use rustls_pemfile::Item;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use std::io::{self};
use std::sync::Arc;
use x509_parser::prelude::X509Certificate;

pub fn load_domains<G: ArenaService>(
    domains: &[DomainDto],
) -> Option<(Arc<ServerConfig>, NonZeroUnixMillis)> {
    let mut certificates = domains
        .iter()
        .filter_map(|d| {
            let cert_chain = rustls_pemfile::certs(&mut d.certificate.as_bytes())
                .collect::<Result<Vec<CertificateDer<'static>>, io::Error>>()
                .ok()?;
            let private_key_item =
                rustls_pemfile::read_one(&mut d.private_key.as_bytes()).ok()??;
            let private_key_der = match private_key_item {
                Item::Pkcs1Key(key) => PrivateKeyDer::Pkcs1(key),
                Item::Pkcs8Key(key) => PrivateKeyDer::Pkcs8(key),
                Item::Sec1Key(key) => PrivateKeyDer::Sec1(key),
                _ => {
                    warn!("private key format not supported");
                    return None;
                }
            };
            let private_key = rustls::crypto::ring::default_provider()
                .key_provider
                .load_private_key(private_key_der)
                .ok()?;

            let expiration = cert_chain.first().and_then(|cert| {
                use x509_parser::prelude::FromDer;
                let (_, x509) = X509Certificate::from_der(cert.as_ref()).ok()?;
                let t = NonZeroUnixMillis::from_i64(x509.validity().not_after.timestamp() * 1000);
                Some(t)
            })?;

            let certified_key = CertifiedKey::new(cert_chain, private_key);
            match certified_key.keys_match() {
                // Don't treat unknown consistency as an error
                Ok(()) | Err(rustls::Error::InconsistentKeys(InconsistentKeys::Unknown)) => (),
                Err(_) => return None,
            }
            Some((
                d.domain.as_bytes().into(),
                Arc::new(certified_key),
                expiration,
            ))
        })
        .collect::<Vec<(Box<[u8]>, Arc<CertifiedKey>, NonZeroUnixMillis)>>();

    if let Some(default) = certificates
        .iter()
        .position(|c| &*c.0 == G::GAME_CONSTANTS.domain.as_bytes())
    {
        let (_, default, date_certificate_expiers) = certificates.swap_remove(default);
        let resolver = ResolvesServerCertUsingSniOrDefault {
            by_name: certificates.into_iter().map(|(n, k, _)| (n, k)).collect(),
            default,
        };

        let mut config = ServerConfig::builder()
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(resolver));

        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        Some((Arc::new(config), date_certificate_expiers))
    } else {
        None
    }
}

#[derive(Debug)]
struct ResolvesServerCertUsingSniOrDefault {
    by_name: Vec<(Box<[u8]>, Arc<rustls::sign::CertifiedKey>)>,
    default: Arc<rustls::sign::CertifiedKey>,
}

impl rustls::server::ResolvesServerCert for ResolvesServerCertUsingSniOrDefault {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        if let Some(server_name) = client_hello.server_name() {
            for (name, key) in &self.by_name {
                if server_name.as_bytes().ends_with(&**name) {
                    return Some(key.clone());
                }
            }
        }
        Some(self.default.clone())
    }
}
