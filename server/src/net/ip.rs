// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::RegionId;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use hyper::http::HeaderValue;
use hyper::HeaderMap;
use log::{info, warn};
use reqwest::Client;
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::net::{AddrParseError, IpAddr, Ipv4Addr};
use std::str::FromStr;
use std::sync::LazyLock;
use std::time::Duration;

pub fn ip_to_region_id(ip: IpAddr) -> Option<RegionId> {
    use db_ip::{include_region_database, DbIpDatabase, Region};

    static DB_IP: LazyLock<DbIpDatabase<Region>> = LazyLock::new(|| include_region_database!());

    /// Convert from [`db_ip::Region`] to [`RegionId`].
    /// The mapping is one-to-one, since the types mirror each other.
    fn region_to_region_id(region: Region) -> RegionId {
        match region {
            Region::Africa => RegionId::Africa,
            Region::Asia => RegionId::Asia,
            Region::Europe => RegionId::Europe,
            Region::NorthAmerica => RegionId::NorthAmerica,
            Region::Oceania => RegionId::Oceania,
            Region::SouthAmerica => RegionId::SouthAmerica,
        }
    }

    DB_IP.get(&ip).map(region_to_region_id)
}

pub trait IpAddrType: Hash + Eq + Copy + Display + FromStr<Err = AddrParseError> {
    /// URLs that return this type of address.
    const CHECKERS: &'static [&'static str];
}

impl IpAddrType for Ipv4Addr {
    const CHECKERS: &'static [&'static str] = &[
        "https://v4.ident.me/",
        "https://v4.tnedi.me/",
        "https://ipecho.net/plain",
        "https://ifconfig.me/ip",
        "https://icanhazip.com/",
        "https://ipinfo.io/ip",
        "https://api.ipify.org/",
    ];
}

/// Gets public ip by consulting various 3rd party APIs.
pub async fn get_own_public_ip<IP: IpAddrType>() -> Option<IP> {
    let mut default_headers = HeaderMap::new();

    default_headers.insert(
        reqwest::header::CONNECTION,
        HeaderValue::from_str("close").unwrap(),
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(1))
        .http1_only()
        .default_headers(default_headers)
        .build()
        .ok()?;

    let checkers = IP::CHECKERS;

    let mut checks: FuturesUnordered<_> = checkers
        .iter()
        .map(move |&checker| {
            let client = client.clone();
            let request_result = client.get(checker).build();

            async move {
                let request = request_result.ok()?;
                let fut = client.execute(request);

                let response = match fut.await {
                    Ok(response) => response,
                    Err(e) => {
                        info!("checker {} returned {:?}", checker, e);
                        return None;
                    }
                };

                let string = match response.text().await {
                    Ok(string) => string,
                    Err(e) => {
                        info!("checker {} returned {:?}", checker, e);
                        return None;
                    }
                };

                match IP::from_str(string.trim()) {
                    Ok(ip) => Some(ip),
                    Err(e) => {
                        info!("checker {} returned {:?}", checker, e);
                        None
                    }
                }
            }
        })
        .collect();

    // We pick the most common API response.
    let mut guesses = HashMap::new();
    let mut max = 0;
    let mut arg_max = None;

    while let Some(check) = checks.next().await {
        if let Some(ip_address) = check {
            let entry = guesses.entry(ip_address).or_insert(0);
            *entry += 1;
            if *entry > max {
                max = *entry;
                arg_max = Some(ip_address);
            }
        }
    }

    if let Some(ip) = arg_max {
        info!(
            "got public IP {ip} (confirmed by {max}/{} 3rd parties)",
            checkers.len()
        );
    } else {
        warn!("failed to get public IP");
    }

    arg_max
}
