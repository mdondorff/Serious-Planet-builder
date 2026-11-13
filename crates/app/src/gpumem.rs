//! What the GPU allocator holds (VIEW-070): the memory the device allocated and reserved, and the largest allocations by name, at the start and at the end of a window run, so that
//! a growth of the video memory over a flight (a leak, a pool that keeps growing) is named. Informational: nothing here decides anything.

use planet_render::GpuContext;
use std::collections::BTreeMap;

/// The allocations shown on the line, largest first.
const SHOWN: usize = 6;

/// `(name, bytes)` of every live allocation of the report, summed by name (the part before the first digit run or `#`, so numbered pool blocks count as one).
pub fn by_name(allocations: &[(String, u64)]) -> Vec<(String, u64, usize)> {
    let mut sum: BTreeMap<String, (u64, usize)> = BTreeMap::new();
    for (name, size) in allocations {
        let key: String = name.chars().take_while(|c| !c.is_ascii_digit() && *c != '#').collect::<String>().trim().to_string();
        let key = if key.is_empty() { "(unnamed)".to_string() } else { key };
        let e = sum.entry(key).or_default();
        e.0 += size;
        e.1 += 1;
    }
    let mut v: Vec<(String, u64, usize)> = sum.into_iter().map(|(k, (b, n))| (k, b, n)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

/// The line: allocated and reserved memory with the number of allocations and blocks, then the largest names.
pub fn format_report(label: &str, allocated: u64, reserved: u64, blocks: usize, allocations: &[(String, u64)]) -> String {
    let names = by_name(allocations);
    let top: Vec<String> = names.iter().take(SHOWN).map(|(n, b, c)| format!("{n} x{c} {:.0} MiB", mib(*b))).collect();
    format!(
        "planet editor: gpu memory {label}: allocated {:.0} MiB in {} allocations, reserved {:.0} MiB in {blocks} blocks; largest: {}",
        mib(allocated),
        allocations.len(),
        mib(reserved),
        top.join(", ")
    )
}

/// The allocator report of the device as a line, or the reason there is none.
pub fn line(ctx: &GpuContext, label: &str) -> String {
    match ctx.allocator_report() {
        Some(r) => format_report(label, r.allocated_bytes, r.reserved_bytes, r.blocks, &r.allocations),
        None => format!("planet editor: gpu memory {label}: the backend reports no allocator"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: VIEW-070
    #[test]
    fn the_allocations_are_summed_by_name_and_the_line_names_the_largest() {
        let a = |n: &str, s: u64| (n.to_string(), s);
        let allocations =
            vec![a("terrain pool block 0", 134_217_728), a("terrain pool block 1", 134_217_728), a("lit target", 8_000_000), a("", 100)];
        let names = by_name(&allocations);
        assert_eq!(names[0], ("terrain pool block".to_string(), 268_435_456, 2));
        assert_eq!(names.last().unwrap().0, "(unnamed)");
        let line = format_report("at the end", 276_435_556, 402_653_184, 3, &allocations);
        assert_eq!(
            line,
            "planet editor: gpu memory at the end: allocated 264 MiB in 4 allocations, reserved 384 MiB in 3 blocks; largest: terrain pool block x2 256 MiB, lit target x1 8 MiB, (unnamed) x1 0 MiB"
        );
    }
}
