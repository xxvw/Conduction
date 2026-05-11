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

    pub fn delete_user_template(&self, id: &str) -> LibraryResult<()> {
        let affected = self.raw_conn().execute(
            "DELETE FROM user_templates WHERE id = ?1",
            params![id],
        )?;
        if affected == 0 {
            return Err(LibraryError::Unsupported(format!(
                "user template not found: {id}"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_list_get_delete() {
        let lib = Library::in_memory().unwrap();
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
}
