use std::collections::VecDeque;

use marix_common::external::uuid;

const CHUNK_BYTES: usize = 16 * 1024;
const CACHE_THRESHOLD_BYTES: usize = 24 * 1024;
const CACHE_CAPACITY: usize = 10;

struct Entry {
    cursor: String,
    content: String,
}

#[derive(Default)]
pub(crate) struct ExecutorCache {
    entries: VecDeque<Entry>,
}

impl ExecutorCache {
    pub(crate) fn try_cache(&mut self, content: &str) -> Result<(String, Option<String>), String> {
        if content.len() <= CACHE_THRESHOLD_BYTES {
            return Ok((content.to_owned(), None));
        }
        let end = Self::chunk_end(content);
        let cursor = self.insert(content[end..].to_owned());
        Ok((content[..end].to_owned(), Some(cursor)))
    }

    pub(crate) fn pick(&mut self, cursor: &str) -> Result<(String, Option<String>), String> {
        let entry = self.remove_entry(cursor).ok_or_else(Self::not_available)?;
        self.try_cache(&entry.content)
    }
}

// -- Private -- //

impl ExecutorCache {
    fn insert(&mut self, content: String) -> String {
        let cursor = self.generate_cursor();
        if self.entries.len() >= CACHE_CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(Entry {
            cursor: cursor.clone(),
            content,
        });
        cursor
    }

    fn generate_cursor(&self) -> String {
        loop {
            let cursor = format!("tc_{}", uuid::Uuid::new_v4().simple());
            if self.entries.iter().all(|entry| entry.cursor != cursor) {
                return cursor;
            }
        }
    }

    fn remove_entry(&mut self, cursor: &str) -> Option<Entry> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.cursor == cursor)?;
        self.entries.remove(index)
    }

    fn chunk_end(content: &str) -> usize {
        let mut end = content.len().min(CHUNK_BYTES);
        while !content.is_char_boundary(end) {
            end -= 1;
        }
        end
    }

    fn not_available() -> String {
        "continuation_not_available".to_owned()
    }
}
