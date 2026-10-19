//! The command layer over the world definition (AUTH-004, ADR 0066 point 7): put, change and delete an authored item; undo and redo; a drag
//! coalesced into one command; a group of such steps (the parts of a line, VIEW-028) as one command.
//!
//! Every command writes the item files through the store (undo and redo too): git is the history, and the log is only the in-memory
//! stack of this session. Undo and redo restore the exact previous item, so the definition hash is exactly the previous one (the hash is a
//! function of the set of items). Nothing in here touches a GPU or a window.

use crate::ids::IdMaker;
use crate::polyline::{self, GeoPoint, PolylineError};
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
    /// The points of a line or a region in degrees, the first being the anchor's place (ADR 0069); empty for an item without points.
    pub points: Vec<(f64, f64)>,
    /// The line this item is a part of, with `part` (the editor sets both when it splits a long line).
    pub group: Option<ItemId>,
    pub part: Option<u32>,
    /// The layer a region clips (regions only).
    pub mask_layer: Option<String>,
}

/// One step of a group command (VIEW-028): put a new item, replace the content of an item (its id, body and versions stay), or delete one.
#[derive(Clone, Debug, PartialEq)]
pub enum GroupStep {
    Put(NewItem),
    Change(ItemId, NewItem),
    Delete(ItemId),
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
    /// The points of a line or a region are refused (VIEW-027): the reason.
    Geometry(PolylineError),
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
            EditError::Geometry(e) => write!(f, "{e}"),
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

impl From<PolylineError> for EditError {
    fn from(e: PolylineError) -> Self {
        EditError::Geometry(e)
    }
}

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

/// One item's part of a command: the item before (none for a put) and after (none for a delete).
#[derive(Clone, Debug, PartialEq)]
struct Step {
    before: Option<Item>,
    after: Option<Item>,
}

impl Step {
    fn id(&self) -> ItemId {
        self.after.as_ref().or(self.before.as_ref()).expect("a step has an item").id
    }

    fn inverse(&self) -> Step {
        Step { before: self.after.clone(), after: self.before.clone() }
    }
}

/// One command as the log keeps it: one step for a put, a change, a delete or a drag, several for a group, undone together.
#[derive(Clone, Debug, PartialEq)]
struct Command {
    steps: Vec<Step>,
}

impl Command {
    fn one(before: Option<Item>, after: Option<Item>) -> Command {
        Command { steps: vec![Step { before, after }] }
    }

    fn affected(&self) -> Vec<Affected> {
        self.steps.iter().flat_map(|s| [&s.before, &s.after]).flatten().map(affected_of).collect()
    }

    fn inverse(&self) -> Command {
        Command { steps: self.steps.iter().rev().map(Step::inverse).collect() }
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
    /// Counts the changes made to the definition (commands, undo, redo, a drag step, a load); a cheap way to ask whether it moved.
    revision: u64,
}

impl<S: ItemStore> Editor<S> {
    /// An editor over `def` and `store`. `versions` are the generator and stage versions new items are authored against; their base hash
    /// is that of the definition at the time of each command. The items already in `def` are not known to the editor (use
    /// [`load`](Editor::load) for those of the store).
    pub fn new(def: WorldDefinition, store: S, ids: IdMaker, generator_version: u32, stage_version: u32) -> Editor<S> {
        Editor {
            def,
            store,
            ids,
            generator_version,
            stage_version,
            undo: Vec::new(),
            redo: Vec::new(),
            open: None,
            items: BTreeMap::new(),
            revision: 0,
        }
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
        // A load that finds nothing leaves the definition as it was.
        self.revision += u64::from(!items.is_empty());
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
    /// How many times the definition has been changed (zero at the start): equal revisions are the same definition.
    pub fn revision(&self) -> u64 {
        self.revision
    }

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

    /// Every item the editor knows, in id order.
    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.items.values()
    }

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
        let item = self.make_item(new);
        item.validate()?;
        if self.items.contains_key(&item.id) {
            return Err(EditError::Duplicate(item.id));
        }
        let id = item.id;
        let command = Command::one(None, Some(item));
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
        let command = Command::one(Some(before), Some(after));
        let affected = self.run(&command)?;
        self.record(command);
        Ok(affected)
    }

    /// Delete an item.
    pub fn delete(&mut self, id: ItemId) -> Result<Vec<Affected>, EditError> {
        self.no_open_edit()?;
        let before = self.items.get(&id).cloned().ok_or(EditError::NoSuchItem(id))?;
        let command = Command::one(Some(before), None);
        let affected = self.run(&command)?;
        self.record(command);
        Ok(affected)
    }

    /// An item made of a new item's fields: a fresh id from the injected clock and entropy, authored against the active versions and the
    /// base hash of the definition now.
    fn make_item(&mut self, new: NewItem) -> Item {
        Item {
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
            points: new.points,
            group: new.group,
            part: new.part,
            mask_layer: new.mask_layer,
        }
        .canonical()
    }

    // ----- a group of steps: one command (VIEW-028)

    /// Apply several steps as one command: all happen or none does, and one undo takes them all back (and one redo does them again). A put
    /// gets a fresh id; a change replaces the content of the item (its id, body and the versions it was authored against stay); a delete
    /// removes it. Two steps on one item are refused, and a step that changes nothing is dropped. Returns the ids of the puts in order and
    /// the places to refresh; a group that changes nothing is not a command.
    pub fn group(&mut self, steps: Vec<GroupStep>) -> Result<(Vec<ItemId>, Vec<Affected>), EditError> {
        self.no_open_edit()?;
        let mut made: Vec<Step> = Vec::new();
        let mut put_ids = Vec::new();
        let mut touched: Vec<ItemId> = Vec::new();
        for step in steps {
            match step {
                GroupStep::Put(new) => {
                    let item = self.make_item(new);
                    item.validate()?;
                    if self.items.contains_key(&item.id) {
                        return Err(EditError::Duplicate(item.id));
                    }
                    put_ids.push(item.id);
                    made.push(Step { before: None, after: Some(item) });
                }
                GroupStep::Change(id, new) => {
                    let before = self.items.get(&id).cloned().ok_or(EditError::NoSuchItem(id))?;
                    let after = Item {
                        surface: new.surface,
                        layer: new.layer,
                        priority: new.priority,
                        kind: new.kind,
                        anchor: new.anchor,
                        reach_m: new.reach_m,
                        max_delta_m: new.max_delta_m,
                        params: new.params,
                        name: new.name,
                        notes: new.notes,
                        points: new.points,
                        group: new.group,
                        part: new.part,
                        mask_layer: new.mask_layer,
                        ..before.clone()
                    }
                    .canonical();
                    after.validate()?;
                    if touched.contains(&id) {
                        return Err(EditError::Invalid(ItemFileError { key: "group".into(), why: "two steps name the same item".into() }));
                    }
                    touched.push(id);
                    if after != before {
                        made.push(Step { before: Some(before), after: Some(after) });
                    }
                }
                GroupStep::Delete(id) => {
                    let before = self.items.get(&id).cloned().ok_or(EditError::NoSuchItem(id))?;
                    if touched.contains(&id) {
                        return Err(EditError::Invalid(ItemFileError { key: "group".into(), why: "two steps name the same item".into() }));
                    }
                    touched.push(id);
                    made.push(Step { before: Some(before), after: None });
                }
            }
        }
        if made.is_empty() {
            return Ok((put_ids, Vec::new()));
        }
        let command = Command { steps: made };
        let affected = self.run(&command)?;
        self.record(command);
        Ok((put_ids, affected))
    }

    // ----- lines and regions (ADR 0069, VIEW-028)

    fn radius_of(&self, body: BodyId) -> Result<f64, EditError> {
        let path = planet_world_def::body_radius_path(body);
        match self.def.get(&path) {
            Some(planet_world_def::NodeValue::F64(r)) if r.is_finite() && r > 0.0 => Ok(r),
            _ => Err(EditError::Index(ItemError::Radius(planet_world_def::MissingNode(path)))),
        }
    }

    /// The group steps of a line of `points` made like `template` (its kind, layer, priority, parameters, name; `width_m` in its
    /// parameters is the line's half-width): one item when the line fits the reach of one, else the linked parts of
    /// [`polyline::split_line`] sharing one group, each anchored at its first point with a reach that covers its points and its width.
    fn line_news(&self, template: &NewItem, points: &[GeoPoint], group: ItemId) -> Result<Vec<NewItem>, EditError> {
        let radius = self.radius_of(template.body)?;
        let width = *template.params.get("width_m").ok_or_else(|| {
            EditError::Invalid(ItemFileError { key: "width_m".into(), why: format!("a {} needs a width_m parameter", template.kind) })
        })?;
        let parts = polyline::split_line(points, width, radius)?;
        let split = parts.len() > 1;
        Ok(parts
            .iter()
            .enumerate()
            .map(|(k, part)| NewItem {
                anchor: Anchor { latitude_deg: part[0].lat_deg, longitude_deg: part[0].lon_deg, ..template.anchor },
                reach_m: polyline::extent_m(part, radius) + width + polyline::REACH_SLACK_M,
                points: part.iter().map(|p| (p.lat_deg, p.lon_deg)).collect(),
                group: split.then_some(group),
                part: split.then_some(u32::try_from(k).expect("a part index")),
                mask_layer: None,
                ..template.clone()
            })
            .collect())
    }

    /// Put a line (a `ridge` or `channel` made like `template`, which names the width and the amount in its parameters) through `points`,
    /// split into linked parts when it is longer than one item reaches: one command, one undo. Returns the ids of the parts in order.
    pub fn put_line(&mut self, template: NewItem, points: &[GeoPoint]) -> Result<(Vec<ItemId>, Vec<Affected>), EditError> {
        self.no_open_edit()?;
        let group = self.ids.next_id();
        let news = self.line_news(&template, points, group)?;
        self.group(news.into_iter().map(GroupStep::Put).collect())
    }

    /// The items of the line `id` belongs to, by part (the item alone when it is not split).
    pub fn line_items(&self, id: ItemId) -> Result<Vec<ItemId>, EditError> {
        let item = self.items.get(&id).ok_or(EditError::NoSuchItem(id))?;
        let Some(group) = item.group else { return Ok(vec![id]) };
        let mut members: Vec<(u32, ItemId)> =
            self.items.values().filter(|i| i.group == Some(group)).map(|i| (i.part.unwrap_or(0), i.id)).collect();
        members.sort();
        Ok(members.into_iter().map(|(_, id)| id).collect())
    }

    /// The points of the line `id` belongs to, joined from its parts (a boundary point shared by two parts once).
    pub fn line_points(&self, id: ItemId) -> Result<Vec<GeoPoint>, EditError> {
        let mut points: Vec<GeoPoint> = Vec::new();
        for member in self.line_items(id)? {
            let item = &self.items[&member];
            for (k, (lat, lon)) in item.points.iter().enumerate() {
                if k == 0 && !points.is_empty() {
                    continue;
                }
                points.push(GeoPoint::new(*lat, *lon));
            }
        }
        Ok(points)
    }

    /// Give the line `id` belongs to new points: its parts are made again (more, fewer or as many as before) and the items of the line
    /// are changed, put or deleted to match, as one command; the line keeps its kind, layer, parameters, name and ids where it can.
    pub fn set_line_points(&mut self, id: ItemId, points: &[GeoPoint]) -> Result<(Vec<ItemId>, Vec<Affected>), EditError> {
        self.no_open_edit()?;
        let members = self.line_items(id)?;
        let first = self.items[&members[0]].clone();
        let group = first.group.unwrap_or_else(|| self.ids.next_id());
        let template = NewItem {
            body: first.body,
            surface: first.surface.clone(),
            layer: first.layer.clone(),
            priority: first.priority,
            kind: first.kind.clone(),
            anchor: first.anchor,
            reach_m: first.reach_m,
            max_delta_m: first.max_delta_m,
            params: first.params.clone(),
            name: first.name.clone(),
            notes: first.notes.clone(),
            points: Vec::new(),
            group: None,
            part: None,
            mask_layer: None,
        };
        let news = self.line_news(&template, points, group)?;
        let mut steps = Vec::new();
        for (k, new) in news.into_iter().enumerate() {
            steps.push(match members.get(k) {
                Some(existing) => GroupStep::Change(*existing, new),
                None => GroupStep::Put(new),
            });
        }
        for surplus in members.iter().skip(steps.len()) {
            steps.push(GroupStep::Delete(*surplus));
        }
        self.group(steps)
    }

    /// Delete the line `id` belongs to, all its parts, as one command.
    pub fn delete_line(&mut self, id: ItemId) -> Result<Vec<Affected>, EditError> {
        self.no_open_edit()?;
        let steps = self.line_items(id)?.into_iter().map(GroupStep::Delete).collect();
        self.group(steps).map(|(_, affected)| affected)
    }

    /// Put a region (a `region` made like `template`, with its surface type and mask in the parameters and `mask_layer`) over `points`.
    /// A region cannot be split (a mask of pieces is not the mask): one beyond the reach of an item is refused with the limit.
    pub fn put_region(&mut self, template: NewItem, points: &[GeoPoint]) -> Result<(ItemId, Vec<Affected>), EditError> {
        self.no_open_edit()?;
        let new = self.region_new(&template, points)?;
        self.put(new)
    }

    /// Give the region `id` new points, keeping its type, mask and name: one change.
    pub fn set_region_points(&mut self, id: ItemId, points: &[GeoPoint]) -> Result<Vec<Affected>, EditError> {
        self.no_open_edit()?;
        let item = self.items.get(&id).ok_or(EditError::NoSuchItem(id))?.clone();
        let template = NewItem {
            body: item.body,
            surface: item.surface.clone(),
            layer: item.layer.clone(),
            priority: item.priority,
            kind: item.kind.clone(),
            anchor: item.anchor,
            reach_m: item.reach_m,
            max_delta_m: item.max_delta_m,
            params: item.params.clone(),
            name: item.name.clone(),
            notes: item.notes.clone(),
            points: Vec::new(),
            group: None,
            part: None,
            mask_layer: item.mask_layer.clone(),
        };
        let new = self.region_new(&template, points)?;
        self.change(id, |i| {
            i.anchor = new.anchor;
            i.reach_m = new.reach_m;
            i.points = new.points.clone();
        })
    }

    fn region_new(&self, template: &NewItem, points: &[GeoPoint]) -> Result<NewItem, EditError> {
        let radius = self.radius_of(template.body)?;
        polyline::validate_region(points, radius)?;
        Ok(NewItem {
            anchor: Anchor { latitude_deg: points[0].lat_deg, longitude_deg: points[0].lon_deg, ..template.anchor },
            reach_m: polyline::extent_m(points, radius) + polyline::REACH_SLACK_M,
            points: points.iter().map(|p| (p.lat_deg, p.lon_deg)).collect(),
            group: None,
            part: None,
            ..template.clone()
        })
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
        self.revision += 1;
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
        let command = Command::one(Some(open.before.clone()), Some(open.now.clone()));
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
        self.revision += 1;
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
        let inverse = command.inverse();
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

    /// Bring the store, the definition and the item copy to the state after `command`, step by step. A failure leaves everything as it
    /// was: the steps done before it are taken back, the index refuses an item before it changes anything, and a store failure puts the
    /// definition back.
    fn run(&mut self, command: &Command) -> Result<Vec<Affected>, EditError> {
        self.revision += 1;
        for (k, step) in command.steps.iter().enumerate() {
            if let Err(e) = self.run_step(step) {
                for done in command.steps[..k].iter().rev() {
                    let _ = self.run_step(&done.inverse());
                }
                return Err(e);
            }
        }
        Ok(command.affected())
    }

    fn run_step(&mut self, step: &Step) -> Result<(), EditError> {
        let id = step.id();
        let current = self.items.get(&id).cloned();
        match (&step.after, current) {
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
        Ok(())
    }
}
