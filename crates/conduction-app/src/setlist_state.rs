//! Setlist の永続化バックエンド (Phase C1)。
//!
//! `library.db` (SQLite) に保存。CRUD は conduction-library の Setlist 系メソッドへ委譲。

use std::sync::Arc;

use conduction_core::{Setlist, SetlistEntry, SetlistEntryId, SetlistId, TrackId, TransitionSpec};
use conduction_library::Library;
use parking_lot::Mutex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SetlistError {
    #[error("setlist not found: {0}")]
    NotFound(SetlistId),
    #[error("entry not found: {0}")]
    EntryNotFound(SetlistEntryId),
    #[error("invalid index: {0}")]
    BadIndex(i64),
    #[error("library error: {0}")]
    Library(#[from] conduction_library::LibraryError),
}

pub type SetlistResult<T> = Result<T, SetlistError>;

#[derive(Clone)]
pub struct SetlistHandle {
    library: Arc<Mutex<Library>>,
}

impl SetlistHandle {
    pub fn new(library: Arc<Mutex<Library>>) -> Self {
        Self { library }
    }

    pub fn list(&self) -> Vec<Setlist> {
        // 失敗時は空リスト (UI 側は別 commands で原因を見られる)。
        self.library.lock().list_setlists().unwrap_or_default()
    }

    pub fn get(&self, id: SetlistId) -> SetlistResult<Setlist> {
        self.library
            .lock()
            .get_setlist(id)?
            .ok_or(SetlistError::NotFound(id))
    }

    pub fn create(&self, name: String) -> Setlist {
        // create は基本失敗しないが、SQLite I/O エラー時にプレースホルダを返す
        // (UI は次回の list でズレに気付く)。
        match self.library.lock().create_setlist(name.clone()) {
            Ok(s) => s,
            Err(_) => Setlist::new(name),
        }
    }

    pub fn delete(&self, id: SetlistId) -> SetlistResult<()> {
        self.library
            .lock()
            .delete_setlist(id)
            .map_err(SetlistError::Library)
    }

    pub fn rename(&self, id: SetlistId, name: String) -> SetlistResult<Setlist> {
        self.library
            .lock()
            .rename_setlist(id, name)
            .map_err(SetlistError::Library)
    }

    pub fn add_entry(
        &self,
        id: SetlistId,
        track_id: TrackId,
    ) -> SetlistResult<SetlistEntry> {
        self.library
            .lock()
            .add_setlist_entry(id, track_id)
            .map_err(SetlistError::Library)
    }

    pub fn remove_entry(
        &self,
        id: SetlistId,
        entry_id: SetlistEntryId,
    ) -> SetlistResult<()> {
        self.library
            .lock()
            .remove_setlist_entry(id, entry_id)
            .map_err(SetlistError::Library)
    }

    pub fn move_entry(
        &self,
        id: SetlistId,
        entry_id: SetlistEntryId,
        new_index: i64,
    ) -> SetlistResult<Setlist> {
        self.library
            .lock()
            .move_setlist_entry(id, entry_id, new_index)
            .map_err(SetlistError::Library)
    }

    pub fn set_transition(
        &self,
        id: SetlistId,
        entry_id: SetlistEntryId,
        spec: Option<TransitionSpec>,
    ) -> SetlistResult<SetlistEntry> {
        self.library
            .lock()
            .set_setlist_transition(id, entry_id, spec)
            .map_err(SetlistError::Library)
    }
}
