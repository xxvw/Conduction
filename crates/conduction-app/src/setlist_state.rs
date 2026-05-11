//! Setlist の in-memory state (Phase A1)。永続化は後段で SQLite / .cset に切り替える。

use std::sync::Arc;

use conduction_core::{Setlist, SetlistEntry, SetlistEntryId, SetlistId, TrackId, TransitionSpec};
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
}

pub type SetlistResult<T> = Result<T, SetlistError>;

#[derive(Clone)]
pub struct SetlistHandle {
    inner: Arc<Mutex<Vec<Setlist>>>,
}

impl SetlistHandle {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn list(&self) -> Vec<Setlist> {
        self.inner.lock().clone()
    }

    pub fn get(&self, id: SetlistId) -> SetlistResult<Setlist> {
        self.inner
            .lock()
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or(SetlistError::NotFound(id))
    }

    pub fn create(&self, name: String) -> Setlist {
        let setlist = Setlist::new(name);
        let mut guard = self.inner.lock();
        guard.push(setlist.clone());
        setlist
    }

    pub fn delete(&self, id: SetlistId) -> SetlistResult<()> {
        let mut guard = self.inner.lock();
        let before = guard.len();
        guard.retain(|s| s.id != id);
        if guard.len() == before {
            return Err(SetlistError::NotFound(id));
        }
        Ok(())
    }

    pub fn rename(&self, id: SetlistId, name: String) -> SetlistResult<Setlist> {
        let mut guard = self.inner.lock();
        let s = guard
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(SetlistError::NotFound(id))?;
        s.name = name;
        Ok(s.clone())
    }

    pub fn add_entry(
        &self,
        id: SetlistId,
        track_id: TrackId,
    ) -> SetlistResult<SetlistEntry> {
        let mut guard = self.inner.lock();
        let s = guard
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(SetlistError::NotFound(id))?;
        let entry = SetlistEntry::new(track_id);
        s.entries.push(entry.clone());
        Ok(entry)
    }

    pub fn remove_entry(
        &self,
        id: SetlistId,
        entry_id: SetlistEntryId,
    ) -> SetlistResult<()> {
        let mut guard = self.inner.lock();
        let s = guard
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(SetlistError::NotFound(id))?;
        let before = s.entries.len();
        s.entries.retain(|e| e.id != entry_id);
        if s.entries.len() == before {
            return Err(SetlistError::EntryNotFound(entry_id));
        }
        Ok(())
    }

    /// `entry_id` を `new_index` の位置に移動する。`new_index` が範囲外なら端にクランプ。
    pub fn move_entry(
        &self,
        id: SetlistId,
        entry_id: SetlistEntryId,
        new_index: i64,
    ) -> SetlistResult<Setlist> {
        let mut guard = self.inner.lock();
        let s = guard
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(SetlistError::NotFound(id))?;
        let from = s
            .entries
            .iter()
            .position(|e| e.id == entry_id)
            .ok_or(SetlistError::EntryNotFound(entry_id))?;
        let to = new_index.clamp(0, (s.entries.len() as i64).saturating_sub(1)) as usize;
        let item = s.entries.remove(from);
        s.entries.insert(to, item);
        Ok(s.clone())
    }

    pub fn set_transition(
        &self,
        id: SetlistId,
        entry_id: SetlistEntryId,
        spec: Option<TransitionSpec>,
    ) -> SetlistResult<SetlistEntry> {
        let mut guard = self.inner.lock();
        let s = guard
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(SetlistError::NotFound(id))?;
        let e = s
            .entries
            .iter_mut()
            .find(|e| e.id == entry_id)
            .ok_or(SetlistError::EntryNotFound(entry_id))?;
        e.transition_to_next = spec;
        Ok(e.clone())
    }
}

impl Default for SetlistHandle {
    fn default() -> Self {
        Self::new()
    }
}
