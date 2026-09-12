//! Root and PRODUCT elements of `DJ_PLAYLISTS`.
//!
//! Spec (de-facto, reverse-engineered): the file always opens with
//!
//! ```xml
//! <DJ_PLAYLISTS Version="1.0.0">
//!   <PRODUCT Name="rekordbox" Version="..." Company="AlphaTheta"/>
//!   <COLLECTION Entries="..."> ... </COLLECTION>
//!   <PLAYLISTS> ... </PLAYLISTS>
//! </DJ_PLAYLISTS>
//! ```
//!
//! Subsequent commits add the `COLLECTION` and `PLAYLISTS` types and wire
//! them into [`DjPlaylists`] as optional children.

use serde::{Deserialize, Serialize};

use super::collection::Collection;
use super::playlists::Playlists;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "DJ_PLAYLISTS")]
pub struct DjPlaylists {
    #[serde(rename = "@Version", default = "default_version")]
    pub version: String,
    #[serde(rename = "PRODUCT", default, skip_serializing_if = "Option::is_none")]
    pub product: Option<Product>,
    #[serde(
        rename = "COLLECTION",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub collection: Option<Collection>,
    #[serde(rename = "PLAYLISTS", default, skip_serializing_if = "Option::is_none")]
    pub playlists: Option<Playlists>,
}

impl Default for DjPlaylists {
    fn default() -> Self {
        Self {
            version: default_version(),
            product: None,
            collection: None,
            playlists: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename = "PRODUCT")]
pub struct Product {
    #[serde(rename = "@Name", default)]
    pub name: String,
    #[serde(rename = "@Version", default)]
    pub version: String,
    #[serde(rename = "@Company", default)]
    pub company: String,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_header_only() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<DJ_PLAYLISTS Version="1.0.0">
  <PRODUCT Name="rekordbox" Version="6.7.4" Company="AlphaTheta"/>
</DJ_PLAYLISTS>"#;
        let dj: DjPlaylists = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(dj.version, "1.0.0");
        let p = dj.product.expect("PRODUCT must parse");
        assert_eq!(p.name, "rekordbox");
        assert_eq!(p.version, "6.7.4");
        assert_eq!(p.company, "AlphaTheta");
    }

    #[test]
    fn serializes_back_to_xml() {
        let dj = DjPlaylists {
            version: "1.0.0".into(),
            product: Some(Product {
                name: "rekordbox".into(),
                version: "6.7.4".into(),
                company: "AlphaTheta".into(),
            }),
            collection: None,
            playlists: None,
        };
        let s = quick_xml::se::to_string(&dj).unwrap();
        assert!(s.contains(r#"Version="1.0.0""#));
        assert!(s.contains(r#"<PRODUCT"#));
        assert!(s.contains(r#"Name="rekordbox""#));
        assert!(s.contains(r#"Company="AlphaTheta""#));
    }
}
