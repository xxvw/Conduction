//! conduction-library — ライブラリ管理と SQLite 永続化。
//!
//! Phase 3 で DB スキーマ + CRUD を実装する。楽曲インポート（メタデータ抽出）と
//! フォルダ監視は 3a-2 以降で追加する。

#![forbid(unsafe_code)]

pub mod error;
pub mod import;
pub mod library;
pub mod mapping;
pub mod schema;
pub mod setlist_repo;
pub mod user_template_repo;

pub use error::{LibraryError, LibraryResult};
pub use import::{build_track_from_file, extract_metadata, ImportedMetadata};
pub use library::Library;
pub use schema::CURRENT_SCHEMA_VERSION;
