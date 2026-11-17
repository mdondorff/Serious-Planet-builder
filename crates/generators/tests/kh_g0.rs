//! Spike test (spike/kh-stage, spikes #137; not merged as is): KH (SPIKE-136's h2, G0 contracts reading 19) on the G0 fixture. KH(z_s) + r at L is deterministic and tile-local:
//! the same bits on 1 and 4 threads and under the fixture's two layouts (one region; two regions cut along the tile line at -15 degrees, G0 contract section 11).

#![allow(clippy::needless_range_loop)]

#[path = "support/g0_fixture.rs"]
pub(crate) mod fixture;

use fixture::*;
use planet_core::Face;
use planet_generators::bake::h0::GlobalGrid;
use planet_generators::bake::k7::RegionField;
use planet_generators::bake::kh;
use planet_generators::bake::route::NONE;
use planet_generators::bake::Grid;

/// KH(z_s) + r at every L cell, evaluated in the order `cells` gives (each value depends on its cell alone).
fn kh_surface(fx: &Fixture, order: &[usize]) -> (Vec<f64>, kh::SplitStats) {
    let nn = H1_N * H1_N;
    let gh = Grid::cube(H1_N, RADIUS_M);
    let outlet = gh.outlets(&fx.z_s, SEA_LEVEL_M);
    let (recv, area) = kh::routing(&gh, &fx.z_s, &outlet);
    let mut stats = kh::SplitStats::default();
    let splits: Vec<Vec<u8>> = (0..6)
        .map(|f| {
            let block = &fx.z_s[f * nn..(f + 1) * nn];
            let (s, st) = kh::split(
                block,
                H1_N,
                H1_N,
                &|c| {
                    let r = recv[f * nn + c];
                    (r != NONE && r as usize / nn == f).then(|| r as usize % nn)
                },
                &|c| area[f * nn + c],
            );
            stats.by_step += st.by_step;
            stats.both += st.both;
            stats.by_pair += st.by_pair;
            s
        })
        .collect();
    let l = GlobalGrid::new(L_N, RADIUS_M);
    let zb_fields: Vec<RegionField> =
        (0..6).map(|f| RegionField::new(Face(f as u8), H1_N, (0, 0), H1_N, H1_N, fx.z_b[f * nn..(f + 1) * nn].iter().map(|&x| x as f32).collect())).collect();
    let ln = L_N * L_N;
    let lr = L_PER_H1 as f64;
    let mut out = vec![f64::NAN; 6 * ln];
    for &p in order {
        let (f, i, j) = (p / ln, p % L_N, (p % ln) / L_N);
        let r = fx.b[p] - zb_fields[f].at(l.direction(p as u32));
        let z = kh::read(&fx.z_s[f * nn..(f + 1) * nn], H1_N, H1_N, &splits[f], (i as f64 - (lr - 1.0) / 2.0) / lr, (j as f64 - (lr - 1.0) / 2.0) / lr);
        out[p] = z + r;
    }
    (out, stats)
}

// spec: GEN-189 (G0 contracts reading 19, KH): deterministic, the same bits under both layouts and thread counts.
#[test]
fn kh_on_the_fixture_is_tile_local() {
    let fx4 = build(4);
    let fx1 = build(1);
    let all: Vec<usize> = (0..6 * L_N * L_N).collect();
    let (one, st) = kh_surface(&fx4, &all);
    let (serial, _) = kh_surface(&fx1, &all);
    let [_, two_layout] = layouts();
    let region = |p: usize| two_layout.region_of_h1[h1_of_l(p)];
    let two_order: Vec<usize> = (0..2u8).flat_map(|r| all.iter().copied().filter(move |&p| region(p) == r)).collect();
    let (two, _) = kh_surface(&fx4, &two_order);
    let differ = |a: &[f64], b: &[f64]| a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
    let max_dev = one.iter().zip(&fx4.s0).map(|(k, s)| (k - s).abs()).fold(0.0, f64::max);
    eprintln!(
        "KH on the G0 fixture: quads by step {}, both {}, by pair {}; two layouts {} cells differ; 1 against 4 threads {}; |KH + r - S0| max {max_dev:.2} m",
        st.by_step,
        st.both,
        st.by_pair,
        differ(&one, &two),
        differ(&one, &serial)
    );
    assert!(one.iter().all(|v| v.is_finite()), "every L cell read");
    assert_eq!(differ(&one, &two), 0, "the two-region layout gives the same bits");
    assert_eq!(differ(&one, &serial), 0, "1 and 4 threads give the same bits");
    assert!(st.by_step > 0, "the fixture's trunk gives diagonal D8 steps");
}
