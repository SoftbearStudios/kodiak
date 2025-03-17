// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use rcgen::PKCS_ECDSA_P256_SHA256;
use std::fs;
use std::io::{BufReader, Cursor};
use std::path::Path;
use std::time::SystemTime;
use time::{Duration, OffsetDateTime};

const CERTIFICATE_PATH: &str = "../server/src/net/certificate.pem";
const PRIVATE_KEY_PATH: &str = "../server/src/net/private_key.pem";
const DIGEST_PATH: &str = "../client/src/net/certificate_hash.bin";

fn main() {
    // https://stackoverflow.com/a/76743504
    // println!("cargo:rerun-if-changed=ALWAYS_REBUILD");

    fn invalid(path: &str) -> bool {
        if !Path::new(path).exists() {
            return true;
        }
        let now = SystemTime::now();
        let modification = fs::metadata(path).unwrap().modified().unwrap();
        let duration = now.duration_since(modification).unwrap();
        if duration > std::time::Duration::from_secs(7 * 24 * 60 * 60) {
            return true;
        }
        false
    }

    if invalid(CERTIFICATE_PATH) || invalid(PRIVATE_KEY_PATH) || invalid(DIGEST_PATH) {
        println!("cargo:warning=regenerating certificate");
        let mut names = vec!["localhost".to_owned()];
        if let Ok(hostname) = gethostname::gethostname().into_string() {
            names.push(hostname);
        }
        let mut params = rcgen::CertificateParams::new(names);
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Constrained(2));
        params.alg = &PKCS_ECDSA_P256_SHA256;
        params.not_before = OffsetDateTime::now_utc();
        // WebTransport allows 2w.
        params.not_after = OffsetDateTime::now_utc()
            .checked_add(Duration::days(7))
            .unwrap();
        let cert = rcgen::Certificate::from_params(params).unwrap();
        let pem = cert.serialize_pem().unwrap().into_bytes();
        fs::write(CERTIFICATE_PATH, &pem).unwrap();
        fs::write(
            PRIVATE_KEY_PATH,
            cert.serialize_private_key_pem().into_bytes(),
        )
        .unwrap();
        // go via PEM, like the server, or the hash doesn't match :(
        let cert2 = rustls_pemfile::certs(&mut BufReader::new(Cursor::new(pem)))
            .unwrap()
            .remove(0);
        let digest = x509_certificate::DigestAlgorithm::Sha256.digest_data(&cert2);
        fs::write(DIGEST_PATH, digest).unwrap();
    }
}
