use std::sync::{Arc, Mutex};

use lettre::{
    AsyncSmtpTransport, AsyncTransport as _, Message, Tokio1Executor,
    transport::smtp::authentication::Credentials,
};
use serde::Serialize;

use super::config::MailConfig;

#[derive(Clone, Serialize)]
pub struct Mail {
    pub to: String,
    pub subject: String,
    pub body: String,
}

#[derive(Clone)]
pub enum Mailer {
    File(std::path::PathBuf),
    Smtp {
        transport: AsyncSmtpTransport<Tokio1Executor>,
        from: String,
    },
    /// Explicit test transport; environment configuration can never select this variant.
    Capture(Arc<Mutex<Vec<Mail>>>),
}

impl Mailer {
    pub fn new(config: &MailConfig) -> anyhow::Result<Self> {
        Ok(match config {
            MailConfig::File(path) => Self::File(path.clone()),
            MailConfig::Smtp {
                host,
                port,
                username,
                password,
                from,
            } => {
                let _: lettre::message::Mailbox = from.parse()?;
                Self::Smtp {
                    transport: AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?
                        .port(*port)
                        .credentials(Credentials::new(username.clone(), password.clone()))
                        .timeout(Some(std::time::Duration::from_secs(10)))
                        .build(),
                    from: from.clone(),
                }
            }
        })
    }

    pub async fn send(&self, mail: Mail) -> anyhow::Result<()> {
        match self {
            Self::Capture(messages) => {
                messages.lock().expect("mail capture lock").push(mail);
            }
            Self::File(directory) => {
                tokio::fs::create_dir_all(directory).await?;
                let path = directory.join(format!("{}.json", uuid::Uuid::now_v7()));
                let mut options = tokio::fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                options.mode(0o600);
                let mut file = options.open(path).await?;
                use tokio::io::AsyncWriteExt as _;
                file.write_all(&serde_json::to_vec_pretty(&mail)?).await?;
            }
            Self::Smtp { transport, from } => {
                let message = Message::builder()
                    .from(from.parse()?)
                    .to(mail.to.parse()?)
                    .subject(mail.subject)
                    .body(mail.body)?;
                transport.send(message).await?;
            }
        }
        Ok(())
    }
}
