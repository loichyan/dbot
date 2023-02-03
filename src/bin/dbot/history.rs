use dbot::compile::CompiledEntries;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct HistoryManager {
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Entry {
    pub timespan: OffsetDateTime,
    pub entries: CompiledEntries,
}

impl HistoryManager {
    pub fn last(&self) -> Option<&Entry> {
        self.entries.last()
    }

    pub fn pop(&mut self) -> Option<Entry> {
        self.entries.pop()
    }

    pub fn push(&mut self, entries: CompiledEntries) {
        self.entries.push(Entry {
            timespan: OffsetDateTime::now_utc(),
            entries,
        });
    }
}
