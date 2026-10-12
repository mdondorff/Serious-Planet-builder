//! Compaction and the size cap (STRM-013, ADR 0019 "space reclamation", ADR 0024 points 3 and 8).
//!
//! `compact` rewrites the live records of a store directory into fresh packs in index order and removes the packs it read.
//! The output is a pure function of the set of records kept (never of which pack held them, of the order they were first
//! written, or of timing), so its bytes are reproducible and hashable (CON-14); the set kept is a pure function of the
//! stored records, the liveness predicate, the ranking and the cap. Nothing is edited in place: new packs are written, then
//! the old ones are removed, and a crash in between leaves duplicates that the newest-pack-wins rule resolves.

use std::collections::BTreeMap;

use super::*;

/// What `compact` keeps.
pub struct CompactPolicy<'a> {
    /// Whether a stored record is still producible and current: its kind's generator version, its hash version and its
    /// definition node set id equal the current ones (ADR 0019). A record the predicate refuses is dropped.
    pub live: &'a dyn Fn(&Record) -> bool,
    /// Bytes of kept records (headers and payloads; pack headers and index files come on top). When the live set is larger,
    /// the records of the lowest rank are evicted first (ties: the later index order) until it fits. `None` keeps all.
    pub cap_bytes: Option<u64>,
    /// Eviction priority of a live record given the sequence number of the pack that held its winning copy: the highest
    /// rank is kept longest. The default ([`rank_by_pack_age`]) keeps the newest packs; a last-hit sidecar can replace it.
    pub rank: &'a dyn Fn(&Record, u32) -> u64,
    /// The size limit of the packs written (ADR 0024 point 9).
    pub max_pack_bytes: u64,
}

/// The default eviction rank: the sequence number of the pack, so the records written longest ago go first.
pub fn rank_by_pack_age(_: &Record, pack_seq: u32) -> u64 {
    u64::from(pack_seq)
}

/// What a compaction did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompactReport {
    /// Records written to the new packs.
    pub kept: usize,
    /// Live-set records dropped because the predicate refused them.
    pub dead: usize,
    /// Older copies of a key held again by a newer pack or later in the same pack.
    pub superseded: usize,
    /// Records dropped because their payload fails its CRC (and the older copies a corrupt newest copy hides, ADR 0024 point 8).
    pub corrupt: usize,
    /// Live records dropped to fit the cap.
    pub evicted: usize,
    /// Packs read and removed, and the packs of another hash version removed with them.
    pub packs_in: usize,
    pub dead_packs: usize,
    /// Packs written.
    pub packs_out: usize,
    pub bytes_before: u64,
    pub bytes_after: u64,
    /// BLAKE3-128 of the output packs' content, in order (not of their file names).
    pub output_hash: [u8; 16],
    /// Packs left alone and damage found, for a person to read.
    pub problems: Vec<String>,
}

struct Kept {
    sort: SortKey,
    key: CacheKey,
    pack: usize,
    entry: IndexEntry,
    rank: u64,
    bytes: u64,
}

/// Compact the store in `dir` under its writer lock (a held lock is [`PackError::Locked`]).
pub fn compact(dir: &Path, params: StoreParams, policy: &CompactPolicy<'_>) -> Result<CompactReport, PackError> {
    let (lock, holder) = take_dir_lock(dir)?;
    let result = compact_locked(dir, params, policy);
    let _ = fs::remove_file(&holder);
    let _ = lock.unlock();
    result
}

fn compact_locked(dir: &Path, params: StoreParams, policy: &CompactPolicy<'_>) -> Result<CompactReport, PackError> {
    let mut report = CompactReport::default();
    let seqs = pack_sequences(dir)?;
    let mut ours: Vec<(u32, PackFile)> = Vec::new();
    let mut remove: Vec<u32> = Vec::new();
    for &seq in &seqs {
        let (data_path, index_path) = pack_paths(dir, seq);
        let mut problems = Vec::new();
        match PackFile::open(dir, seq, params, &mut problems)? {
            Some(p) => {
                report.bytes_before += io(fs::metadata(&data_path), "reading the size of", &data_path)?.len();
                ours.push((seq, p));
            }
            None => {
                let mut f = io(File::open(&data_path), "opening", &data_path)?;
                let dead = match read_pack_header(&mut f, &data_path)? {
                    Some(h) => matches!(check_pack_header(&data_path, &h, params), Ok(HeaderCheck::OtherHashVersion)),
                    None => false,
                };
                if dead {
                    report.bytes_before += io(fs::metadata(&data_path), "reading the size of", &data_path)?.len();
                    remove.push(seq);
                    report.dead_packs += 1;
                } else {
                    report.problems.push(format!("{}: left alone (not a usable pack of this store)", data_path.display()));
                }
            }
        }
        let _ = index_path;
        report.problems.extend(problems);
    }
    report.packs_in = ours.len();

    // The winning copy of every key: the newest pack, the later offset within a pack (the index already holds one entry per
    // key per pack).
    let mut winners: BTreeMap<(SortKey, [u8; 16]), (usize, IndexEntry)> = BTreeMap::new();
    let mut total_entries = 0usize;
    for (i, (_, p)) in ours.iter().enumerate() {
        for e in &p.entries {
            total_entries += 1;
            winners.insert((e.sort, e.key.0), (i, *e));
        }
    }
    // A corrupt newest copy is a miss even where an older pack holds an intact one (ADR 0024 point 8): a key with a damaged
    // copy in a newer pack than its winner is dropped.
    let mut damaged_in: BTreeMap<[u8; 16], usize> = BTreeMap::new();
    for (i, (_, p)) in ours.iter().enumerate() {
        for d in &p.damage {
            if let Some(k) = d.key {
                let slot = damaged_in.entry(k.0).or_insert(i);
                *slot = (*slot).max(i);
            }
        }
    }
    let before = winners.len();
    winners.retain(|(_, key), (pack, _)| damaged_in.get(key).is_none_or(|&d| d <= *pack));
    report.corrupt += before - winners.len();
    report.superseded = total_entries - before;

    let mut kept: Vec<Kept> = Vec::new();
    for ((sort, key), (pack, entry)) in winners {
        let record = match ours[pack].1.read(&entry) {
            Ok(r) => r,
            Err(PackError::Corrupt { .. }) => {
                report.corrupt += 1;
                continue;
            }
            Err(e) => return Err(e),
        };
        if !(policy.live)(&record) {
            report.dead += 1;
            continue;
        }
        let rank = (policy.rank)(&record, ours[pack].0);
        kept.push(Kept { sort, key: CacheKey(key), pack, entry, rank, bytes: RECORD_HEADER_LEN as u64 + u64::from(entry.length) });
    }

    if let Some(cap) = policy.cap_bytes {
        if kept.iter().map(|k| k.bytes).sum::<u64>() > cap {
            // Strict priority: the highest rank first, ties to the earlier index order; stop at the first record that does
            // not fit, so what is kept never depends on a smaller record further down.
            let mut order: Vec<usize> = (0..kept.len()).collect();
            order.sort_by(|&a, &b| kept[b].rank.cmp(&kept[a].rank).then(a.cmp(&b)));
            let mut used = 0u64;
            let mut keep = vec![false; kept.len()];
            for i in order {
                if used + kept[i].bytes > cap {
                    break;
                }
                used += kept[i].bytes;
                keep[i] = true;
            }
            report.evicted = keep.iter().filter(|k| !**k).count();
            let mut i = 0;
            kept.retain(|_| {
                i += 1;
                keep[i - 1]
            });
        }
    }

    // Write the kept records in index order into new packs after the highest sequence number seen.
    let mut hasher = blake3::Hasher::new();
    let mut next = seqs.last().copied().unwrap_or(0);
    let mut out: Option<Current> = None;
    let finish = |c: Current, hasher: &mut blake3::Hasher, report: &mut CompactReport| -> Result<(), PackError> {
        let Current { data_path, index_path, mut file, len, entries, .. } = c;
        io(file.flush(), "flushing", &data_path)?;
        let tail = tail_crc(&mut file, len, &data_path)?
            .ok_or_else(|| PackError::Io(format!("{} is shorter than the {len} bytes written to it", data_path.display())))?;
        write_index_atomic(&index_path, &normalise(entries), len, tail)?;
        hasher.update(b"end");
        hasher.update(&len.to_le_bytes());
        report.packs_out += 1;
        report.bytes_after += len;
        Ok(())
    };
    for k in &kept {
        let record = ours[k.pack].1.read(&k.entry)?;
        let bytes = encode_record(&record)?;
        if let Some(c) = &out {
            if c.len > PACK_HEADER_LEN && c.len + bytes.len() as u64 > policy.max_pack_bytes {
                finish(out.take().expect("checked"), &mut hasher, &mut report)?;
            }
        }
        if out.is_none() {
            next = next.checked_add(1).ok_or_else(|| PackError::NoRoom { dir: dir.display().to_string() })?;
            let (data_path, index_path) = pack_paths(dir, next);
            let mut file =
                io(OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&data_path), "creating", &data_path)?;
            let header = pack_header(params);
            io(file.write_all(&header), "writing", &data_path)?;
            hasher.update(b"pack");
            hasher.update(&header);
            out = Some(Current { seq: next, data_path, index_path, file, len: PACK_HEADER_LEN, entries: Vec::new(), index_current: false });
        }
        let c = out.as_mut().expect("opened");
        io(c.file.write_all(&bytes), "writing", &c.data_path)?;
        hasher.update(&bytes);
        c.entries.push(IndexEntry { sort: k.sort, key: k.key, offset: c.len, length: record.payload.len() as u32 });
        c.len += bytes.len() as u64;
        report.kept += 1;
    }
    if let Some(c) = out.take() {
        finish(c, &mut hasher, &mut report)?;
    }

    // Only now the packs that were read go away (data first: a pack without data is not listed).
    remove.extend(ours.iter().map(|(seq, _)| *seq));
    drop(ours);
    for seq in remove {
        let (data_path, index_path) = pack_paths(dir, seq);
        io(fs::remove_file(&data_path), "removing", &data_path)?;
        match fs::remove_file(&index_path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(PackError::Io(format!("removing {}: {e}", index_path.display()))),
        }
    }
    report.output_hash = hasher.finalize().as_bytes()[..16].try_into().expect("16 bytes");
    Ok(report)
}
