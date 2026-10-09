use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                if value.trim().is_empty() || value.len() > 256 {
                    return Err(serde::de::Error::custom(
                        "entity IDs must contain 1..256 bytes",
                    ));
                }
                Ok(Self(value))
            }
        }

        impl $name {
            #[must_use]
            pub fn new_random() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}

string_id!(ProjectId);
string_id!(Revision);
string_id!(DefinitionId);
string_id!(ChildPlacementId);
string_id!(NoteId);
string_id!(TrackId);
string_id!(InstanceId);
string_id!(ModifierId);
string_id!(ProposalId);
string_id!(HunkId);
string_id!(HistoryId);
string_id!(AssetId);
string_id!(RegionId);

impl Revision {
    #[must_use]
    pub fn initial() -> Self {
        Self::from("rev-0")
    }
}
