//! Per-document native export cache; file writes and save identity stay with the caller.

use std::sync::{Arc, Mutex};

use photocraft_doc::Document;
use photocraft_format::{PcraftWriter, SaveOptions};

/// Shared native export cache. Clones reuse tile hashes and up to 64 MiB of compressed payloads;
/// dropping the last clone releases it, including when a document closes after a save worker.
#[derive(Clone)]
pub struct PcraftExportCache(Arc<Mutex<PcraftWriter>>);

impl Default for PcraftExportCache {
    fn default() -> Self {
        let mut writer = PcraftWriter::new();
        writer.set_compressed_cache_budget(64 << 20);
        Self(Arc::new(Mutex::new(writer)))
    }
}

impl std::fmt::Debug for PcraftExportCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PcraftExportCache")
    }
}

impl PartialEq for PcraftExportCache {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl PcraftExportCache {
    pub(crate) fn encode(&self, doc: &Document, opts: &SaveOptions) -> photocraft_format::Result<Vec<u8>> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).save_zip(doc, opts).map(|(bytes, _)| bytes)
    }
}