use std::{net::IpAddr, path::PathBuf, time::Duration};

use anyhow::{Context as _, ensure};

use super::crypto;

#[derive(Clone)]
pub struct AuthConfig {
    pub public_origin: String,
    pub app_origins: Vec<String>,
    pub secure_cookie: bool,
    pub ticket_key: Vec<u8>,
    pub trusted_proxies: Vec<IpAddr>,
    pub native_version: (u32, u32, u32),
    pub mail: MailConfig,
    pub captcha: Option<(String, String)>,
}

impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthConfig")
            .field("public_origin", &self.public_origin)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub enum MailConfig {
    File(PathBuf),
    Smtp {
        host: String,
        port: u16,
        username: String,
        password: String,
        from: String,
    },
}

pub fn version(value: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<_> = value
        .split('.')
        .map(str::parse::<u32>)
        .collect::<Result<_, _>>()
        .ok()?;
    match parts.as_slice() {
        [a, b, c] => Some((*a, *b, *c)),
        _ => None,
    }
}

fn origin(value: &str) -> anyhow::Result<bool> {
    let url = reqwest::Url::parse(value).context("invalid auth origin")?;
    ensure!(
        url.origin().ascii_serialization() == value
            && url.username().is_empty()
            && url.password().is_none(),
        "auth origins must be exact http(s) origins without path or credentials"
    );
    let local = url.host_str().is_some_and(|host| {
        host == "localhost"
            || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
            || host == "[::1]"
    });
    ensure!(
        url.scheme() == "https" || (url.scheme() == "http" && local),
        "auth requires HTTPS except on loopback"
    );
    Ok(local)
}

impl AuthConfig {
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let var = |name| lookup(name).filter(|value| !value.is_empty());
        let public_origin = var("PUBLIC_ORIGIN").context("PUBLIC_ORIGIN is required")?;
        let local = origin(&public_origin)?;
        let secure_cookie = public_origin.starts_with("https://");
        let app_origins: Vec<String> = var("APP_ORIGINS")
            .unwrap_or_else(|| "https://localhost".into())
            .split(',')
            .map(|s| s.trim().to_owned())
            .collect();
        for app in &app_origins {
            origin(app)?;
            ensure!(app.starts_with("https://"), "app origin requires HTTPS");
        }
        ensure!(
            !app_origins.contains(&public_origin),
            "web and app origins must differ"
        );
        let ticket_key = match var("WS_TICKET_KEY") {
            Some(value) => {
                crypto::unhex(&value).context("WS_TICKET_KEY requires 64 hexadecimal characters")?
            }
            None if local => crypto::unhex(&crypto::token()?).expect("generated key"),
            None => anyhow::bail!("WS_TICKET_KEY is required outside loopback"),
        };
        let mail = match var("MAIL_MODE").as_deref() {
            None | Some("file") if local => MailConfig::File(
                var("MAIL_OUTBOX")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| "target/mail".into()),
            ),
            None | Some("smtp") => MailConfig::Smtp {
                host: var("SMTP_HOST").context("SMTP_HOST is required")?,
                port: var("SMTP_PORT")
                    .map(|p| p.parse())
                    .transpose()
                    .context("invalid SMTP_PORT")?
                    .unwrap_or(587),
                username: var("SMTP_USER").context("SMTP_USER is required")?,
                password: var("SMTP_PASSWORD").context("SMTP_PASSWORD is required")?,
                from: var("SMTP_FROM").context("SMTP_FROM is required")?,
            },
            _ => {
                anyhow::bail!("MAIL_MODE=file is allowed only on loopback; use smtp in production")
            }
        };
        let captcha = match (var("TURNSTILE_SITE_KEY"), var("TURNSTILE_SECRET")) {
            (Some(site), Some(secret)) => Some((site, secret)),
            (None, None) if local => None,
            _ => anyhow::bail!(
                "TURNSTILE_SITE_KEY and TURNSTILE_SECRET are required outside loopback"
            ),
        };
        let trusted_proxies = var("TRUSTED_PROXY_IPS")
            .map(|s| {
                s.split(',')
                    .map(|s| s.trim().parse())
                    .collect::<Result<Vec<IpAddr>, _>>()
            })
            .transpose()
            .context("invalid TRUSTED_PROXY_IPS")?
            .unwrap_or_default();
        let native_version =
            version(&var("APP_MIN_NATIVE_VERSION").unwrap_or_else(|| "1.0.0".into()))
                .context("APP_MIN_NATIVE_VERSION requires major.minor.patch")?;
        Ok(Self {
            public_origin,
            app_origins,
            secure_cookie,
            ticket_key,
            trusted_proxies,
            native_version,
            mail,
            captcha,
        })
    }

    pub fn local() -> Self {
        Self::from_lookup(|key| (key == "PUBLIC_ORIGIN").then(|| "http://localhost:5173".into()))
            .expect("local auth config")
    }
    pub fn session_age(&self) -> Duration {
        Duration::from_secs(30 * 86400)
    }
    pub fn cookie_name(&self) -> &'static str {
        if self.secure_cookie {
            "__Host-game_session"
        } else {
            "game_session"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn non_loopback_auth_requires_https_secrets_captcha_and_smtp() {
        assert!(
            AuthConfig::from_lookup(
                |key| (key == "PUBLIC_ORIGIN").then(|| "http://game.example".into())
            )
            .is_err()
        );
        assert!(
            AuthConfig::from_lookup(
                |key| (key == "PUBLIC_ORIGIN").then(|| "https://game.example".into())
            )
            .is_err()
        );
        let config = AuthConfig::from_lookup(|key| {
            Some(match key {
                "PUBLIC_ORIGIN" => "https://game.example".into(),
                "WS_TICKET_KEY" => "ab".repeat(32),
                "SMTP_HOST" => "smtp.example.test".into(),
                "SMTP_USER" => "user".into(),
                "SMTP_PASSWORD" => "secret".into(),
                "SMTP_FROM" => "game@example.test".into(),
                "TURNSTILE_SITE_KEY" => "site".into(),
                "TURNSTILE_SECRET" => "secret".into(),
                _ => return None,
            })
        })
        .unwrap();
        assert!(config.secure_cookie);
        assert!(matches!(config.mail, MailConfig::Smtp { .. }));
        assert!(!format!("{config:?}").contains("secret"));
        assert!(
            AuthConfig::from_lookup(|key| match key {
                "PUBLIC_ORIGIN" => Some("http://localhost:5173/path".into()),
                _ => None,
            })
            .is_err()
        );
    }
}
