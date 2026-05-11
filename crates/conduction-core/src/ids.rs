use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! newtype_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            #[inline]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            #[inline]
            pub const fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }

            #[inline]
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

newtype_id!(TrackId);
newtype_id!(CueId);
newtype_id!(TemplateId);
newtype_id!(SetlistId);
newtype_id!(SetlistEntryId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let a = TrackId::new();
        let b = TrackId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn ids_roundtrip_json() {
        let id = CueId::new();
        let s = serde_json::to_string(&id).unwrap();
        let back: CueId = serde_json::from_str(&s).unwrap();
        assert_eq!(id, back);
    }
}
