//! Tier A: the region polygons of the regions overlay (VIEW-030): which regions a frame carries, the nearest eight with the rest counted, the
//! text form, and that the plane the fragment stage tests in agrees with the polygon test that decides the stamp mask (WDEF-012).

use planet_core::Vec3;
use planet_frame::regions::{region_list, RegionList, MAX_REGIONS};
use planet_world_def::footprint::{Cap, FootprintItem};
use planet_world_def::item::{Anchor, Item};
use planet_world_def::region::{polygon_contains, SURFACE_TYPES};
use planet_world_def::{BodyId, WorldDefinition};
use std::collections::BTreeMap;

const R: f64 = 6_371_000.0;

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (la, lo) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la))
}

/// A square region of about 2 km a side whose first corner is at (`lat`, `lon`), of surface type `surface`, filed in its own layer.
fn square(id: u128, lat: f64, lon: f64, surface: f64) -> Item {
    let d = 0.01;
    Item {
        id,
        body: BodyId::PLANET,
        surface: "sphere".into(),
        layer: "regions".into(),
        priority: 0,
        kind: "region".into(),
        anchor: Anchor { latitude_deg: lat, longitude_deg: lon, height_m: 0.0, heading_deg: 0.0 },
        reach_m: 20_000.0,
        max_delta_m: 0.0,
        params: BTreeMap::from([("surface_type".to_string(), surface), ("mask_mode".to_string(), 0.0)]),
        base_hash: 1,
        generator_version: 1,
        stage_version: 1,
        name: None,
        notes: None,
        points: vec![(lat, lon), (lat, lon + d), (lat + d, lon + d), (lat + d, lon)],
        group: None,
        part: None,
        mask_layer: None,
    }
}

fn items_of(regions: &[Item]) -> Vec<FootprintItem> {
    let mut def = WorldDefinition::single_planet(R, 7);
    for r in regions {
        r.put_into(&mut def).unwrap();
    }
    def.footprint_items_cap(BodyId::PLANET, "regions", Cap { centre: Vec3::new(1.0, 0.0, 0.0), angle: core::f64::consts::PI }).unwrap()
}

// spec: VIEW-030
#[test]
fn a_frame_carries_the_eight_nearest_regions_and_counts_the_rest() {
    // Ten squares along the equator, 0.1 degree (about 11 km) apart; the camera above the first.
    let regions: Vec<Item> = (0..10).map(|i| square(100 + i as u128, 0.0, 0.1 * i as f64, (i % 9) as f64)).collect();
    let items = items_of(&regions);
    let camera = unit(0.0, 0.0) * (R + 500.0);
    let list = region_list(&items, camera, R);
    assert_eq!((list.polys.len(), list.dropped), (MAX_REGIONS, 2));
    // Nearest first, so the two farthest (squares 8 and 9) are the ones left out.
    let nearest = &list.polys[0];
    assert!(
        nearest.anchor.iter().map(|v| f64::from(*v) * f64::from(*v)).sum::<f64>().sqrt() < 600.0,
        "the first is right under the camera"
    );
    let dists: Vec<f64> = list.polys.iter().map(|p| p.anchor.iter().map(|v| f64::from(*v) * f64::from(*v)).sum::<f64>().sqrt()).collect();
    assert!(dists.windows(2).all(|w| w[0] <= w[1]), "nearest first: {dists:?}");
    let none = region_list(&items_of(&[]), camera, R);
    assert_eq!(none, RegionList::default());
    assert_eq!(none.to_text(), "", "a frame without regions has no region text");
}

// spec: VIEW-030
#[test]
fn regions_at_the_same_distance_go_by_item_id_whatever_the_order_they_were_put_in() {
    // Two regions on the same anchor, the higher id put first.
    let items = items_of(&[square(10, 20.0, 30.0, 4.0), square(9, 20.0, 30.0, 6.0)]);
    let list = region_list(&items, unit(20.0, 30.0) * (R + 100.0), R);
    assert_eq!(list.polys.len(), 2);
    let lin = |c: u8| {
        let c = f64::from(c) / 255.0;
        (if c <= 0.04045 { c / 12.92 } else { libm::pow((c + 0.055) / 1.055, 2.4) }) as f32
    };
    let first = SURFACE_TYPES[6].tint;
    assert_eq!(list.polys[0].tint, [lin(first[0]), lin(first[1]), lin(first[2])], "id 9 (surface 6) comes before id 10");
}

// spec: VIEW-030
#[test]
fn the_list_round_trips_through_its_text_and_malformed_text_is_an_error() {
    let regions = [square(1, 20.0, 30.0, 2.0), square(2, 20.0, 30.05, 5.0), square(3, 20.0, 30.1, 8.0)];
    let list = region_list(&items_of(&regions), unit(20.0, 30.0) * (R + 100.0), R);
    assert_eq!(list.polys.len(), 3);
    let text = list.to_text();
    assert_eq!(text.lines().count(), 4, "a header and one line per polygon");
    assert_eq!(RegionList::from_text(&text).unwrap(), list);
    assert_eq!(RegionList::from_text("").unwrap(), RegionList::default());
    let nine = text.replacen("regions 3", "regions 9", 1);
    assert!(RegionList::from_text(&nine).unwrap_err().contains("at most 8"));
    let short = text.lines().take(3).collect::<Vec<_>>().join("\n");
    assert!(RegionList::from_text(&short).unwrap_err().contains("announced"));
    assert!(RegionList::from_text(&text.replace(" band ", " bend ")).unwrap_err().contains("band"));
}

// spec: VIEW-030
#[test]
fn the_tint_is_the_surface_types_in_linear_light() {
    let list = region_list(&items_of(&[square(1, 20.0, 30.0, 3.0)]), unit(20.0, 30.0) * R, R);
    let [r, g, b] = SURFACE_TYPES[3].tint;
    let lin = |c: u8| {
        let c = f64::from(c) / 255.0;
        (if c <= 0.04045 { c / 12.92 } else { libm::pow((c + 0.055) / 1.055, 2.4) }) as f32
    };
    assert_eq!(list.polys[0].tint, [lin(r), lin(g), lin(b)]);
}

/// What the fragment stage computes for a point of the surface, in f64: the point relative to the camera, the anchor subtracted, projected
/// on the basis, then the even-odd rule on the polygon's points.
fn shader_inside(p: &planet_frame::regions::RegionPoly, camera: Vec3, point: Vec3) -> bool {
    let rel = point * R - camera;
    let d = [rel.x - f64::from(p.anchor[0]), rel.y - f64::from(p.anchor[1]), rel.z - f64::from(p.anchor[2])];
    let dot = |v: &[f32; 3]| d[0] * f64::from(v[0]) + d[1] * f64::from(v[1]) + d[2] * f64::from(v[2]);
    if dot(&p.up).abs() > f64::from(p.band_m) {
        return false;
    }
    let q = [dot(&p.e1), dot(&p.e2)];
    if q[0] * q[0] + q[1] * q[1] > f64::from(p.reach_sq) {
        return false;
    }
    let pts = &p.points;
    let mut inside = false;
    let mut j = pts.len() - 1;
    for i in 0..pts.len() {
        let (pi, pj) = ([f64::from(pts[i][0]), f64::from(pts[i][1])], [f64::from(pts[j][0]), f64::from(pts[j][1])]);
        if (pi[1] > q[1]) != (pj[1] > q[1]) && q[0] < (pj[0] - pi[0]) * (q[1] - pi[1]) / (pj[1] - pi[1]) + pi[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

// spec: VIEW-030, WDEF-012
#[test]
fn the_planes_test_agrees_with_the_mask_polygon_test_away_from_the_edges_and_rejects_the_far_side() {
    // A concave L about 4 km across at 20N 30E, a camera 3 km above it, and a grid of points over and around it.
    let l = [(0.0, 0.0), (0.0, 0.04), (0.02, 0.04), (0.02, 0.02), (0.04, 0.02), (0.04, 0.0)];
    let mut item = square(7, 20.0, 30.0, 4.0);
    item.points = l.iter().map(|(a, b)| (20.0 + a, 30.0 + b)).collect();
    let list = region_list(&items_of(&[item.clone()]), unit(20.0, 30.0) * (R + 3_000.0), R);
    let poly = &list.polys[0];
    let dirs: Vec<Vec3> = item.points.iter().map(|(la, lo)| unit(*la, *lo)).collect();
    let camera = unit(20.0, 30.0) * (R + 3_000.0);
    let (mut inside, mut outside) = (0, 0);
    for i in -4..=10 {
        for j in -4..=10 {
            let p = unit(20.0 + 0.005 * f64::from(i) + 0.0013, 30.0 + 0.005 * f64::from(j) + 0.0007);
            let want = polygon_contains(&dirs, p);
            assert_eq!(shader_inside(poly, camera, p), want, "grid ({i}, {j})");
            if want {
                inside += 1;
            } else {
                outside += 1;
            }
        }
    }
    assert!(inside > 10 && outside > 10, "the grid has points on both sides ({inside}, {outside})");
    // The antipode of the anchor projects into the plane too, but is far from it.
    assert!(!shader_inside(poly, camera, unit(-20.0, 210.0)));
    // The far side of a point inside the polygon projects inside it too: only the band keeps it out.
    assert!(!shader_inside(poly, camera, -unit(20.01, 30.01)));
    assert!(shader_inside(poly, camera, unit(20.01, 30.01)));
}
