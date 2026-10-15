## Decisions

See ADR 0034 (proposed). Granularity is the view, derived from the recorded clock samples by one pure function used by the writer and the reader (old logs included). GPU-clock-dependent metrics: `gpu_ms` and `frame_ms`. The session-level P5-or-lower warning of ADR 0020 stays. The warm-up is a closure-driven function so tier A uses a fake renderer, sampler and time source.
