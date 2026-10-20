use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_generators::height::HeightField;
const R: f64 = 6_371_000.0;
fn main() {
    let world = WorldTerrain::open(planet_world_def::WorldDefinition::single_planet(R, 1), 256).unwrap();
    let field = world.field().unwrap();
    let mut s = 0x1234567u64;
    let mut next = move || { s ^= s << 13; s ^= s >> 7; s ^= s << 17; (s >> 11) as f64 / (1u64 << 53) as f64 };
    let level = 15u8;
    let mut best = (0.0f64, Vec3::new(1.0, 0.0, 0.0));
    for _ in 0..4000 {
        let (z, a) = (2.0 * next() - 1.0, std::f64::consts::TAU * next());
        let r = libm::sqrt(1.0 - z * z);
        let t = TileId::from_direction(&TangentWarp, Vec3::new(r * libm::cos(a), r * libm::sin(a), z), level);
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for l in 0..=8 { for k in 0..=8 { let h = field.height(t.sample_direction(&TangentWarp, 8, k, l)); lo = lo.min(h); hi = hi.max(h); } }
        if hi - lo > best.0 { best = (hi - lo, t.center_direction(&TangentWarp)); }
    }
    println!("{:.1} m relief; dir {:.9},{:.9},{:.9}; ground {:.1} m", best.0, best.1.x, best.1.y, best.1.z, field.height(best.1));
}
