use std::collections::VecDeque;

use marix_common::external::uuid;

const CHUNK_BYTES: usize = 16 * 1024;
// A cached remainder restarts this far before the emitted chunk's
// end, so the next chunk repeats a small tail of the previous one
// and a fact straddling the boundary stays readable in at least one
// chunk. Must stay strictly below `CHUNK_BYTES`: each continuation
// advances by `CHUNK_BYTES - CHUNK_OVERLAP_BYTES` bytes, so an
// overlap at or above the chunk size would stop making progress and
// the continuation chain would never terminate.
const CHUNK_OVERLAP_BYTES: usize = 1024;
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
        let start = Self::chunk_start(content, end);
        let cursor = self.insert(content[start..].to_owned());
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
            let cursor = format!("tc_{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
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

    fn chunk_start(content: &str, end: usize) -> usize {
        let mut start = end.saturating_sub(CHUNK_OVERLAP_BYTES);
        while !content.is_char_boundary(start) {
            start -= 1;
        }
        start
    }

    fn not_available() -> String {
        "continuation_not_available".to_owned()
    }
}
