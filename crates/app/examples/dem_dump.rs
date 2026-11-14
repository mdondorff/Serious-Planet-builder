//! SPIKE-116 (spike code): decode a GLO-30 tile to raw f32 (row-major, north first) for the picture sheet. `dem_dump <tif> <out.f32>`; prints the size.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t = planet_testkit::glo30::read(std::path::Path::new(&a[1])).expect("a GLO-30 tile");
    let (r, c) = t.size();
    let mut b = Vec::with_capacity(r * c * 4);
    for i in 0..r {
        for j in 0..c {
            b.extend_from_slice(&t.sample(i, j).to_le_bytes());
        }
    }
    std::fs::write(&a[2], b).expect("written");
    println!("{r} {c}");
}
