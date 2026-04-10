use rusqlite::{params, Connection, Result as SqlResult};
use std::path::Path;

pub struct EmbeddingStore {
    conn: Connection,
}

impl EmbeddingStore {
    pub fn open(home: &Path) -> SqlResult<Self> {
        let db_path = home.join("embeddings.db");
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS embeddings (
                id    TEXT PRIMARY KEY,
                topic TEXT NOT NULL,
                text  TEXT NOT NULL,
                vec   BLOB NOT NULL
            );",
        )?;
        Ok(EmbeddingStore { conn })
    }

    fn make_id(topic: &str, text: &str) -> String {
        format!("{}::{}", topic, text)
    }

    pub fn upsert(&self, topic: &str, text: &str, vec: &[f32]) -> SqlResult<()> {
        let id = Self::make_id(topic, text);
        let blob = f32_slice_to_bytes(vec);
        self.conn.execute(
            "INSERT OR REPLACE INTO embeddings (id, topic, text, vec) VALUES (?1, ?2, ?3, ?4)",
            params![id, topic, text, blob],
        )?;
        Ok(())
    }

    pub fn delete(&self, topic: &str, text: &str) -> SqlResult<()> {
        let id = Self::make_id(topic, text);
        self.conn
            .execute("DELETE FROM embeddings WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn delete_topic(&self, topic: &str) -> SqlResult<()> {
        self.conn
            .execute("DELETE FROM embeddings WHERE topic = ?1", params![topic])?;
        Ok(())
    }

    pub fn all(&self) -> SqlResult<Vec<(String, String, Vec<f32>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT topic, text, vec FROM embeddings")?;
        let rows = stmt.query_map([], |row| {
            let topic: String = row.get(0)?;
            let text: String = row.get(1)?;
            let blob: Vec<u8> = row.get(2)?;
            Ok((topic, text, bytes_to_f32_vec(&blob)))
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn by_topic(&self, topic: &str) -> SqlResult<Vec<(String, Vec<f32>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT text, vec FROM embeddings WHERE topic = ?1")?;
        let rows = stmt.query_map(params![topic], |row| {
            let text: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((text, bytes_to_f32_vec(&blob)))
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }
}

fn f32_slice_to_bytes(v: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(v.len() * 4);
    for &x in v {
        bytes.extend_from_slice(&x.to_le_bytes());
    }
    bytes
}

fn bytes_to_f32_vec(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
