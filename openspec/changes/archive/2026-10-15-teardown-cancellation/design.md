## Decisions

- Cancellation reaches the queue by construction: the streamer keeps its queue itself and hands the spawner only `max_in_flight` jobs, so dropping the streamer empties the queue and sets the tokens of the dispatched jobs; a job still waiting in the spawner (the pool channel) sees its token at the start and ends without calling the source. `ThreadPool::drop` therefore joins only workers that are inside a source call.
- A source decides its stage boundaries (`load_staged`). The tile source's stage is the height sample: once cancelled, `CancelGuarded` answers 0 without reading the store, the generation finishes at once and the result is discarded (never returned, so nothing depends on timing, CON-14: a cancelled job's tile is never resident).
- The plan job cannot be interrupted inside a plan (no stage boundary in node selection); teardown waits for that plan only. `PlanPipeline::drop` closes the worker so nothing follows it.
