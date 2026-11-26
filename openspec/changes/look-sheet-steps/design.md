## Design
- The stage list lives in the store, not in the command. A pipeline that writes crops names its stages as it writes them (`CropWriter::stage`), so B, a chain run and a later pipeline need no code in the sheet.
- Fallback: crop stores of the spike branch's chain runner have no `stages.txt`; they take BUILD-067's eight columns, those present.
- The layout follows `step_row.py` with one change: rows are stages and the two columns are the stage and its step, so one sheet reads top to bottom as the pipeline.
- The step scale is `step_row.py`'s 2 m, not BUILD-067's 10 m: consecutive stages differ by less than a switch does.
- The difference tint reuses `stage_sheet::tint_change` over the stage's own shade, like BUILD-067's difference row; `compose` gets its column labels and the empty-panel note as arguments.
- `steps.txt` adds the signed mean to `look.txt`'s numbers, so a sign is readable without the image.
