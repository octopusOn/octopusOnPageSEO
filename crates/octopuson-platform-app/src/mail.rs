//! Sending e-mail is the host's: the web server sends with its SMTP settings, the desktop
//! program sends nothing. Reports reach their recipients through a [`Mailer`] the host gives
//! the workspace.

/// A file sent with a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// File name (`rapor.pdf`).
    pub name: String,
    /// Media type (`application/pdf`).
    pub content_type: String,
    /// Its bytes.
    pub bytes: Vec<u8>,
}

/// A message to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mail {
    /// Recipients.
    pub to: Vec<String>,
    /// Where answers go: the account's own address.
    pub reply_to: Option<String>,
    /// Subject.
    pub subject: String,
    /// Plain-text body.
    pub text: String,
    /// A file, if any.
    pub attachment: Option<Attachment>,
}

/// Shows system notifications (the desktop program).
pub trait Notifier: Send + Sync {
    /// Shows one notification; failures are the host's to ignore.
    fn notify(&self, title: &str, body: &str);
}

/// Sends messages.
pub trait Mailer: Send + Sync {
    /// Sends one message to all its recipients.
    ///
    /// # Errors
    /// A message for people when the message could not be handed over.
    fn send(&self, mail: &Mail) -> Result<(), String>;
}
