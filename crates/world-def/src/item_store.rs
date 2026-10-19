//! The item store (ADR 0066 point 6): where the item files of a body live, behind an interface, loose files as the first backend. The id
//! and the content hash do not depend on the backend, so a packed or database backend later needs a one-time conversion and no re-key.

use crate::item::{parse_ulid, ulid_text, Item, ItemFileError};
use crate::item_id::ItemId;
use crate::BodyId;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// Why a store operation failed.
#[derive(Debug)]
pub enum StoreError {
    /// A file or directory could not be read or written: what was being done, the path, the system's message.
    Io { what: &'static str, path: PathBuf, message: String },
    /// A file holds no valid item: the path and the reason.
    BadFile { path: PathBuf, error: ItemFileError },
    /// A file is in the item directory under a name that is not `<ULID>.toml`, or its `id` or `body` is not its name's.
    Misplaced { path: PathBuf, why: String },
    /// An item that is not valid (it was never written).
    Invalid(ItemFileError),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io { what, path, message } => write!(f, "item store: {what} {}: {message}", path.display()),
            StoreError::BadFile { path, error } => write!(f, "item store: {}: {error}", path.display()),
            StoreError::Misplaced { path, why } => write!(f, "item store: {}: {why}", path.display()),
            StoreError::Invalid(e) => write!(f, "item store: {e}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Where the items of a world are kept. Every operation is whole: an item is written completely or not at all, and `load_all` returns the
/// items of a body in id order (the order of the ULID text) or the first file that is wrong.
pub trait ItemStore {
    /// Write the item (adding it, or replacing the item of the same id and body).
    fn put(&mut self, item: &Item) -> Result<(), StoreError>;
    /// Remove an item; `false` when there was none.
    fn delete(&mut self, body: BodyId, id: ItemId) -> Result<bool, StoreError>;
    /// The item, if the store has it.
    fn get(&self, body: BodyId, id: ItemId) -> Result<Option<Item>, StoreError>;
    /// Every item of a body, in id order.
    fn load_all(&self, body: BodyId) -> Result<Vec<Item>, StoreError>;
}

/// Items in memory: for tests, and for an editor that is not given a directory.
#[derive(Clone, Debug, Default)]
pub struct MemoryStore {
    items: BTreeMap<(BodyId, ItemId), Item>,
}

impl MemoryStore {
    pub fn new() -> MemoryStore {
        MemoryStore::default()
    }
}

impl ItemStore for MemoryStore {
    fn put(&mut self, item: &Item) -> Result<(), StoreError> {
        item.validate().map_err(StoreError::Invalid)?;
        self.items.insert((item.body, item.id), item.clone().canonical());
        Ok(())
    }

    fn delete(&mut self, body: BodyId, id: ItemId) -> Result<bool, StoreError> {
        Ok(self.items.remove(&(body, id)).is_some())
    }

    fn get(&self, body: BodyId, id: ItemId) -> Result<Option<Item>, StoreError> {
        Ok(self.items.get(&(body, id)).cloned())
    }

    fn load_all(&self, body: BodyId) -> Result<Vec<Item>, StoreError> {
        Ok(self.items.range((body, 0)..=(body, ItemId::MAX)).map(|(_, item)| item.clone()).collect())
    }
}

/// One TOML file per item, `<root>/<body>/<ULID>.toml`: adding an item adds a file, so two people adding items never touch the same
/// file and git merges the directories without a conflict.
#[derive(Clone, Debug)]
pub struct LooseFileStore {
    root: PathBuf,
    flush: bool,
}

impl LooseFileStore {
    /// A store under `root` (the directory is made when the first item is written).
    pub fn new(root: impl Into<PathBuf>) -> LooseFileStore {
        LooseFileStore { root: root.into(), flush: true }
    }

    /// The same store without the flush of each file to disk (tests that write thousands of files and do not test durability).
    pub fn without_flush(mut self) -> LooseFileStore {
        self.flush = false;
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The directory of a body's items.
    pub fn body_dir(&self, body: BodyId) -> PathBuf {
        self.root.join(body.0.to_string())
    }

    /// The file of an item.
    pub fn path_of(&self, body: BodyId, id: ItemId) -> PathBuf {
        self.body_dir(body).join(format!("{}.toml", ulid_text(id)))
    }

    fn read(&self, path: &Path, body: BodyId, expected: ItemId) -> Result<Item, StoreError> {
        let text = std::fs::read_to_string(path).map_err(|e| StoreError::Io {
            what: "reading",
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
        let item = Item::from_toml(&text).map_err(|error| StoreError::BadFile { path: path.to_path_buf(), error })?;
        if item.id != expected {
            return Err(StoreError::Misplaced {
                path: path.to_path_buf(),
                why: format!("the file is named for {} but holds the item {}", ulid_text(expected), ulid_text(item.id)),
            });
        }
        if item.body != body {
            return Err(StoreError::Misplaced {
                path: path.to_path_buf(),
                why: format!("the file is in the directory of body {} but holds an item of body {}", body.0, item.body.0),
            });
        }
        Ok(item)
    }
}

impl ItemStore for LooseFileStore {
    fn put(&mut self, item: &Item) -> Result<(), StoreError> {
        let text = item.to_toml().map_err(StoreError::Invalid)?;
        let dir = self.body_dir(item.body);
        std::fs::create_dir_all(&dir).map_err(|e| StoreError::Io { what: "creating", path: dir.clone(), message: e.to_string() })?;
        let path = self.path_of(item.body, item.id);
        // Written whole beside the file under a name no other writer uses (the process and a counter), flushed to disk, and moved over it,
        // so a reader never sees half an item and two writers of one id never share a temporary file. A leftover temporary file (a crash,
        // a failed move) is hidden from `load_all`; keep `.*.tmp` in the repository's ignore list.
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let temp = dir.join(format!(".{}.{}.{n}.tmp", ulid_text(item.id), std::process::id()));
        let io = |what: &'static str, path: &Path| {
            let path = path.to_path_buf();
            move |e: std::io::Error| StoreError::Io { what, path: path.clone(), message: e.to_string() }
        };
        let write = || -> Result<(), StoreError> {
            use std::io::Write;
            let mut file = std::fs::File::create(&temp).map_err(io("creating", &temp))?;
            file.write_all(text.as_bytes()).map_err(io("writing", &temp))?;
            if self.flush {
                file.sync_all().map_err(io("flushing", &temp))?;
            }
            std::fs::rename(&temp, &path).map_err(io("moving into place", &path))
        };
        let result = write();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result
    }

    fn delete(&mut self, body: BodyId, id: ItemId) -> Result<bool, StoreError> {
        let path = self.path_of(body, id);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(StoreError::Io { what: "removing", path, message: e.to_string() }),
        }
    }

    fn get(&self, body: BodyId, id: ItemId) -> Result<Option<Item>, StoreError> {
        // A missing file is no item; any other failure to read it (a permission, a bad file) is an error, not "none".
        let path = self.path_of(body, id);
        match std::fs::metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(StoreError::Io { what: "reading", path, message: e.to_string() }),
            Ok(_) => self.read(&path, body, id).map(Some),
        }
    }

    fn load_all(&self, body: BodyId) -> Result<Vec<Item>, StoreError> {
        let dir = self.body_dir(body);
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(StoreError::Io { what: "listing", path: dir, message: e.to_string() }),
        };
        let mut found: BTreeMap<ItemId, PathBuf> = BTreeMap::new();
        for entry in entries {
            let entry = entry.map_err(|e| StoreError::Io { what: "listing", path: dir.clone(), message: e.to_string() })?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            // Hidden files (a leftover temporary file, a `.gitkeep`) are not items.
            if name.starts_with('.') {
                continue;
            }
            let stem = name
                .strip_suffix(".toml")
                .ok_or_else(|| StoreError::Misplaced { path: path.clone(), why: "an item file is named `<ULID>.toml`".into() })?;
            let id = parse_ulid(stem)
                .map_err(|e| StoreError::Misplaced { path: path.clone(), why: format!("the name is not a ULID: {}", e.why) })?;
            found.insert(id, path);
        }
        found.into_iter().map(|(id, path)| self.read(&path, body, id)).collect()
    }
}
