//! `cargo xtask agent-guard` (BUILD-062): Antigravity `PreToolUse` safety guardrail in pure Rust (`CON-25`: zero scripts in repo).
//! Reads hook JSON from stdin and emits `{"decision":"allow"|"deny"|"force_ask","reason":...}` on stdout.

use crate::heavy;
use crate::util::Res;
use std::io::Read;
use std::path::Path;
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionKind {
    Allow,
    Deny,
    ForceAsk,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub kind: DecisionKind,
    pub reason: Option<String>,
}

impl Decision {
    pub fn allow() -> Self {
        Self { kind: DecisionKind::Allow, reason: None }
    }
    pub fn deny(reason: impl Into<String>) -> Self {
        Self { kind: DecisionKind::Deny, reason: Some(reason.into()) }
    }
    pub fn force_ask(reason: impl Into<String>) -> Self {
        Self { kind: DecisionKind::ForceAsk, reason: Some(reason.into()) }
    }

    pub fn to_json(&self) -> String {
        let k = match self.kind {
            DecisionKind::Allow => "allow",
            DecisionKind::Deny => "deny",
            DecisionKind::ForceAsk => "force_ask",
        };
        match &self.reason {
            None => format!(r#"{{"decision":"{k}"}}"#),
            Some(r) => format!(r#"{{"decision":"{k}","reason":"{}"}}"#, json_escape(r)),
        }
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str(r#"\""#),
            '\\' => out.push_str(r#"\\"#),
            '\n' => out.push_str(r#"\n"#),
            '\r' => out.push_str(r#"\r"#),
            '\t' => out.push_str(r#"\t"#),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Machine lock and quiet state inspected before running cargo commands (`CON-04`: injectable for tests).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuardContext {
    pub quiet_age_min: Option<u64>,
    pub exclusive_holder: Option<String>,
}

impl GuardContext {
    pub fn from_system() -> Self {
        let mut quiet_age_min: Option<u64> = None;
        let mut exclusive_holder: Option<String> = None;
        let mut dirs = Vec::new();
        if let Some(d) = heavy::state_dir() {
            dirs.push(d);
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty()) {
            let p = std::path::PathBuf::from(local).join("planet");
            if !dirs.contains(&p) {
                dirs.push(p);
            }
        }
        for d in dirs {
            if quiet_age_min.is_none() {
                if let Some(age) = heavy::quiet_age(&d) {
                    let mins = age.as_secs() / 60;
                    if mins < 60 {
                        quiet_age_min = Some(mins);
                    }
                }
            }
            if exclusive_holder.is_none() {
                for note in heavy::holder_notes(&d) {
                    if note.contains("exclusive)") || note.contains(", exclusive") {
                        exclusive_holder = Some(note);
                        break;
                    }
                }
            }
        }
        Self { quiet_age_min, exclusive_holder }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HookInput {
    pub tool_name: String,
    pub path: Option<String>,
    pub command_line: Option<String>,
}

/// Minimal pure-std JSON parser extracting `toolCall.name`, `toolCall.args.Path`, and `toolCall.args.CommandLine`.
pub fn parse_hook_input(raw: &str) -> Result<HookInput, String> {
    let mut p = JsonScanner::new(raw);
    p.skip_ws();
    let mut out = HookInput::default();
    p.expect_char(b'{')?;
    p.parse_object(|key, scanner| match key {
        "toolCall" => {
            scanner.skip_ws();
            scanner.expect_char(b'{')?;
            scanner.parse_object(|tc_key, sc| match tc_key {
                "name" => {
                    out.tool_name = sc.parse_string()?;
                    Ok(())
                }
                "args" => {
                    sc.skip_ws();
                    if sc.peek() == Some(b'n') {
                        sc.skip_value()
                    } else {
                        sc.expect_char(b'{')?;
                        sc.parse_object(|arg_key, asc| match arg_key {
                            "Path" | "path" | "TargetFile" | "target_file" | "FilePath" | "file_path" => {
                                asc.skip_ws();
                                if asc.peek() == Some(b'"') {
                                    out.path = Some(asc.parse_string()?);
                                    Ok(())
                                } else {
                                    asc.skip_value()
                                }
                            }
                            "CommandLine" | "commandLine" | "command_line" | "command" => {
                                asc.skip_ws();
                                if asc.peek() == Some(b'"') {
                                    out.command_line = Some(asc.parse_string()?);
                                    Ok(())
                                } else {
                                    asc.skip_value()
                                }
                            }
                            _ => asc.skip_value(),
                        })
                    }
                }
                _ => sc.skip_value(),
            })
        }
        _ => scanner.skip_value(),
    })?;
    p.skip_ws();
    Ok(out)
}

struct JsonScanner<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> JsonScanner<'a> {
    fn new(s: &'a str) -> Self {
        Self { bytes: s.as_bytes(), pos: 0 }
    }
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.pos += 1;
        }
    }
    fn expect_char(&mut self, expected: u8) -> Result<(), String> {
        self.skip_ws();
        match self.peek() {
            Some(c) if c == expected => {
                self.pos += 1;
                Ok(())
            }
            other => Err(format!("expected '{}' at byte {}, found {:?}", expected as char, self.pos, other.map(|b| b as char))),
        }
    }
    fn parse_string(&mut self) -> Result<String, String> {
        self.skip_ws();
        self.expect_char(b'"')?;
        let mut out = String::new();
        while let Some(b) = self.peek() {
            self.pos += 1;
            if b == b'"' {
                return Ok(out);
            }
            if b == b'\\' {
                let esc = self.peek().ok_or_else(|| "unterminated JSON escape".to_string())?;
                self.pos += 1;
                match esc {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\x08'),
                    b'f' => out.push('\x0c'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let hex = self.bytes.get(self.pos..self.pos + 4).ok_or_else(|| "short \\u escape".to_string())?;
                        let s = std::str::from_utf8(hex).map_err(|_| "invalid \\u escape".to_string())?;
                        let cp = u32::from_str_radix(s, 16).map_err(|_| "invalid hex in \\u escape".to_string())?;
                        self.pos += 4;
                        if (0xD800..=0xDBFF).contains(&cp)
                            && self.bytes.get(self.pos..self.pos + 2) == Some(b"\\u")
                            && self.bytes.len() >= self.pos + 6
                        {
                            let lo_s = std::str::from_utf8(&self.bytes[self.pos + 2..self.pos + 6]).unwrap_or("");
                            if let Ok(lo) = u32::from_str_radix(lo_s, 16) {
                                if (0xDC00..=0xDFFF).contains(&lo) {
                                    self.pos += 6;
                                    let combined = 0x10000 + (((cp - 0xD800) << 10) | (lo - 0xDC00));
                                    if let Some(ch) = char::from_u32(combined) {
                                        out.push(ch);
                                        continue;
                                    }
                                }
                            }
                        }
                        if let Some(ch) = char::from_u32(cp) {
                            out.push(ch);
                        }
                    }
                    other => return Err(format!("unknown escape \\{}", other as char)),
                }
            } else if b < 0x80 {
                out.push(b as char);
            } else {
                // Multi-byte UTF-8 sequence starting at self.pos - 1
                let start = self.pos - 1;
                let rest = &self.bytes[start..];
                let s = std::str::from_utf8(rest).unwrap_or_else(|e| std::str::from_utf8(&rest[..e.valid_up_to()]).unwrap_or(""));
                let Some(ch) = s.chars().next() else {
                    return Err("invalid UTF-8 inside JSON string".into());
                };
                out.push(ch);
                self.pos = start + ch.len_utf8();
            }
        }
        Err("unterminated JSON string".into())
    }
    fn parse_object<F>(&mut self, mut on_field: F) -> Result<(), String>
    where
        F: FnMut(&str, &mut JsonScanner<'_>) -> Result<(), String>,
    {
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(());
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.expect_char(b':')?;
            on_field(&key, self)?;
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(());
                }
                other => return Err(format!("expected ',' or '}}' at byte {}, found {:?}", self.pos, other.map(|b| b as char))),
            }
        }
    }
    fn skip_value(&mut self) -> Result<(), String> {
        self.skip_ws();
        match self.peek() {
            Some(b'"') => {
                let _ = self.parse_string()?;
                Ok(())
            }
            Some(b'{') => {
                self.pos += 1;
                self.parse_object(|_, sc| sc.skip_value())
            }
            Some(b'[') => {
                self.pos += 1;
                self.skip_ws();
                if self.peek() == Some(b']') {
                    self.pos += 1;
                    return Ok(());
                }
                loop {
                    self.skip_value()?;
                    self.skip_ws();
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b']') => {
                            self.pos += 1;
                            return Ok(());
                        }
                        other => return Err(format!("expected ',' or ']' at byte {}, found {:?}", self.pos, other.map(|b| b as char))),
                    }
                }
            }
            Some(b't') if self.bytes.get(self.pos..self.pos + 4) == Some(b"true") => {
                self.pos += 4;
                Ok(())
            }
            Some(b'f') if self.bytes.get(self.pos..self.pos + 5) == Some(b"false") => {
                self.pos += 5;
                Ok(())
            }
            Some(b'n') if self.bytes.get(self.pos..self.pos + 4) == Some(b"null") => {
                self.pos += 4;
                Ok(())
            }
            Some(b) if b == b'-' || b.is_ascii_digit() => {
                while matches!(self.peek(), Some(b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E')) {
                    self.pos += 1;
                }
                Ok(())
            }
            other => Err(format!("unexpected JSON value at byte {}: {:?}", self.pos, other.map(|b| b as char))),
        }
    }
}

fn norm_path(raw: &str) -> String {
    let mut s = raw.trim().replace('\\', "/");
    if let Some(rest) = s.strip_prefix("//?/") {
        s = rest.to_string();
    }
    while s.ends_with('/') && s.len() > 3 {
        s.pop();
    }
    s.to_ascii_lowercase()
}

fn under_dir(norm_path: &str, root: &str) -> bool {
    norm_path == root || norm_path.starts_with(&format!("{root}/"))
}

fn is_allowed_data_write(norm_path: &str) -> bool {
    const DATA: &str = "c:/dev/seriousplanetdata";
    if under_dir(norm_path, &format!("{DATA}/tmp/heavy")) || under_dir(norm_path, &format!("{DATA}/tmp/ag2")) {
        return true;
    }
    if let Some(file) = norm_path.strip_prefix(&format!("{DATA}/lanes/")) {
        return !file.contains('/') && !file.contains("..") && file.starts_with("ag2-") && file.ends_with(".md");
    }
    false
}

fn is_untouchable_repo_file(norm_path: &str) -> bool {
    ["/docs/constitution.md", "/docs/status.md"].iter().any(|suffix| norm_path.ends_with(suffix))
        || matches!(norm_path, "docs/constitution.md" | "docs/status.md")
        || mentions_sprint_file(norm_path)
}

/// Whether the text names a sprint or milestone file `docs/milestones/<M>.md` (the active sprint file and its siblings, the lead's files;
/// ADR 0130). A record under `docs/milestones/<M>/` is not one.
fn mentions_sprint_file(norm: &str) -> bool {
    const DIR: &str = "docs/milestones/";
    let mut pos = 0;
    while let Some(rel) = norm[pos..].find(DIR) {
        let start = pos + rel + DIR.len();
        let rest = &norm[start..];
        let len = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')).unwrap_or(rest.len());
        let name = &rest[..len];
        if name.len() > 3 && name.ends_with(".md") && !rest[len..].starts_with('/') {
            return true;
        }
        pos = start;
    }
    false
}

fn is_script_in_repo(norm_path: &str) -> bool {
    let in_owned = under_dir(norm_path, "c:/dev/ag2_serious-planet-builder") || !norm_path.contains(':');
    if !in_owned {
        return false;
    }
    let ignored = ["/target/", "/.git/", "/node_modules/"].iter().any(|seg| norm_path.contains(seg));
    !ignored && [".ps1", ".sh", ".bat", ".cmd"].iter().any(|ext| norm_path.ends_with(ext))
}

/// Evaluate a file write/edit path against companion boundaries.
fn evaluate_write_path(raw: &str) -> Decision {
    let p = norm_path(raw);
    if under_dir(&p, "c:/dev/serious-planet-builder") {
        return Decision::deny(format!("Guardrail: '{raw}' is inside read-only c:/dev/serious-planet-builder."));
    }
    if under_dir(&p, "c:/dev/ag_serious-planet-builder") {
        return Decision::deny(format!("Guardrail: '{raw}' is inside read-only c:/dev/ag_serious-planet-builder."));
    }
    if under_dir(&p, "c:/dev/seriousplanetdata") && !is_allowed_data_write(&p) {
        return Decision::deny(format!(
            "Guardrail: '{raw}' is read-only under SeriousPlanetData (writable zones: tmp/heavy/**, tmp/ag2/**, lanes/ag2-*.md)."
        ));
    }
    if is_untouchable_repo_file(&p) {
        return Decision::deny(format!("Guardrail: '{raw}' is owner/lead-controlled and must not be edited by ag2."));
    }
    if is_script_in_repo(&p) {
        return Decision::deny(format!("Guardrail: '{raw}' is a script file forbidden inside the repository (CON-25 / BUILD-002)."));
    }
    Decision::allow()
}

fn is_left_boundary(b: Option<u8>) -> bool {
    matches!(b, None | Some(b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'\\' | b'\'' | b'"' | b'=' | b'(' | b';'))
}

fn is_right_boundary(b: Option<u8>) -> bool {
    matches!(b, None | Some(b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'\\' | b'\'' | b'"' | b',' | b';' | b')'))
}

/// Collect occurrences of a root directory token (matching path boundaries so `ag2_serious-planet-builder` does NOT match `serious-planet-builder`).
fn find_root_occurrences(norm_cmd: &str, token: &str) -> Vec<usize> {
    let bytes = norm_cmd.as_bytes();
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(rel) = norm_cmd[pos..].find(token) {
        let at = pos + rel;
        let before = at.checked_sub(1).and_then(|i| bytes.get(i).copied());
        let after = bytes.get(at + token.len()).copied();
        if is_left_boundary(before) && is_right_boundary(after) {
            out.push(at);
        }
        pos = at + token.len();
    }
    out
}

fn has_word(haystack: &str, word: &str) -> bool {
    let b = haystack.as_bytes();
    let mut pos = 0;
    while let Some(rel) = haystack[pos..].find(word) {
        let at = pos + rel;
        let before = at.checked_sub(1).and_then(|i| b.get(i).copied());
        let after = b.get(at + word.len()).copied();
        let left_ok = !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-');
        let right_ok = !after.is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-');
        if left_ok && right_ok {
            return true;
        }
        pos = at + word.len();
    }
    false
}

fn is_mutating_shell_cmd(norm_cmd: &str) -> bool {
    let verbs = [
        "remove-item",
        "set-content",
        "add-content",
        "out-file",
        "clear-content",
        "new-item",
        "rename-item",
        "move-item",
        "copy-item",
        "rm",
        "del",
        "rmdir",
        "mv",
        "cp",
        "mkdir",
    ];
    if verbs.iter().any(|v| has_word(norm_cmd, v)) {
        return true;
    }
    if has_word(norm_cmd, "git")
        && [
            "commit",
            "push",
            "reset",
            "clean",
            "checkout",
            "switch",
            "merge",
            "rebase",
            "cherry-pick",
            "stash",
            "worktree",
            "fetch",
            "pull",
            "tag",
            "init",
        ]
        .iter()
        .any(|sub| has_word(norm_cmd, sub))
    {
        return true;
    }
    // Shell redirections `>` / `>>` (not PowerShell operators `-gt`, `->`, `=>`).
    let bytes = norm_cmd.as_bytes();
    for (i, &c) in bytes.iter().enumerate() {
        if c == b'>' {
            let prev = i.checked_sub(1).and_then(|k| bytes.get(k).copied());
            if !matches!(prev, Some(b'-' | b'=')) {
                return true;
            }
        }
    }
    false
}

fn data_paths_all_writable(norm_cmd: &str, occs: &[usize]) -> bool {
    occs.iter().all(|&at| {
        let rest = &norm_cmd[at + "seriousplanetdata".len()..];
        let end = rest.find(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | ';' | ',' | ')')).unwrap_or(rest.len());
        let suffix = &rest[..end];
        let full = format!("c:/dev/seriousplanetdata{suffix}");
        is_allowed_data_write(&norm_path(&full))
    })
}

/// Split a command line into segments on unquoted command separators (`;`, `\n`, `\r`, `|`, `&`, `(`, `)`, `` ` ``).
/// Keeps `'...'` and `"..."` literals intact (honoring `\"` and `\\` inside double quotes on Windows paths).
fn split_shell_segments(cmd: &str) -> Vec<&str> {
    let bytes = cmd.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_single {
            if b == b'\'' {
                in_single = false;
            }
            i += 1;
            continue;
        }
        if in_double {
            if b == b'\\' && matches!(bytes.get(i + 1), Some(b'"' | b'\\')) {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_double = false;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' => {
                in_single = true;
                i += 1;
            }
            b'"' => {
                in_double = true;
                i += 1;
            }
            b';' | b'\n' | b'\r' | b'|' | b'&' | b'(' | b')' | b'`' => {
                let seg = cmd[start..i].trim();
                if !seg.is_empty() {
                    out.push(seg);
                }
                i += 1;
                start = i;
            }
            _ => i += 1,
        }
    }
    let tail = cmd[start..].trim();
    if !tail.is_empty() {
        out.push(tail);
    }
    out
}

/// Tokenize one shell segment into unquoted arguments, collapsing surrounding quotes within each argument.
fn shell_tokens(seg: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut has_token = false;
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = seg.chars().peekable();
    while let Some(c) = chars.next() {
        if in_single {
            if c == '\'' {
                in_single = false;
            } else {
                cur.push(c);
            }
            continue;
        }
        if in_double {
            if c == '\\' && matches!(chars.peek(), Some('"' | '\\')) {
                if let Some(escaped) = chars.next() {
                    cur.push(escaped);
                }
            } else if c == '"' {
                in_double = false;
            } else {
                cur.push(c);
            }
            continue;
        }
        if c.is_ascii_whitespace() {
            if has_token {
                out.push(std::mem::take(&mut cur));
                has_token = false;
            }
            continue;
        }
        has_token = true;
        match c {
            '\'' => in_single = true,
            '"' => in_double = true,
            _ => cur.push(c),
        }
    }
    if has_token {
        out.push(cur);
    }
    out
}

fn normalized_bin_name(tok: &str) -> String {
    let slash = tok.replace('\\', "/");
    let base = slash.rsplit_once('/').map_or(slash.as_str(), |(_, r)| r);
    let low = base.to_ascii_lowercase();
    low.strip_suffix(".exe").unwrap_or(&low).to_string()
}

fn git_push_deny_reason(tokens: &[String]) -> Option<Decision> {
    let mut positionals: Vec<String> = Vec::new();
    let mut end_of_opts = false;
    let mut i = 0usize;
    while i < tokens.len() {
        let low = tokens[i].to_ascii_lowercase();
        if !end_of_opts && low == "--" {
            end_of_opts = true;
            i += 1;
            continue;
        }
        if !end_of_opts && low.starts_with('-') {
            let short_force = !low.starts_with("--") && !low.starts_with("-o") && low.as_bytes()[1..].contains(&b'f');
            if short_force
                || matches!(low.as_str(), "-f" | "--force" | "--force-with-lease" | "--force-if-includes" | "--mirror" | "--all" | "--tags")
                || low.starts_with("--force-with-lease=")
                || low.starts_with("--force-if-includes=")
            {
                return Some(Decision::deny("Guardrail: force-push, --all, --mirror, and --tags are forbidden (CLAUDE.md / GEMINI.md)."));
            }
            if matches!(low.as_str(), "-o" | "--push-option" | "--repo" | "--receive-pack" | "--exec" | "--upload-pack") {
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        positionals.push(low);
        i += 1;
    }

    if positionals.len() < 2 {
        return Some(Decision::deny(
            "Guardrail: bare 'git push' without an explicit feature-branch refspec is blocked; use 'git push -u origin change/<id>'.",
        ));
    }

    for refspec in &positionals[1..] {
        if refspec.starts_with('+') {
            return Some(Decision::deny("Guardrail: force-push refspec ('+...') is forbidden (CLAUDE.md / GEMINI.md)."));
        }
        let dst = refspec.rsplit_once(':').map_or(refspec.as_str(), |(_, d)| d);
        let branch = dst.strip_prefix("refs/heads/").or_else(|| dst.strip_prefix("refs/remotes/origin/")).unwrap_or(dst);
        if branch.is_empty() || matches!(branch, "main" | "master" | "head") {
            return Some(Decision::deny(
                "Guardrail: direct 'git push' to 'main' (or 'master') is forbidden; push to a feature branch (e.g. 'change/<id>') and merge via 'gh pr merge --squash'.",
            ));
        }
    }
    None
}

fn check_git_or_gh_segment(tokens: &[String]) -> Option<Decision> {
    let mut i = 0usize;
    while let Some(tok) = tokens.get(i) {
        let low = tok.to_ascii_lowercase();
        if low == "env" || low == "command" || (!low.starts_with('-') && low.contains('=')) {
            i += 1;
            continue;
        }
        break;
    }
    let bin_tok = tokens.get(i)?;
    let bin = normalized_bin_name(bin_tok);
    i += 1;

    if bin == "gh" {
        let mut subcmd: Option<String> = None;
        while let Some(tok) = tokens.get(i) {
            let low = tok.to_ascii_lowercase();
            if low.starts_with('-') {
                if matches!(low.as_str(), "-r" | "--repo" | "--hostname") {
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }
            subcmd = Some(low);
            i += 1;
            break;
        }
        if subcmd.as_deref() == Some("repo") {
            while let Some(tok) = tokens.get(i) {
                let low = tok.to_ascii_lowercase();
                if low.starts_with('-') {
                    i += 1;
                    continue;
                }
                if matches!(low.as_str(), "delete" | "archive") {
                    return Some(Decision::deny("Guardrail: repo deletion/archiving via gh is forbidden (CLAUDE.md / GEMINI.md)."));
                }
                break;
            }
        }
        return None;
    }

    if bin == "git" {
        let mut subcmd: Option<String> = None;
        while let Some(tok) = tokens.get(i) {
            let low = tok.to_ascii_lowercase();
            if low.starts_with('-') {
                if matches!(low.as_str(), "-c" | "--git-dir" | "--work-tree" | "--namespace" | "--super-prefix" | "--config-env") {
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }
            subcmd = Some(low);
            i += 1;
            break;
        }
        if subcmd.as_deref() == Some("push") {
            return git_push_deny_reason(&tokens[i..]);
        }
    }
    None
}

fn evaluate_command(raw_cmd: &str, ctx: &GuardContext) -> Decision {
    for seg in split_shell_segments(raw_cmd) {
        let tokens = shell_tokens(seg);
        if let Some(decision) = check_git_or_gh_segment(&tokens) {
            return decision;
        }
    }

    let norm = raw_cmd.replace('\\', "/").to_ascii_lowercase();
    let compact = norm.split_whitespace().collect::<Vec<_>>().join(" ");

    if compact.contains("cargo xtask perf") || (compact.contains("xtask") && has_word(&compact, "perf")) {
        return Decision::deny("Guardrail: cargo xtask perf on Rasierklinge requires a coordinated perf window (ADR 0020).");
    }
    if compact.contains("cargo xtask quiet off") {
        return Decision::deny("Guardrail: lowering the owner quiet flag (cargo xtask quiet off) is forbidden for companion agents.");
    }

    let mutating = is_mutating_shell_cmd(&norm);
    if mutating {
        if !find_root_occurrences(&norm, "serious-planet-builder").is_empty()
            || !find_root_occurrences(&norm, "ag_serious-planet-builder").is_empty()
        {
            return Decision::deny("Guardrail: mutating shell/git commands targeting read-only sibling workspaces are blocked.");
        }
        let data_occs = find_root_occurrences(&norm, "seriousplanetdata");
        if !data_occs.is_empty() && !data_paths_all_writable(&norm, &data_occs) {
            return Decision::deny(
                "Guardrail: mutating commands against SeriousPlanetData may target only tmp/heavy/**, tmp/ag2/**, or lanes/ag2-*.md.",
            );
        }
        if is_untouchable_repo_file(&norm)
            || has_word(&norm, "docs/constitution.md")
            || has_word(&norm, "docs/status.md")
            || mentions_sprint_file(&norm)
        {
            return Decision::deny(
                "Guardrail: mutating docs/constitution.md, docs/status.md, or a sprint file docs/milestones/<M>.md is blocked.",
            );
        }
    }

    if compact.contains("cargo xtask store") || compact.contains("cargo xtask accept") || compact.contains("--planet-bake") {
        return Decision::force_ask("Guardrail: multi-GB store or milestone acceptance run requires explicit owner confirmation.");
    }

    let is_cargo = has_word(&compact, "cargo");
    let is_guard_self = compact.contains("agent-guard");
    if is_cargo && !is_guard_self {
        if let Some(age) = ctx.quiet_age_min {
            return Decision::deny(format!("Guardrail: Rasierklinge quiet flag is active ({age} min old); defer cargo."));
        }
        if let Some(holder) = &ctx.exclusive_holder {
            return Decision::deny(format!("Guardrail: machine exclusive lock is active ({holder}); defer cargo."));
        }
    }
    Decision::allow()
}

pub fn evaluate(raw_stdin: &str, ctx: &GuardContext) -> Decision {
    if raw_stdin.trim().is_empty() {
        return Decision::allow();
    }
    let input = match parse_hook_input(raw_stdin) {
        Ok(v) => v,
        Err(e) => return Decision::force_ask(format!("Guardrail hook parse warning: {e}")),
    };
    match input.tool_name.as_str() {
        "write_file" | "edit_file" | "write_to_file" | "replace_file_content" | "multi_replace_file_content" | "code_action" => {
            match input.path.as_deref() {
                Some(p) => evaluate_write_path(p),
                None => Decision::allow(),
            }
        }
        "run_command" => match input.command_line.as_deref() {
            Some(cmd) => evaluate_command(cmd, ctx),
            None => Decision::allow(),
        },
        _ => Decision::allow(),
    }
}

fn guard_bin_path(root: &Path) -> std::path::PathBuf {
    root.join("target").join("tools").join(if cfg!(windows) { "xtask-guard.exe" } else { "xtask-guard" })
}

fn ensure_guard_bin(root: &Path) -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let dst = guard_bin_path(root);
    if exe != dst {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        let tmp = dst.with_extension(format!("tmp.{}", std::process::id()));
        std::fs::copy(&exe, &tmp).map_err(|e| format!("copy to {}: {e}", tmp.display()))?;
        if dst.exists() {
            let _ = std::fs::remove_file(&dst);
        }
        std::fs::rename(&tmp, &dst).map_err(|e| format!("rename to {}: {e}", dst.display()))?;
    }
    Ok(dst)
}

pub fn run(root: &Path, args: &[String]) -> Res {
    if args.iter().any(|a| a == "--refresh-bin") {
        let dst = ensure_guard_bin(root)?;
        println!("installed {}", dst.display());
        return Ok(());
    }
    if !guard_bin_path(root).exists() {
        let _ = ensure_guard_bin(root);
    }
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw).map_err(|e| format!("read stdin: {e}"))?;
    let ctx = GuardContext::from_system();
    let _ = SystemTime::now();
    println!("{}", evaluate(&raw, &ctx).to_json());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hook(tool: &str, key: &str, val: &str) -> String {
        format!(r#"{{"toolCall":{{"name":"{}","args":{{"{}":"{}"}}}},"stepIdx":1}}"#, tool, key, json_escape(val))
    }

    // spec: BUILD-062
    #[test]
    fn write_paths_enforce_readonly_siblings_zoned_data_untouchable_docs_and_no_scripts() {
        let ctx = GuardContext::default();
        assert_eq!(
            evaluate(&hook("write_file", "Path", r"c:\dev\ag2_serious-planet-builder\crates\editor\src\lib.rs"), &ctx),
            Decision::allow()
        );
        assert_eq!(evaluate(&hook("write_file", "Path", r"c:\dev\seriousplanetdata\tmp\ag2\cockpit.html"), &ctx), Decision::allow());
        assert_eq!(evaluate(&hook("write_file", "Path", r"c:\dev\seriousplanetdata\lanes\ag2-state.md"), &ctx), Decision::allow());
        let record = r"c:\dev\ag2_serious-planet-builder\docs\milestones\S-Region-Authoring\E2-record.md";
        assert_eq!(evaluate(&hook("write_file", "Path", record), &ctx), Decision::allow(), "a record is not the sprint file");

        for denied in [
            r"c:\dev\serious-planet-builder\CLAUDE.md",
            r"c:\dev\ag_serious-planet-builder\GEMINI.md",
            r"c:\dev\seriousplanetdata\lanes\explorer.md",
            r"c:\dev\seriousplanetdata\lanes\digest\2026-11-19-morning.md",
            r"c:\dev\seriousplanetdata\worlds\earth\pack.bin",
            r"c:\dev\ag2_serious-planet-builder\docs\constitution.md",
            r"c:\dev\ag2_serious-planet-builder\docs\status.md",
            r"c:\dev\ag2_serious-planet-builder\docs\milestones\S-Region-Authoring.md",
            r"c:\dev\ag2_serious-planet-builder\.agents\scripts\helper.ps1",
        ] {
            let d = evaluate(&hook("edit_file", "Path", denied), &ctx);
            assert_eq!(d.kind, DecisionKind::Deny, "expected deny for {denied}: {d:?}");
        }
    }

    // spec: BUILD-062
    #[test]
    fn shell_commands_distinguish_owned_repo_from_readonly_siblings_and_allow_reads() {
        let ctx = GuardContext::default();
        assert_eq!(
            evaluate(&hook("run_command", "CommandLine", r"git -C c:/dev/serious-planet-builder show origin/main:CLAUDE.md"), &ctx),
            Decision::allow()
        );
        assert_eq!(
            evaluate(&hook("run_command", "CommandLine", r"git -C c:/dev/ag2_serious-planet-builder commit -m 'ok'"), &ctx),
            Decision::allow()
        );
        assert_eq!(
            evaluate(&hook("run_command", "CommandLine", r"Copy-Item a.md c:/dev/seriousplanetdata/lanes/ag2-outbox.md"), &ctx),
            Decision::allow()
        );

        for denied in [
            r"git -C c:/dev/serious-planet-builder checkout main",
            r"Remove-Item c:/dev/ag_serious-planet-builder/README.md",
            r"Set-Content c:/dev/seriousplanetdata/lanes/explorer.md 'x'",
            r"Set-Content docs/milestones/S-Region-Authoring.md 'x'",
            r"cargo xtask perf",
            r"cargo xtask quiet off",
            r"git push --force origin main",
        ] {
            let d = evaluate(&hook("run_command", "CommandLine", denied), &ctx);
            assert_eq!(d.kind, DecisionKind::Deny, "expected deny for {denied}: {d:?}");
        }
    }

    // spec: BUILD-062
    #[test]
    fn cargo_commands_yield_when_quiet_or_exclusive_lock_is_active() {
        let quiet_ctx = GuardContext { quiet_age_min: Some(12), exclusive_holder: None };
        assert_eq!(evaluate(&hook("run_command", "CommandLine", "cargo xtask check"), &quiet_ctx).kind, DecisionKind::Deny);
        assert_eq!(
            evaluate(
                &hook("run_command", "CommandLine", "cargo run --quiet --manifest-path ../xtask/Cargo.toml -- agent-guard"),
                &quiet_ctx
            ),
            Decision::allow()
        );

        let excl_ctx = GuardContext { quiet_age_min: None, exclusive_holder: Some("rg_chain (exclusive)".into()) };
        assert_eq!(evaluate(&hook("run_command", "CommandLine", "cargo test -p planet-editor"), &excl_ctx).kind, DecisionKind::Deny);
        assert_eq!(evaluate(&hook("run_command", "CommandLine", "python build_cockpit.py"), &excl_ctx), Decision::allow());

        assert_eq!(
            evaluate(&hook("replace_file_content", "TargetFile", r"c:\dev\serious-planet-builder\CLAUDE.md"), &GuardContext::default())
                .kind,
            DecisionKind::Deny
        );
    }

    // spec: BUILD-062
    #[test]
    fn malformed_json_fails_safe_with_force_ask_and_escapes_roundtrip() {
        let bad = evaluate("{not json", &GuardContext::default());
        assert_eq!(bad.kind, DecisionKind::ForceAsk);
        let d = Decision::deny("quote \"here\" and \\slash\nline2");
        let j = d.to_json();
        assert!(j.contains(r#"quote \"here\" and \\slash\nline2"#), "{j}");
    }

    // spec: BUILD-062
    #[test]
    fn git_push_blocks_direct_main_bare_and_force_while_allowing_feature_branches_and_commit_messages() {
        let ctx = GuardContext::default();
        for allowed in [
            r"git push -u origin change/xtask-guard",
            r"git push origin change/main-stem-fix",
            r"git push origin main-stem-fix",
            r"git push origin HEAD:change/feature",
            r"git push origin --delete change/old-branch",
            r"git -C c:/dev/ag2_serious-planet-builder push -u origin change/ok",
            r#"git commit -m "guard: block git push origin main and git push --force""#,
            r"gh pr merge 1622 --squash --delete-branch",
        ] {
            let d = evaluate(&hook("run_command", "CommandLine", allowed), &ctx);
            assert_eq!(d, Decision::allow(), "expected allow for {allowed}: {d:?}");
        }

        for denied in [
            r"git push",
            r"git push origin",
            r"git push origin main",
            r"git push -u origin main",
            r"git push origin HEAD",
            r"git push origin HEAD:main",
            r"git push origin HEAD:refs/heads/main",
            r"git push origin change/foo:main",
            r"git push origin :main",
            r"git push origin --delete main",
            r"git push --force origin change/foo",
            r"git push -f origin change/foo",
            r"git push -uf origin change/foo",
            r"git push --force-with-lease origin change/foo",
            r"git push --all origin",
            r"git push --tags origin",
            r"git push origin +change/foo",
            r"git -C c:/dev/ag2_serious-planet-builder push origin main",
            r"git status; git push origin main",
            r"& git push origin main",
            r"gh repo delete mdondorff/Serious-Planet-builder",
            r"gh repo archive mdondorff/Serious-Planet-builder",
        ] {
            let d = evaluate(&hook("run_command", "CommandLine", denied), &ctx);
            assert_eq!(d.kind, DecisionKind::Deny, "expected deny for {denied}: {d:?}");
        }
    }
}
