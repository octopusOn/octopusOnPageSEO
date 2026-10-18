use std::fmt;

use octopuson_core::Timestamp;

macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(i64);

        impl $name {
            /// Wraps a raw database id.
            pub const fn new(raw: i64) -> Self {
                Self(raw)
            }

            /// The raw database id.
            pub const fn get(self) -> i64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

id_type!(
    /// Identifier of a [`Client`].
    ClientId
);
id_type!(
    /// Identifier of a [`Site`].
    SiteId
);

/// An agency customer owning one or more sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    /// Identifier.
    pub id: ClientId,
    /// Unique display name.
    pub name: String,
    /// Creation time.
    pub created_at: Timestamp,
}

/// A website whose logs are imported and analysed together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    /// Identifier; also names the site's data directory.
    pub id: SiteId,
    /// Owning client.
    pub client_id: ClientId,
    /// Owning client's name.
    pub client_name: String,
    /// Unique display name.
    pub name: String,
    /// Serialized log format choice (interpreted by the application layer).
    pub format_spec: String,
    /// Creation time.
    pub created_at: Timestamp,
    /// Serialized site settings (JSON, interpreted by the application layer).
    pub settings: String,
}

/// Input for [`Catalog::add_site`](super::Catalog::add_site).
#[derive(Debug, Clone, Copy)]
pub struct NewSite<'a> {
    /// Name of an existing client.
    pub client_name: &'a str,
    /// Unique site name.
    pub name: &'a str,
    /// Serialized log format choice.
    pub format_spec: &'a str,
}

/// A query saved under a name for reuse on any site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedQuery {
    /// Identifier.
    pub id: i64,
    /// Unique name.
    pub name: String,
    /// Free-text description.
    pub description: String,
    /// Query-language text.
    pub query: String,
    /// Creation time.
    pub created_at: Timestamp,
    /// Last change.
    pub updated_at: Timestamp,
}
