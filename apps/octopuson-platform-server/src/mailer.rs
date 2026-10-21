//! Report e-mails over SMTP (TLS), with the sender and server from the settings.

use std::time::Duration;

use lettre::message::header::ContentType;
use lettre::message::{Attachment, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::extension::ClientId;
use lettre::{Message, SmtpTransport, Transport};
use octopuson_platform_app::mail::{Mail, Mailer};

/// How long the SMTP server is waited for.
const TIMEOUT: Duration = Duration::from_secs(30);

/// Sends through one SMTP server as one sender.
pub struct SmtpMailer {
    transport: SmtpTransport,
    from: Mailbox,
}

impl std::fmt::Debug for SmtpMailer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmtpMailer")
            .field("from", &self.from)
            .finish_non_exhaustive()
    }
}

impl SmtpMailer {
    /// A mailer for `url` (`smtps://…:465`, or `smtp://…:587?tls=required`) sending as `from`,
    /// greeting the server as `hello` (the public host).
    ///
    /// # Errors
    /// A message naming the setting that cannot be used.
    pub fn new(url: &str, from: &str, hello: &str) -> Result<Self, String> {
        if !(url.starts_with("smtps://") || url.contains("tls=required")) {
            return Err(
                "PLATFORM_SMTP_URL: use smtps:// or smtp://…?tls=required, never plain text"
                    .to_owned(),
            );
        }
        let transport = SmtpTransport::from_url(url)
            .map_err(|error| format!("PLATFORM_SMTP_URL: {error}"))?
            .hello_name(ClientId::Domain(hello.to_owned()))
            .timeout(Some(TIMEOUT))
            .build();
        let from = from
            .parse()
            .map_err(|error| format!("PLATFORM_MAIL_FROM: {error}"))?;
        Ok(Self { transport, from })
    }

    /// The message as it is sent.
    fn message(&self, mail: &Mail) -> Result<Message, String> {
        let mut builder = Message::builder()
            .from(self.from.clone())
            .subject(mail.subject.clone());
        for to in &mail.to {
            let to: Mailbox = to
                .parse()
                .map_err(|error| format!("recipient {to}: {error}"))?;
            builder = builder.to(to);
        }
        if let Some(reply_to) = mail
            .reply_to
            .as_deref()
            .and_then(|address| address.parse().ok())
        {
            builder = builder.reply_to(reply_to);
        }
        let text = SinglePart::plain(mail.text.clone());
        let body = match &mail.attachment {
            Some(file) => {
                let kind = ContentType::parse(&file.content_type)
                    .map_err(|error| format!("attachment type: {error}"))?;
                MultiPart::mixed()
                    .singlepart(text)
                    .singlepart(Attachment::new(file.name.clone()).body(file.bytes.clone(), kind))
            }
            None => MultiPart::mixed().singlepart(text),
        };
        builder.multipart(body).map_err(|error| error.to_string())
    }
}

impl Mailer for SmtpMailer {
    fn send(&self, mail: &Mail) -> Result<(), String> {
        let message = self.message(mail)?;
        self.transport
            .send(&message)
            .map(drop)
            .map_err(|error| format!("the e-mail was not sent: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use octopuson_platform_app::mail::{Attachment as File, Mail};

    use super::SmtpMailer;

    #[test]
    fn builds_messages_and_refuses_plain_text_servers() {
        assert!(
            SmtpMailer::new(
                "smtp://user:pass@mail.example.com:25",
                "a@b.com",
                "app.example.com"
            )
            .is_err()
        );
        assert!(
            SmtpMailer::new(
                "smtps://user:pass@mail.example.com:465",
                "not an address",
                "app.example.com"
            )
            .is_err()
        );
        let mailer = SmtpMailer::new(
            "smtps://user:pass@mail.example.com:465",
            "OctopusOn SEO <rapor@octopuson.com>",
            "app.example.com",
        )
        .unwrap();
        let message = mailer
            .message(&Mail {
                to: vec!["musteri@example.com".to_owned()],
                reply_to: Some("ajans@example.com".to_owned()),
                subject: "Aylık rapor · ornekmagaza.com".to_owned(),
                text: "Rapor ektedir.".to_owned(),
                attachment: Some(File {
                    name: "aylik-rapor.pdf".to_owned(),
                    content_type: "application/pdf".to_owned(),
                    bytes: b"%PDF-1.7".to_vec(),
                }),
            })
            .unwrap();
        let raw = String::from_utf8_lossy(&message.formatted()).into_owned();
        assert!(raw.contains("To: musteri@example.com"));
        assert!(raw.contains("Reply-To: ajans@example.com"));
        assert!(raw.contains("aylik-rapor.pdf"));
        assert!(raw.contains("application/pdf"));
        assert!(
            mailer
                .message(&Mail {
                    to: vec!["a@b.com\r\nBcc: x@y.com".to_owned()],
                    reply_to: None,
                    subject: "s".to_owned(),
                    text: "t".to_owned(),
                    attachment: None,
                })
                .is_err(),
            "a recipient cannot add headers"
        );
    }
}
