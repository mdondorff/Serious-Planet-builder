## Design notes
- Form detection: a milestone file with an `Opened:` line is a sprint file; any other keeps BUILD-053's exit table. P3 is read as before.
- Selection: one `active` file, one `active` and one `draft`, or one `draft` alone. Two actives, two drafts, or neither fail. A draft must be a sprint file.
- Header: `Opened`, `Closes by`, `Owner look` and `R&D lane` are read from one line, split at the labels. `Closes by` is at most 7 days after `Opened`; `Owner look` is a date or `none`.
- Exit cells: Item, Mode, Done when, Gate, Park, Lane and State must be filled; Result may be `-`. A Gate part is `<class>: ...` with a class of num, fun, dom or look (long forms allowed). `-` in Park means none (a look or process row).
- States: open, running, look-pending, waiting-owner, `done <date>`, `promoted <record>`, `parked <record>`, `carried <M>`. A bare record name is in `docs/milestones/<M>/`; a name with a `/` is relative to the repository (a spike's results).
- Records: the first table with the columns criterion, class and verdict. A promotion refuses a `fail` or empty verdict; promotion and park both refuse an exception on a `num` criterion. The date-set check against the ledger's first row waits with the ledger counters (3b).
- Decisions: open means state `open` or `waiting-owner`. A safe default starts with park, keep-off or keep today's (A1). Past `By`, `plan` prints that a safe default fires and that any other waits for the owner. A `default taken` state with an unsafe default fails.
- `--pr`: a title naming an R or P exit (not promoted) allows only `docs/` and `spikes/`; a `change/*` branch must name an X or M exit or a promoted P exit; past `Closes by`, a PR touching more than `docs/` must change the sprint file. A docs-only PR never fails.
- Queue: an active sprint's open exits each need a queue row, as in BUILD-053; promoted, parked and carried rows count as closed.
