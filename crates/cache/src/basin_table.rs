//! The basin table of a bake (STRM-080, GEN-163; architect D16): the lakes of one cube face as one record of [`KIND_BASIN_TABLE`] (19), sorted by basin id, filed under the face's root tile.
//!
//! The lake producer's records are bake output: the level, the class and `q_out` per basin are computed once and stored; the runtime only reads them (loads a region's table once and indexes by tile).
//! Per-tile copies are not stored: a lake lists the tiles that hold its water and the runtime indexes the table by them.
//!
//! The key carries everything the bytes depend on (the table layout version, the producer version, the hydrology parameters, the tile grid, the identity of the base): a changed one is another key and the
//! table reads as absent, never as wrong. The payload is
//! `codec (u8) | table version (u32) | producer version (u32) | tile cells (u32) | tiles per edge (u32) | face (u8) | lakes (u32) | key (16) |` the lakes, each
//! `id, floor id, spill id (u64) | floor, spill, level (f64) | q_in, q_out, area (f64) | cells (u32) | flags (u8: class in bits 0-1, overflows bit 2, wet bit 3) | tiles (u16) |` the tiles, each
//! `face (u8) | ti (u32) | tj (u32)`, all little-endian. A decoder reserves no more than the bytes it was given allow.

use planet_core::Face;
use planet_generators::bake::hydro::{BasinClass, HydroParams};
use planet_generators::bake::lakes::{Lake, TileRef, LAKES_VERSION};

use crate::global_store::GlobalWorld;
use crate::key::{cache_key, CacheKey, Kind, PARAM_BASIN_TABLE, PARAM_RESOLUTION};
use crate::pack::{Record, TileStore};
use crate::relief_store::face_tile;

/// The kind byte of a basin table.
pub const KIND_BASIN_TABLE: u8 = Kind::BasinTable as u8;
/// Version of the layout and of the meaning of the key. History: 1 = the layout of this file.
pub const BASIN_TABLE_VERSION: u32 = 1;
const CODEC_VERSION: u8 = 1;
const IMPLEMENTATION_ID: &str = "basin-table";
const HEADER_LEN: usize = 1 + 4 + 4 + 4 + 4 + 1 + 4 + 16;
/// The fixed part of a lake: three ids, six f64, the cells, the flags and the tile count.
const LAKE_FIXED: usize = 8 * 3 + 8 * 6 + 4 + 1 + 2;
const TILE_LEN: usize = 1 + 4 + 4;

/// Why a stored table could not be used. Every message names the record and what was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasinTableError(pub String);

impl std::fmt::Display for BasinTableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a basin table is made of besides the bake it is beside.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BasinSpec {
    /// The hydrology parameters the basins were found with.
    pub hydro: HydroParams,
    /// The cells of a tile side and the tiles of a face edge: the grid the tile lists are on.
    pub tile_cells: u32,
    pub tiles_per_edge: u32,
    /// The identity of the base the surface was computed on (a changed base is another table).
    pub base_id: u64,
}

impl BasinSpec {
    fn bytes(&self) -> Vec<u8> {
        let h = &self.hydro;
        assert!(h.initiation_m2.is_finite() && h.pit_min_m.is_finite() && h.min_depth_m.is_finite(), "the hydrology parameters are finite");
        let mut b = Vec::with_capacity(72);
        b.extend(BASIN_TABLE_VERSION.to_le_bytes());
        b.extend(LAKES_VERSION.to_le_bytes());
        b.extend(planet_generators::bake::BAKE_KERNEL_VERSION.to_le_bytes());
        b.extend(h.initiation_m2.to_bits().to_le_bytes());
        b.extend(h.pit_min_m.to_bits().to_le_bytes());
        b.extend(h.min_cells.to_le_bytes());
        b.extend(h.min_depth_m.to_bits().to_le_bytes());
        b.extend(h.body.to_le_bytes());
        b.extend(self.tile_cells.to_le_bytes());
        b.extend(self.tiles_per_edge.to_le_bytes());
        b.extend(self.base_id.to_le_bytes());
        b
    }
}

/// The key of a face table.
pub fn basin_key(world: &GlobalWorld, res: u32, spec: &BasinSpec, face: Face) -> CacheKey {
    let mut inputs = world.base_key_inputs(Kind::BasinTable, face_tile(face), res, IMPLEMENTATION_ID);
    inputs.output_params.insert(PARAM_RESOLUTION, spec.tile_cells.to_le_bytes().to_vec());
    inputs.output_params.insert(PARAM_BASIN_TABLE, spec.bytes());
    cache_key(&inputs)
}

fn flags(l: &Lake) -> u8 {
    let class = match l.class {
        BasinClass::Lake => 0u8,
        BasinClass::Endorheic => 1,
        BasinClass::Intentional => 2,
    };
    class | (u8::from(l.overflows) << 2) | (u8::from(l.wet) << 3)
}

/// The record of a face table: `lakes` in any order (the record is sorted by id). Panics on a duplicate id or a lake with more than 65,535 tiles (neither is a bake output).
pub fn basin_table_record(world: &GlobalWorld, res: u32, spec: &BasinSpec, face: Face, lakes: &[Lake]) -> Record {
    let key = basin_key(world, res, spec, face);
    let mut order: Vec<&Lake> = lakes.iter().collect();
    order.sort_unstable_by_key(|l| l.id);
    assert!(order.windows(2).all(|w| w[0].id != w[1].id), "a basin id appears once in a table");
    let mut p = Vec::with_capacity(HEADER_LEN + order.len() * (LAKE_FIXED + TILE_LEN));
    p.push(CODEC_VERSION);
    p.extend(BASIN_TABLE_VERSION.to_le_bytes());
    p.extend(LAKES_VERSION.to_le_bytes());
    p.extend(spec.tile_cells.to_le_bytes());
    p.extend(spec.tiles_per_edge.to_le_bytes());
    p.push(face.0);
    p.extend((order.len() as u32).to_le_bytes());
    p.extend(key.0);
    for l in order {
        for v in [l.id, l.floor_id, l.spill_id] {
            p.extend(v.to_le_bytes());
        }
        for v in [l.floor_m, l.spill_m, l.level_m, l.q_in, l.q_out, l.area_m2] {
            p.extend(v.to_bits().to_le_bytes());
        }
        p.extend(l.cells.to_le_bytes());
        p.push(flags(l));
        let n = u16::try_from(l.tiles.len()).expect("a lake lists at most 65,535 tiles");
        p.extend(n.to_le_bytes());
        for t in &l.tiles {
            p.push(t.face);
            p.extend(t.ti.to_le_bytes());
            p.extend(t.tj.to_le_bytes());
        }
    }
    Record {
        key,
        kind: KIND_BASIN_TABLE,
        tile: face_tile(face),
        generator_version: planet_generators::global::GLOBAL_VERSION,
        hash_version: planet_core::hash::HASH_VERSION,
        node_set_id: world.node_set_id(),
        payload: p,
    }
}

/// The records of all the faces: `face_of` names the face a lake belongs to (the face of its lowest cell). A face without a lake still gets a record (an empty table), so that "no lakes" is stored, not absent.
pub fn basin_table_records(
    world: &GlobalWorld,
    res: u32,
    spec: &BasinSpec,
    lakes: &[Lake],
    face_of: &dyn Fn(&Lake) -> Face,
) -> Vec<Record> {
    (0..6u8)
        .map(|f| {
            let mine: Vec<Lake> = lakes.iter().filter(|l| face_of(l).0 == f).cloned().collect();
            basin_table_record(world, res, spec, Face(f), &mine)
        })
        .collect()
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().expect("four bytes"))
}

fn u64_at(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().expect("eight bytes"))
}

fn f64_at(b: &[u8], at: usize) -> f64 {
    f64::from_bits(u64_at(b, at))
}

/// The stored table of a face: `Ok(None)` when the store holds none under this key (another spec or version, or a store from before the table); an error naming the record and what was found when the
/// record is damaged: another codec, a tile grid or a producer version that is not the spec's, a count the payload cannot hold, lakes out of id order, a bad class or a tile outside the grid.
pub fn load_basin_table(
    store: &TileStore,
    world: &GlobalWorld,
    res: u32,
    spec: &BasinSpec,
    face: Face,
) -> Result<Option<Vec<Lake>>, BasinTableError> {
    let key = basin_key(world, res, spec, face);
    let what = |why: String| BasinTableError(format!("basin table (record kind {KIND_BASIN_TABLE}) face {} key {key}: {why}", face.0));
    let Some(rec) = store.get(KIND_BASIN_TABLE, face_tile(face), key).map_err(|e| what(e.to_string()))? else {
        return Ok(None);
    };
    let p = &rec.payload;
    if p.len() < HEADER_LEN || p[0] != CODEC_VERSION {
        return Err(what(format!("a payload of {} bytes that is not codec version {CODEC_VERSION}", p.len())));
    }
    let (table, producer, cells, per_edge) = (u32_at(p, 1), u32_at(p, 5), u32_at(p, 9), u32_at(p, 13));
    if table != BASIN_TABLE_VERSION || producer != LAKES_VERSION {
        return Err(what(format!(
            "it holds table version {table} and producer version {producer}, the code has {BASIN_TABLE_VERSION} and {LAKES_VERSION}"
        )));
    }
    if (cells, per_edge) != (spec.tile_cells, spec.tiles_per_edge) || p[17] != face.0 {
        return Err(what(format!(
            "it is on a grid of {per_edge} tiles of {cells} cells for face {}, the layout says {} tiles of {} cells for face {}",
            p[17], spec.tiles_per_edge, spec.tile_cells, face.0
        )));
    }
    let count = u32_at(p, 18) as usize;
    if p[22..38] != key.0 {
        return Err(what("the key inside the payload is not the record key".into()));
    }
    // Never reserve more than the payload can hold: a lake takes at least LAKE_FIXED bytes.
    if count > (p.len() - HEADER_LEN) / LAKE_FIXED {
        return Err(what(format!("{count} lakes cannot fit in {} bytes", p.len())));
    }
    let mut lakes: Vec<Lake> = Vec::with_capacity(count);
    let mut at = HEADER_LEN;
    for k in 0..count {
        if at + LAKE_FIXED > p.len() {
            return Err(what(format!("lake {k} is cut off")));
        }
        let b = &p[at..];
        let flag = b[LAKE_FIXED - 3];
        let class = match flag & 3 {
            0 => BasinClass::Lake,
            1 => BasinClass::Endorheic,
            2 => BasinClass::Intentional,
            c => return Err(what(format!("lake {k} has class code {c}"))),
        };
        if flag >> 4 != 0 {
            return Err(what(format!("lake {k} has flag bits {flag:#x} this version does not know")));
        }
        let n = u16::from_le_bytes(b[LAKE_FIXED - 2..LAKE_FIXED].try_into().expect("two bytes")) as usize;
        let total = LAKE_FIXED + n * TILE_LEN;
        if at + total > p.len() {
            return Err(what(format!("lake {k} lists {n} tiles that are cut off")));
        }
        let mut tiles: Vec<TileRef> = Vec::with_capacity(n);
        for t in 0..n {
            let o = LAKE_FIXED + t * TILE_LEN;
            let tile = TileRef { face: b[o], ti: u32_at(b, o + 1), tj: u32_at(b, o + 5) };
            if tile.face >= 6 || tile.ti >= spec.tiles_per_edge || tile.tj >= spec.tiles_per_edge {
                return Err(what(format!("lake {k} lists tile {tile:?} outside the grid of {} tiles a side", spec.tiles_per_edge)));
            }
            if tiles.last().is_some_and(|last| *last >= tile) {
                return Err(what(format!("lake {k} lists its tiles out of order")));
            }
            tiles.push(tile);
        }
        let l = Lake {
            id: u64_at(b, 0),
            floor_id: u64_at(b, 8),
            spill_id: u64_at(b, 16),
            floor_m: f64_at(b, 24),
            spill_m: f64_at(b, 32),
            level_m: f64_at(b, 40),
            q_in: f64_at(b, 48),
            q_out: f64_at(b, 56),
            area_m2: f64_at(b, 64),
            cells: u32_at(b, 72),
            class,
            overflows: flag & 4 != 0,
            wet: flag & 8 != 0,
            tiles,
        };
        if lakes.last().is_some_and(|last| last.id >= l.id) {
            return Err(what(format!("lake {k} (id {}) is out of id order or repeated", l.id)));
        }
        lakes.push(l);
        at += total;
    }
    if at != p.len() {
        return Err(what(format!("{} bytes follow the last lake", p.len() - at)));
    }
    Ok(Some(lakes))
}

const _: () = assert!(LAKE_FIXED == 8 * 3 + 8 * 6 + 4 + 1 + 2);
