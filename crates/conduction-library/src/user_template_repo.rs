//! User Template の SQLite 永続化 (Phase D3)。
//!
//! payload はテンプレート全体を serde JSON 文字列として保持する。
//! ライブラリ層では opaque な文字列として扱い、シリアライズは app 層 (conductor 型を知る側) に任せる。

use chrono::Utc;
use rusqlite::params;

use crate::error::{LibraryError, LibraryResult};
use crate::library::Library;
use crate::mapping::dt_to_str;

#[derive(Debug, Clone)]
pub struct UserTemplateRow {
    pub id: String,
    pub name: String,
    pub payload: String,
}

impl Library {
    pub fn list_user_templates(&self) -> LibraryResult<Vec<UserTemplateRow>> {
        let conn = self.raw_conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, payload FROM user_templates ORDER BY name ASC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(UserTemplateRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    payload: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_user_template(&self, id: &str) -> LibraryResult<Option<UserTemplateRow>> {
        let conn = self.raw_conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, payload FROM user_templates WHERE id = ?1 LIMIT 1",
        )?;
        let mut rows = stmt.query(params![id])?;
        match rows.next()? {
            Some(row) => Ok(Some(UserTemplateRow {
                id: row.get(0)?,
                name: row.get(1)?,
                payload: row.get(2)?,
            })),
            None => Ok(None),
        }
    }

    /// upsert (id 一意)。
    pub fn save_user_template(
        &self,
        id: &str,
        name: &str,
        payload: &str,
    ) -> LibraryResult<()> {
        if id.is_empty() {
            return Err(LibraryError::Unsupported("user template id empty".into()));
        }
        let now = dt_to_str(Utc::now());
        self.raw_conn().execute(
            "INSERT INTO user_templates (id, name, payload, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name,
               payload = excluded.payload,
               updated_at = excluded.updated_at",
            params![id, name, payload, now],
        )?;
        Ok(())
    }

    /// user template を削除し、これを参照していた setlist_entries の transition を
    /// クリアする。`setlist_entries` 側に FK は張れない (built-in preset.* も同じ列に
    /// 入るため) ので、アプリ層でクリーンアップする。
    pub fn delete_user_template(&mut self, id: &str) -> LibraryResult<()> {
        let tx = self.raw_conn_mut().transaction()?;
        // 参照していた setlist transition を全て無効化
        tx.execute(
            "UPDATE setlist_entries SET
               transition_template_id = NULL,
               transition_tempo_mode = NULL,
               transition_entry_cue = NULL,
               transition_exit_cue = NULL
             WHERE transition_template_id = ?1",
            params![id],
        )?;
        let affected = tx.execute(
            "DELETE FROM user_templates WHERE id = ?1",
            params![id],
        )?;
        if affected == 0 {
            return Err(LibraryError::Unsupported(format!(
                "user template not found: {id}"
            )));
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_list_get_delete() {
        let mut lib = Library::in_memory().unwrap();
        lib.save_user_template("user.a", "A", "{\"x\":1}").unwrap();
        lib.save_user_template("user.b", "B", "{\"y\":2}").unwrap();

        let all = lib.list_user_templates().unwrap();
        assert_eq!(all.len(), 2);

        let got = lib.get_user_template("user.a").unwrap().unwrap();
        assert_eq!(got.name, "A");
        assert_eq!(got.payload, "{\"x\":1}");

        // upsert で更新
        lib.save_user_template("user.a", "A2", "{\"x\":99}").unwrap();
        let got = lib.get_user_template("user.a").unwrap().unwrap();
        assert_eq!(got.name, "A2");
        assert_eq!(got.payload, "{\"x\":99}");

        lib.delete_user_template("user.a").unwrap();
        assert!(lib.get_user_template("user.a").unwrap().is_none());
        assert!(lib.delete_user_template("user.missing").is_err());
    }

    #[test]
    fn empty_id_rejected() {
        let lib = Library::in_memory().unwrap();
        assert!(lib.save_user_template("", "A", "{}").is_err());
    }

    /// user template が参照されている setlist transition は、template 削除時に
    /// NULL にクリアされる (dangling 参照を残さない)。
    #[test]
    fn delete_clears_referencing_setlist_transitions() {
        use std::path::PathBuf;
        use conduction_core::{Key, KeyMode, TempoMode, Track, TransitionSpec};

        let mut lib = Library::in_memory().unwrap();
        let mut t1 = Track::placeholder(
            PathBuf::from("/tmp/a.mp3"),
            Key::new(8, KeyMode::Minor).unwrap(),
        );
        t1.bpm = 120.0;
        let mut t2 = Track::placeholder(
            PathBuf::from("/tmp/b.mp3"),
            Key::new(8, KeyMode::Minor).unwrap(),
        );
        t2.bpm = 120.0;
        lib.insert_track(&t1).unwrap();
        lib.insert_track(&t2).unwrap();

        lib.save_user_template("user.tmpl", "Custom", "{}").unwrap();
        let s = lib.create_setlist("X".into()).unwrap();
        let e1 = lib.add_setlist_entry(s.id, t1.id).unwrap();
        lib.add_setlist_entry(s.id, t2.id).unwrap();
        lib.set_setlist_transition(
            s.id,
            e1.id,
            Some(TransitionSpec {
                template_id: "user.tmpl".into(),
                tempo_mode: TempoMode::LinearBlend,
                entry_cue: None,
                exit_cue: None,
            }),
        )
        .unwrap();
        // 削除前: transition_to_next が設定されている
        let before = lib.get_setlist(s.id).unwrap().unwrap();
        assert!(before.entries[0].transition_to_next.is_some());

        lib.delete_user_template("user.tmpl").unwrap();

        // 削除後: transition_to_next が NULL に戻る
        let after = lib.get_setlist(s.id).unwrap().unwrap();
        assert!(after.entries[0].transition_to_next.is_none());
    }
}
