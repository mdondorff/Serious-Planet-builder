//! The command layer over the world definition (AUTH-004, ADR 0066 point 7): put, change and delete an authored item; undo and redo; a drag
//! coalesced into one command.
//!
//! Every command writes the item files through the store (undo and redo too): git is the history, and the log is only the in-memory
//! stack of this session. Undo and redo restore the exact previous item, so the definition hash is exactly the previous one (the hash is a
//! function of the set of items). Nothing in here touches a GPU or a window.

use crate::ids::IdMaker;
use planet_core::Vec3;
use planet_world_def::item::{ActiveVersions, Anchor, Item, ItemFileError, PutError};
use planet_world_def::item_store::{ItemStore, StoreError};
use planet_world_def::{BodyId, ItemError, ItemId, WorldDefinition};
use std::collections::BTreeMap;
use std::fmt;

/// The most operations the undo stack keeps (the oldest are dropped): ten times the 1,000 the M6 exit criterion asks for.
pub const UNDO_LIMIT: usize = 10_000;

/// What a new item is: everything but its id and the versions it is authored against, which the command fills in.
#[derive(Clone, Debug, PartialEq)]
pub struct NewItem {
    pub body: BodyId,
    pub surface: String,
    pub layer: String,
    pub priority: i32,
    pub kind: String,
    pub anchor: Anchor,
    pub reach_m: f64,
    pub max_delta_m: f64,
    pub params: BTreeMap<String, f64>,
    pub name: Option<String>,
    pub notes: Option<String>,
}

/// A place whose tiles an edit may have changed: the anchor of an item before or after the edit and its reach. The caller turns these
/// into the tiles to refresh (the footprint find, `ManagedLoop::edit`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affected {
    pub body: BodyId,
    /// The unit direction of the anchor.
    pub dir: Vec3,
    pub reach_m: f64,
}

/// Why a command did not happen. When a command fails nothing has changed: not the definition, not the files, not the stack.
#[derive(Debug)]
pub enum EditError {
    /// The item is not valid (its numbers, names or ranges): the key and the reason.
    Invalid(ItemFileError),
    /// The index refused the item (a body without a radius, a reach over the limit, ...).
    Index(ItemError),
    /// The store could not write or remove a file.
    Store(StoreError),
    /// There is no item of that id.
    NoSuchItem(ItemId),
    /// An item of that id exists already (an id made twice, or an item loaded twice).
    Duplicate(ItemId),
    /// A drag is open: finish or cancel it first.
    EditOpen,
    /// No drag is open.
    NoEditOpen,
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditError::Invalid(e) => write!(f, "{e}"),
            EditError::Index(e) => write!(f, "item refused by the index: {e:?}"),
            EditError::Store(e) => write!(f, "{e}"),
            EditError::NoSuchItem(id) => write!(f, "no item {}", planet_world_def::item::ulid_text(*id)),
            EditError::Duplicate(id) => write!(f, "an item {} exists already", planet_world_def::item::ulid_text(*id)),
            EditError::EditOpen => write!(f, "an edit is open: end or cancel it first"),
            EditError::NoEditOpen => write!(f, "no edit is open"),
        }
    }
}

impl std::error::Error for EditError {}

impl From<StoreError> for EditError {
    fn from(e: StoreError) -> Self {
        EditError::Store(e)
    }
}

impl From<PutError> for EditError {
    fn from(e: PutError) -> Self {
        match e {
            PutError::Invalid(e) => EditError::Invalid(e),
            PutError::Index(e) => EditError::Index(e),
        }
    }
}

impl From<ItemFileError> for EditError {
    fn from(e: ItemFileError) -> Self {
        EditError::Invalid(e)
    }
}

/// One command as the log keeps it: the item before (none for a put) and after (none for a delete).
#[derive(Clone, Debug, PartialEq)]
struct Command {
    before: Option<Item>,
    after: Option<Item>,
}

impl Command {
    fn id(&self) -> ItemId {
        self.after.as_ref().or(self.before.as_ref()).expect("a command has an item").id
    }

    fn affected(&self) -> Vec<Affected> {
        [&self.before, &self.after].into_iter().flatten().map(affected_of).collect()
    }
}

fn affected_of(item: &Item) -> Affected {
    Affected { body: item.body, dir: item.direction(), reach_m: item.reach_m }
}

/// A drag in progress: the item as it was, and as it is now (in the definition, not yet in its file).
struct OpenEdit {
    before: Item,
    now: Item,
}

/// The editor's state over a definition and a store.
pub struct Editor<S: ItemStore> {
    def: WorldDefinition,
    store: S,
    ids: IdMaker,
    generator_version: u32,
    stage_version: u32,
    undo: Vec<Command>,
    redo: Vec<Command>,
    open: Option<OpenEdit>,
    /// The items by id, the copy the log works from (the definition keeps the index form).
    items: BTreeMap<ItemId, Item>,
}

impl<S: ItemStore> Editor<S> {
    /// An editor over `def` and `store`. `versions` are the generator and stage versions new items are authored against; their base hash
    /// is that of the definition at the time of each command. The items already in `def` are not known to the editor (use
    /// [`load`](Editor::load) for those of the store).
    pub fn new(def: WorldDefinition, store: S, ids: IdMaker, generator_version: u32, stage_version: u32) -> Editor<S> {
        Editor { def, store, ids, generator_version, stage_version, undo: Vec::new(), redo: Vec::new(), open: None, items: BTreeMap::new() }
    }

    /// Load the items of a body from the store into the definition. An item authored against other versions than the active ones is
    /// refused with the message of `check_versions` (AUTH-002), and so is any file the store refuses; nothing is loaded then.
    pub fn load(&mut self, body: BodyId) -> Result<usize, EditError> {
        self.no_open_edit()?;
        let items = self.store.load_all(body)?;
        let versions = ActiveVersions {
            generator_version: self.generator_version,
            stage_version: self.stage_version,
            base_hash: self.def.base_hash(),
        };
        for item in &items {
            item.check_versions(&versions).map_err(|m| EditError::Invalid(ItemFileError { key: "versions".into(), why: m.to_string() }))?;
            if self.items.contains_key(&item.id) {
                return Err(EditError::Duplicate(item.id));
            }
        }
        // Into a copy first, so a refused item leaves the definition as it was.
        let mut trial = self.def.clone();
        for item in &items {
            item.put_into(&mut trial).map_err(EditError::from)?;
        }
        self.def = trial;
        for item in &items {
            self.items.insert(item.id, item.clone().canonical());
        }
        Ok(items.len())
    }

    /// The definition as the commands have left it.
    pub fn definition(&self) -> &WorldDefinition {
        &self.def
    }

    /// The hash of the definition.
    pub fn hash(&self) -> u64 {
        self.def.hash()
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    /// The item of an id, if there is one.
    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.items.get(&id)
    }

    /// The ids of the items, in id order.
    pub fn item_ids(&self) -> Vec<ItemId> {
        self.items.keys().copied().collect()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() && self.open.is_none()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty() && self.open.is_none()
    }

    /// How many commands undo can take back.
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    // ----- the commands

    /// Put a new item: its id comes from the injected clock and entropy, it is authored against the active versions and the base hash of
    /// the definition now. Returns the id and the places to refresh.
    pub fn put(&mut self, new: NewItem) -> Result<(ItemId, Vec<Affected>), EditError> {
        self.no_open_edit()?;
        let item = Item {
            id: self.ids.next_id(),
            body: new.body,
            surface: new.surface,
            layer: new.layer,
            priority: new.priority,
            kind: new.kind,
            anchor: new.anchor,
            reach_m: new.reach_m,
            max_delta_m: new.max_delta_m,
            params: new.params,
            base_hash: self.def.base_hash(),
            generator_version: self.generator_version,
            stage_version: self.stage_version,
            name: new.name,
            notes: new.notes,
        }
        .canonical();
        item.validate()?;
        if self.items.contains_key(&item.id) {
            return Err(EditError::Duplicate(item.id));
        }
        let command = Command { before: None, after: Some(item) };
        let id = command.id();
        let affected = self.run(&command)?;
        self.record(command);
        Ok((id, affected))
    }

    /// Change an item: `f` edits a copy, and the result replaces the item (the id and the body stay). A change that leaves the item as it
    /// was is not a command. Returns the places to refresh.
    pub fn change(&mut self, id: ItemId, f: impl FnOnce(&mut Item)) -> Result<Vec<Affected>, EditError> {
        self.no_open_edit()?;
        let before = self.items.get(&id).cloned().ok_or(EditError::NoSuchItem(id))?;
        let mut after = before.clone();
        f(&mut after);
        let after = Item { id: before.id, body: before.body, ..after }.canonical();
        after.validate()?;
        if after == before {
            return Ok(Vec::new());
        }
        let command = Command { before: Some(before), after: Some(after) };
        let affected = self.run(&command)?;
        self.record(command);
        Ok(affected)
    }

    /// Delete an item.
    pub fn delete(&mut self, id: ItemId) -> Result<Vec<Affected>, EditError> {
        self.no_open_edit()?;
        let before = self.items.get(&id).cloned().ok_or(EditError::NoSuchItem(id))?;
        let command = Command { before: Some(before), after: None };
        let affected = self.run(&command)?;
        self.record(command);
        Ok(affected)
    }

    // ----- a drag: many changes, one command

    /// Begin a drag of an item: the changes that follow are applied to the definition at once (the editor shows them) and written to the
    /// file once, when the drag ends, as one command.
    pub fn begin_edit(&mut self, id: ItemId) -> Result<(), EditError> {
        self.no_open_edit()?;
        let before = self.items.get(&id).cloned().ok_or(EditError::NoSuchItem(id))?;
        self.open = Some(OpenEdit { now: before.clone(), before });
        Ok(())
    }

    /// One step of the open drag: `f` edits the item as it is now. The definition changes; no file is written. Returns the places the
    /// step changed (the item's anchor before and after the step). A step the item cannot take (an invalid number) is refused and leaves
    /// the drag as it was.
    pub fn edit(&mut self, f: impl FnOnce(&mut Item)) -> Result<Vec<Affected>, EditError> {
        let open = self.open.as_ref().ok_or(EditError::NoEditOpen)?;
        let mut next = open.now.clone();
        f(&mut next);
        let next = Item { id: open.now.id, body: open.now.body, ..next }.canonical();
        next.validate()?;
        let affected = vec![affected_of(&open.now), affected_of(&next)];
        next.put_into(&mut self.def).map_err(EditError::from)?;
        self.items.insert(next.id, next.clone());
        self.open.as_mut().expect("open").now = next;
        Ok(affected)
    }

    /// End the drag: the file is written once and the whole drag is one command (none when the item is back where it was).
    pub fn end_edit(&mut self) -> Result<Vec<Affected>, EditError> {
        let open = self.open.take().ok_or(EditError::NoEditOpen)?;
        if open.now == open.before {
            return Ok(Vec::new());
        }
        let command = Command { before: Some(open.before.clone()), after: Some(open.now.clone()) };
        // The definition holds the end of the drag already; only the file is left to write.
        if let Err(e) = self.store.put(&open.now) {
            self.open = Some(open);
            return Err(e.into());
        }
        let affected = command.affected();
        self.record(command);
        Ok(affected)
    }

    /// Cancel the drag: the item is as it was before it began.
    pub fn cancel_edit(&mut self) -> Result<Vec<Affected>, EditError> {
        // The item goes back first: a refusal (which cannot happen for an item that was accepted before) would leave the drag open.
        let open = self.open.as_ref().ok_or(EditError::NoEditOpen)?;
        open.before.put_into(&mut self.def).map_err(EditError::from)?;
        let open = self.open.take().expect("open");
        let affected = vec![affected_of(&open.now), affected_of(&open.before)];
        self.items.insert(open.before.id, open.before);
        Ok(affected)
    }

    // ----- undo and redo

    /// Take back the last command, files included. `None` when there is none; an open drag is an error.
    pub fn undo(&mut self) -> Result<Option<Vec<Affected>>, EditError> {
        self.no_open_edit()?;
        let Some(command) = self.undo.pop() else { return Ok(None) };
        let inverse = Command { before: command.after.clone(), after: command.before.clone() };
        match self.run(&inverse) {
            Ok(affected) => {
                self.redo.push(command);
                Ok(Some(affected))
            }
            Err(e) => {
                self.undo.push(command);
                Err(e)
            }
        }
    }

    /// Do again the command that was last undone, files included.
    pub fn redo(&mut self) -> Result<Option<Vec<Affected>>, EditError> {
        self.no_open_edit()?;
        let Some(command) = self.redo.pop() else { return Ok(None) };
        match self.run(&command) {
            Ok(affected) => {
                self.undo.push(command);
                Ok(Some(affected))
            }
            Err(e) => {
                self.redo.push(command);
                Err(e)
            }
        }
    }

    // ----- the one place state changes

    fn no_open_edit(&self) -> Result<(), EditError> {
        if self.open.is_some() {
            Err(EditError::EditOpen)
        } else {
            Ok(())
        }
    }

    fn record(&mut self, command: Command) {
        self.undo.push(command);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Bring the store, the definition and the item copy to the state after `command`. A failure leaves everything as it was: the index
    /// refuses an item before it changes anything, and a store failure puts the definition back.
    fn run(&mut self, command: &Command) -> Result<Vec<Affected>, EditError> {
        let id = command.id();
        let current = self.items.get(&id).cloned();
        match (&command.after, current) {
            (Some(after), current) => {
                after.put_into(&mut self.def).map_err(EditError::from)?;
                if let Err(e) = self.store.put(after) {
                    match &current {
                        Some(item) => {
                            let _ = item.put_into(&mut self.def);
                        }
                        None => {
                            self.def.remove_item(after.body, id);
                        }
                    }
                    return Err(e.into());
                }
                self.items.insert(id, after.clone());
            }
            (None, Some(item)) => {
                self.store.delete(item.body, id)?;
                self.def.remove_item(item.body, id);
                self.items.remove(&id);
            }
            (None, None) => {}
        }
        Ok(command.affected())
    }
}
