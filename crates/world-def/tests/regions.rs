//! Tier A: regions (WDEF-012, ADR 0069): the table of surface types, the polygon geometry, the refusals at the put, the layer a region is
//! filed in and its footprint and re-key.

use planet_core::{TangentWarp, TileId, Vec3};
use planet_world_def::item::{Anchor, Item};
use planet_world_def::region::{polygon_contains, polygon_self_intersects, surface_type_index, SURFACE_TYPES};
use planet_world_def::{footprint_hash, BodyId, GenContext, WorldDefinition};
use std::collections::BTreeMap;

const R: f64 = 6_371_000.0;

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (la, lo) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la))
}

fn poly(points: &[(f64, f64)]) -> Vec<Vec3> {
    points.iter().map(|(la, lo)| unit(*la, *lo)).collect()
}

/// A region item: `mode` 0 or 2 (or whatever a test refuses), clipping `mask_layer`.
fn region(points: &[(f64, f64)], surface: f64, mode: f64, mask_layer: Option<&str>) -> Item {
    Item {
        id: 9,
        body: BodyId::PLANET,
        surface: "sphere".into(),
        layer: "regions".into(),
        priority: 0,
        kind: "region".into(),
        anchor: Anchor { latitude_deg: points[0].0, longitude_deg: points[0].1, height_m: 0.0, heading_deg: 0.0 },
        reach_m: 20_000.0,
        max_delta_m: 0.0,
        params: BTreeMap::from([("surface_type".to_string(), surface), ("mask_mode".to_string(), mode)]),
        base_hash: 1,
        generator_version: 1,
        stage_version: 1,
        name: None,
        notes: None,
        points: points.to_vec(),
        group: None,
        part: None,
        mask_layer: mask_layer.map(str::to_string),
    }
}

/// A square of about 4 km a side, corner first, around (20.0, 30.0).
fn square() -> Vec<(f64, f64)> {
    vec![(19.98, 29.98), (19.98, 30.02), (20.02, 30.02), (20.02, 29.98)]
}

// spec: WDEF-012
#[test]
fn the_table_has_the_nine_surface_types_in_the_owners_order_each_with_a_tint() {
    let names: Vec<&str> = SURFACE_TYPES.iter().map(|s| s.name).collect();
    assert_eq!(names, ["rock", "sand", "grass", "snow", "forest", "ice", "desert", "wetland", "urban"]);
    assert!(SURFACE_TYPES.iter().all(|s| s.tint != [0, 0, 0]));
    assert_eq!((surface_type_index("grass"), surface_type_index("urban"), surface_type_index("lava")), (Some(2), Some(8), None));
    let mut distinct: Vec<[u8; 3]> = SURFACE_TYPES.iter().map(|s| s.tint).collect();
    distinct.dedup();
    assert_eq!(distinct.len(), 9, "the tints differ");
}

// spec: WDEF-012
#[test]
fn a_point_is_inside_a_polygon_on_an_edge_or_at_a_vertex_and_outside_otherwise_whatever_the_start_and_the_direction() {
    // A concave L: (0,0) (0,2) (1,2) (1,1) (2,1) (2,0) in units of 0.01 degree around (20, 30).
    let l: Vec<(f64, f64)> = [(0.0, 0.0), (0.0, 2.0), (1.0, 2.0), (1.0, 1.0), (2.0, 1.0), (2.0, 0.0)]
        .iter()
        .map(|(a, b)| (20.0 + 0.01 * a, 30.0 + 0.01 * b))
        .collect();
    let pts = poly(&l);
    let at = |a: f64, b: f64| unit(20.0 + 0.01 * a, 30.0 + 0.01 * b);
    let inside = [at(0.5, 0.5), at(0.5, 1.5), at(1.5, 0.5), at(0.1, 1.9)];
    let outside = [at(1.5, 1.5), at(2.5, 0.5), at(-0.5, 0.5), at(1.2, 1.5), at(0.5, 2.5)];
    // The middles of the edges are on the great circles between the vertices (the parallel through two vertices is a few centimetres off).
    let on_edge: Vec<Vec3> = (0..pts.len()).map(|i| (pts[i] + pts[(i + 1) % pts.len()]).normalized()).collect();
    let vertices: Vec<Vec3> = pts.clone();
    for k in 0..pts.len() {
        // Every start and both directions.
        let mut rotated: Vec<Vec3> = pts[k..].iter().chain(pts[..k].iter()).copied().collect();
        for reversed in [false, true] {
            if reversed {
                rotated.reverse();
            }
            for p in inside.iter().chain(on_edge.iter()).chain(vertices.iter()) {
                assert!(polygon_contains(&rotated, *p), "start {k}, reversed {reversed}: {p:?} is inside or on");
            }
            for p in &outside {
                assert!(!polygon_contains(&rotated, *p), "start {k}, reversed {reversed}: {p:?} is outside");
            }
        }
    }
    // The other side of the planet is outside, and so is a point behind the tangent plane.
    assert!(!polygon_contains(&pts, unit(-20.0, 210.0)));
    // Near the pole and across the antimeridian.
    let near_pole = poly(&[(89.99, 0.0), (89.99, 120.0), (89.99, -120.0)]);
    assert!(polygon_contains(&near_pole, unit(90.0, 0.0)) && !polygon_contains(&near_pole, unit(89.9, 60.0)));
    let across = poly(&[(0.0, 179.99), (0.0, -179.99), (0.02, -179.99), (0.02, 179.99)]);
    assert!(
        polygon_contains(&across, unit(0.01, 180.0))
            && !polygon_contains(&across, unit(0.03, 180.0))
            && !polygon_contains(&across, unit(0.01, 179.9))
    );
}

// spec: WDEF-012
#[test]
fn a_polygon_that_crosses_itself_is_found_and_a_simple_one_is_not() {
    assert!(!polygon_self_intersects(&poly(&square())));
    // A bow tie: the edges cross.
    assert!(polygon_self_intersects(&poly(&[(19.98, 29.98), (20.02, 30.02), (20.02, 29.98), (19.98, 30.02)])));
    // A spike that doubles back over its own edge, a vertex on another edge.
    assert!(polygon_self_intersects(&poly(&[(19.98, 29.98), (19.98, 30.02), (20.02, 30.0), (19.98, 30.0), (20.02, 29.98)])));
    let l = poly(&[(20.0, 30.0), (20.0, 30.02), (20.01, 30.02), (20.01, 30.01), (20.02, 30.01), (20.02, 30.0)]);
    assert!(!polygon_self_intersects(&l), "a concave L is simple");
    // A figure 8 through a repeated vertex that is not a neighbour is found, whatever vertex it starts at.
    let eight = [(19.98, 29.98), (20.02, 30.02), (20.02, 29.98), (19.98, 30.0), (20.02, 30.02), (19.98, 30.02)];
    for k in 0..eight.len() {
        let rotated: Vec<(f64, f64)> = eight[k..].iter().chain(eight[..k].iter()).copied().collect();
        assert!(polygon_self_intersects(&poly(&rotated)), "figure 8 from {k}");
    }
    // A polygon with no area is no region: collinear points on one great circle, and a spike doubling back along an edge.
    let (a, b) = (unit(20.0, 30.0), unit(20.0, 30.02));
    let mid = (a + b).normalized();
    assert!(polygon_self_intersects(&[a, mid, b]), "three points on one great circle");
}

fn refused(item: &Item, want: &str) {
    let mut def = WorldDefinition::single_planet(R, 7);
    let before = def.clone();
    let e = match item.validate() {
        Err(e) => e.to_string(),
        Ok(()) => item.put_into(&mut def).expect_err("the region must be refused").to_string(),
    };
    assert!(e.contains(want), "`{want}` in `{e}`");
    assert_eq!(def, before, "a refused put changes nothing");
}

// spec: WDEF-012
#[test]
fn a_region_that_cannot_be_one_is_refused_naming_the_reason_and_changes_nothing() {
    refused(&region(&square(), 9.0, 2.0, Some("stamps")), "surface type");
    refused(&region(&square(), 1.5, 2.0, Some("stamps")), "surface type");
    refused(&region(&square(), -1.0, 2.0, Some("stamps")), "surface type");
    refused(&region(&square(), 2.0, 3.0, Some("stamps")), "mask mode");
    refused(&region(&square(), 2.0, 0.5, Some("stamps")), "mask mode");
    refused(&region(&square(), 2.0, 1.0, Some("stamps")), "clip outside");
    refused(&region(&square(), 2.0, 2.0, None), "needs the layer");
    refused(&region(&square(), 2.0, 0.0, Some("stamps")), "mask_mode 0");
    refused(&region(&square()[..2], 2.0, 2.0, Some("stamps")), "points");
    let bow = [(19.98, 29.98), (20.02, 30.02), (20.02, 29.98), (19.98, 30.02)];
    refused(&region(&bow, 2.0, 2.0, Some("stamps")), "crosses itself");
    let mut far = region(&square(), 2.0, 2.0, Some("stamps"));
    far.reach_m = 1_000.0;
    refused(&far, "above its reach");
    let mut repeated = square();
    repeated.push(repeated[0]);
    refused(&region(&repeated, 2.0, 2.0, Some("stamps")), "coincide");
    // The accepted ones: every surface type, a label (mode 0), a clip.
    for s in 0..9 {
        let mut def = WorldDefinition::single_planet(R, 7);
        region(&square(), f64::from(s), 2.0, Some("stamps")).put_into(&mut def).unwrap();
    }
    let mut def = WorldDefinition::single_planet(R, 7);
    region(&square(), 3.0, 0.0, None).put_into(&mut def).unwrap();
}

// spec: WDEF-012
#[test]
fn a_region_that_clips_is_filed_in_the_layer_it_clips_and_rekeys_only_what_it_touches() {
    let under = TileId::from_direction(&TangentWarp, unit(20.0, 30.0), 13);
    let far = TileId::from_direction(&TangentWarp, unit(-20.0, 210.0), 13);
    let read = |item: &Item, layer: &str, tile: TileId| {
        let mut def = WorldDefinition::single_planet(R, 7);
        item.put_into(&mut def).unwrap();
        let ctx = GenContext::new(&def);
        let items = ctx.query_footprint(BodyId::PLANET, layer, tile, 0.25).unwrap();
        (footprint_hash(&items), items)
    };
    let clip = region(&square(), 2.0, 2.0, Some("stamps"));
    let (h, items) = read(&clip, "stamps", under);
    assert_eq!(items.len(), 1, "the tile of the clipped layer under the region reads it");
    assert_eq!(items[0].stamp.as_ref().unwrap().points().len(), 4, "with its points");
    assert!(read(&clip, "regions", under).1.is_empty(), "the region's own layer does not read a clip of another");
    assert!(read(&clip, "stamps", far).1.is_empty(), "a tile away from it does not");
    // A label stays in its own layer and the layer it would clip reads nothing.
    let label = region(&square(), 2.0, 0.0, None);
    assert_eq!(read(&label, "regions", under).1.len(), 1);
    assert!(read(&label, "stamps", under).1.is_empty());
    // A moved point, the surface type and the layer re-key the tile; the same region under another name does not.
    let mut moved = square();
    moved[2].1 = 30.03;
    assert_ne!(read(&region(&moved, 2.0, 2.0, Some("stamps")), "stamps", under).0, h, "a point");
    assert_ne!(read(&region(&square(), 5.0, 2.0, Some("stamps")), "stamps", under).0, h, "the surface type");
    let mut named = clip.clone();
    named.name = Some("a name".into());
    named.notes = Some("notes".into());
    assert_eq!(read(&named, "stamps", under).0, h, "a name changes nothing");
}
