//! Server-Side Request Forgery (SSRF) protection.

use std::net::IpAddr;

pub struct SsrfGuard;

impl SsrfGuard {
    /// Validates destination URL against SSRF vulnerabilities.
    ///
    /// Blocks loopback, cloud metadata endpoints (169.254.169.254),
    /// and private networks in production mode unless explicitly allowlisted.
    pub fn validate_url(raw_url: &str, is_production: bool) -> Result<(), String> {
        let parsed =
            reqwest::Url::parse(raw_url).map_err(|e| format!("invalid destination URL: {e}"))?;

        let scheme = parsed.scheme();
        if scheme != "http" && scheme != "https" {
            return Err(format!(
                "forbidden scheme '{scheme}': only http and https are permitted"
            ));
        }

        let host_str = parsed
            .host_str()
            .ok_or_else(|| "missing host in URL".to_string())?;

        // 1. Check prohibited hostname keywords
        if host_str.eq_ignore_ascii_case("localhost") && is_production {
            return Err("access to localhost is prohibited in production mode".to_string());
        }

        // 2. Check IP address bounds
        if let Ok(ip) = host_str.parse::<IpAddr>() {
            if ip.is_loopback() && is_production {
                return Err("access to loopback addresses is prohibited in production".to_string());
            }

            // AWS / GCP / Azure Cloud Metadata IP
            if ip == "169.254.169.254".parse::<IpAddr>().unwrap() {
                return Err(
                    "access to cloud instance metadata service (169.254.169.254) is forbidden"
                        .to_string(),
                );
            }

            if is_production {
                match ip {
                    IpAddr::V4(v4) => {
                        let octets = v4.octets();
                        // 10.0.0.0/8
                        if octets[0] == 10 {
                            return Err(
                                "access to private IP range 10.0.0.0/8 is prohibited in production"
                                    .to_string(),
                            );
                        }
                        // 172.16.0.0/12
                        if octets[0] == 172 && (16..=31).contains(&octets[1]) {
                            return Err("access to private IP range 172.16.0.0/12 is prohibited in production".to_string());
                        }
                        // 192.168.0.0/16
                        if octets[0] == 192 && octets[1] == 168 {
                            return Err("access to private IP range 192.168.0.0/16 is prohibited in production".to_string());
                        }
                    }
                    IpAddr::V6(v6) => {
                        if v6.is_loopback() {
                            return Err(
                                "access to IPv6 loopback is prohibited in production".to_string()
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_cloud_metadata_ip() {
        assert!(
            SsrfGuard::validate_url("http://169.254.169.254/latest/meta-data/", false).is_err()
        );
        assert!(SsrfGuard::validate_url("http://169.254.169.254/latest/meta-data/", true).is_err());
    }

    #[test]
    fn blocks_loopback_in_production() {
        assert!(SsrfGuard::validate_url("http://127.0.0.1:8080/admin", true).is_err());
        assert!(SsrfGuard::validate_url("http://localhost:8080/admin", true).is_err());
    }

    #[test]
    fn allows_public_domains() {
        assert!(SsrfGuard::validate_url("https://api.dukcapil.gov.id/v1/verify", true).is_ok());
    }
}
