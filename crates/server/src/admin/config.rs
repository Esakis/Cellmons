use anyhow::{Context as _, ensure};
use ipnet::IpNet;
use std::net::IpAddr;

#[derive(Clone)]
pub struct Config {
    pub(super) key: [u8; 32],
    pub(super) allowed: Vec<IpNet>,
}
impl Config {
    pub fn new(key: [u8; 32], allowed: Vec<IpNet>) -> anyhow::Result<Self> {
        ensure!(
            !allowed.is_empty()
                && allowed.len() <= 64
                && allowed.iter().all(|net| net.prefix_len() > 0),
            "ADMIN_ALLOWED_IPS requires a bounded allowlist; /0 is forbidden"
        );
        Ok(Self { key, allowed })
    }
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> anyhow::Result<Option<Self>> {
        let key = lookup("ADMIN_TOTP_KEY").filter(|s| !s.is_empty());
        let allowed = lookup("ADMIN_ALLOWED_IPS").filter(|s| !s.is_empty());
        match (key, allowed) {
            (None, None) => Ok(None),
            (Some(key), Some(allowed)) => {
                let key = crate::auth::crypto::unhex(&key)
                    .context("ADMIN_TOTP_KEY requires 64 hexadecimal characters")?
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("invalid ADMIN_TOTP_KEY"))?;
                let allowed = allowed
                    .split(',')
                    .map(|s| {
                        let s = s.trim();
                        s.parse::<IpNet>()
                            .or_else(|_| s.parse::<IpAddr>().map(IpNet::from))
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .context("invalid ADMIN_ALLOWED_IPS")?;
                Self::new(key, allowed).map(Some)
            }
            _ => anyhow::bail!(
                "ADMIN_TOTP_KEY and ADMIN_ALLOWED_IPS must both be configured, or both omitted"
            ),
        }
    }
    pub(super) fn allows(&self, ip: IpAddr) -> bool {
        self.allowed
            .iter()
            .any(|net| net.contains(&ip.to_canonical()))
    }
}
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdminConfig")
            .field("allowed", &self.allowed)
            .finish_non_exhaustive()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_and_partial_configuration_fail_closed_and_never_print_the_key() {
        assert!(Config::from_lookup(|_| None).unwrap().is_none());
        assert!(
            Config::from_lookup(|key| (key == "ADMIN_TOTP_KEY").then(|| "ab".repeat(32))).is_err()
        );
        assert!(Config::new([1; 32], vec!["0.0.0.0/0".parse().unwrap()]).is_err());
        let config = Config::new(
            [0xab; 32],
            vec!["127.0.0.1/32".parse().unwrap(), "::1/128".parse().unwrap()],
        )
        .unwrap();
        assert!(config.allows("::ffff:127.0.0.1".parse().unwrap()));
        assert!(!config.allows("192.0.2.1".parse().unwrap()));
        assert!(!format!("{config:?}").contains(&"ab".repeat(32)));
    }
}
