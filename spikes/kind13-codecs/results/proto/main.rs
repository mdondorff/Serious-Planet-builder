use std::io::{Read, Write};
use std::time::Instant;

const N: usize = 3072;

fn counts(v: &[f32], q: f32) -> Vec<i16> {
    v.iter().map(|&x| (x / q).round().clamp(-32768.0, 32767.0) as i16).collect()
}
fn bytes(c: &[i16]) -> Vec<u8> {
    c.iter().flat_map(|x| x.to_le_bytes()).collect()
}
fn row_delta(c: &[i16]) -> Vec<i16> {
    let mut o = Vec::with_capacity(c.len());
    for row in c.chunks(N) {
        o.push(row[0]);
        for i in 1..N {
            o.push(row[i].wrapping_sub(row[i - 1]));
        }
    }
    o
}

struct Codec {
    name: &'static str,
    heavy: bool,
    enc: Box<dyn Fn(&[u8]) -> Vec<u8>>,
    dec: Box<dyn Fn(&[u8], usize) -> Vec<u8>>,
}

fn brotli_codec(name: &'static str, q: u32, heavy: bool) -> Codec {
    Codec {
        name,
        heavy,
        enc: Box::new(move |b| {
            let mut o = Vec::new();
            {
                let mut w = brotli::CompressorWriter::new(&mut o, 65536, q, 22);
                w.write_all(b).unwrap();
            }
            o
        }),
        dec: Box::new(|c, n| {
            let mut o = Vec::with_capacity(n);
            brotli::Decompressor::new(c, 65536).read_to_end(&mut o).unwrap();
            o
        }),
    }
}
fn ruzstd_codec(name: &'static str, l: ruzstd::encoding::CompressionLevel, heavy: bool) -> Codec {
    Codec {
        name,
        heavy,
        enc: Box::new(move |b| ruzstd::encoding::compress_to_vec(b, l)),
        dec: Box::new(|c, n| {
            let mut o = Vec::with_capacity(n);
            ruzstd::decoding::StreamingDecoder::new(c).unwrap().read_to_end(&mut o).unwrap();
            o
        }),
    }
}
fn deflate_codec(name: &'static str, l: u8) -> Codec {
    Codec {
        name,
        heavy: true,
        enc: Box::new(move |b| miniz_oxide::deflate::compress_to_vec(b, l)),
        dec: Box::new(|c, _| miniz_oxide::inflate::decompress_to_vec(c).unwrap()),
    }
}

fn main() {
    use ruzstd::encoding::CompressionLevel as L;
    let dir = std::env::args().nth(1).unwrap();
    let ntime: usize = std::env::args().nth(2).map_or(3, |s| s.parse().unwrap());
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(Result::ok).map(|e| e.path()).collect();
    files.sort();
    let codecs: Vec<Codec> = vec![
        Codec {
            name: "lz4_flex 3 tiles",
            heavy: false,
            enc: Box::new(|b| lz4_flex::compress(b)),
            dec: Box::new(|c, n| lz4_flex::decompress(c, n).unwrap()),
        },
        ruzstd_codec("ruzstd fastest", L::Fastest, false),
        brotli_codec("brotli 5", 5, false),
        brotli_codec("brotli 9", 9, true),
        brotli_codec("brotli 11", 11, true),
        deflate_codec("deflate 6", 6),
        deflate_codec("deflate 9", 9),
    ];
    let tiles: Vec<(u64, Vec<f32>)> = files
        .iter()
        .map(|f| {
            let stem = f.file_stem().unwrap().to_string_lossy().to_string();
            let land = stem.rsplit('-').next().unwrap().parse::<u64>().unwrap();
            let raw = std::fs::read(f).unwrap();
            (land, raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
        })
        .collect();
    let land_all: u64 = tiles.iter().map(|t| t.0).sum();
    let land_t: u64 = tiles[..ntime].iter().map(|t| t.0).sum();
    println!("tiles {} (timing/heavy subset: first {ntime}); load: see notes", tiles.len());
    for q in [0.25f32, 1.0] {
        for delta in [false, true] {
            let inputs: Vec<Vec<u8>> = tiles.iter().map(|(_, v)| { let c = counts(v, q); bytes(&if delta { row_delta(&c) } else { c }) }).collect();
            println!("== {q} m, {} ==", if delta { "row-delta" } else { "plain" });
            let mut base_enc = 0.0;
            let mut base_dec = 0.0;
            for c in &codecs {
                let n = if c.heavy || c.name.contains("3 tiles") { ntime } else { inputs.len() };
                let land = if c.heavy || c.name.contains("3 tiles") { land_t } else { land_all };
                let mut total = 0usize;
                let (mut enc_t, mut dec_min, mut dec_med, mut bytes_t) = (f64::MAX, 0.0f64, 0.0f64, 0usize);
                let (mut e_sum, mut d_min_sum, mut d_med_sum, mut b_sum) = (0.0, 0.0, 0.0, 0usize);
                for (k, inp) in inputs[..n].iter().enumerate() {
                    let t = Instant::now();
                    let out = (c.enc)(inp);
                    let first = t.elapsed().as_secs_f64();
                    total += out.len();
                    if k < ntime {
                        let reps = if first < 3.0 { 5 } else { 1 };
                        enc_t = first;
                        for _ in 1..reps {
                            let t = Instant::now();
                            let _ = (c.enc)(inp);
                            enc_t = enc_t.min(t.elapsed().as_secs_f64());
                        }
                        let mut d: Vec<f64> = (0..5).map(|_| { let t = Instant::now(); let r = (c.dec)(&out, inp.len()); let e = t.elapsed().as_secs_f64(); assert!(r == *inp, "{} roundtrip", c.name); e }).collect();
                        d.sort_by(f64::total_cmp);
                        dec_min = d[0];
                        dec_med = d[2];
                        e_sum += enc_t;
                        d_min_sum += dec_min;
                        d_med_sum += dec_med;
                        b_sum += inp.len();
                        bytes_t += out.len();
                    }
                }
                let mb = b_sum as f64 / 1e6;
                let (enc_mbs, dec_mbs, dec_med_mbs) = (mb / e_sum, mb / d_min_sum, mb / d_med_sum);
                if c.name == "lz4_flex" { base_enc = enc_mbs; base_dec = dec_mbs; }
                println!(
                    "{:15} {} tiles  {:.3} B/land cell  {:6.2} MB/tile  planet {:5.1} GB  | ratio {:.3} | enc {:7.1} MB/s ({:.3}x lz4) | dec {:7.1} MB/s min ({:.3}x lz4), {:7.1} median | dec {:.0} ms/tile min | encode {:.1} s/tile",
                    c.name, n, total as f64 / land as f64, total as f64 / n as f64 / 1e6, total as f64 / n as f64 * 2742.0 / 1e9,
                    b_sum as f64 / bytes_t as f64, enc_mbs, enc_mbs / base_enc, dec_mbs, dec_mbs / base_dec, dec_med_mbs, d_min_sum / ntime as f64 * 1e3, e_sum / ntime as f64
                );
            }
        }
    }
}
