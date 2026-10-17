## Rule
Files come from `git ls-files` (tracked only; no extra setup in CI, git is on every runner). Files that are not valid UTF-8, contain a NUL byte or exceed 2 MiB are skipped.
A line (CR trimmed) is an opening marker if it is `<<<<<<<` or starts with `<<<<<<< `, a closing marker likewise with `>>>>>>>`, and a separator if it is exactly `=======`.
A file is reported when it has an opening, later a separator, later a closing marker (a complete block: report the opening line), or a git-format opening or closing marker alone (`<<<<<<< ref`, `>>>>>>> ref`, i.e. the marker followed by a space and a non-empty name). A lone `=======` (markdown setext underline) or a bare `<<<<<<<` never counts.
## False positives
No exempt path. A document quoting a marker line of git format at the start of a line must indent it or fence it differently; fixtures are built at run time.
