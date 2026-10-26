## ADDED Requirements

### Requirement: STRM-055 The job log tells a refresh and its waits
The job log (STRM-022) SHALL record for each job the milliseconds on the streamer's clock from the request of its tile to its dispatch (the wait in the streamer's own queue) and whether the job rebuilds a resident tile that was stale (a refresh, STRM-026). `refresh_summary` SHALL put the refresh jobs of a slice of the log in one line: their count and the median, 90th percentile and largest of the wait in the streamer's queue, the wait in the spawner's queue and the load, in milliseconds; none without a refresh. The viewer SHALL write that line about the jobs since the last, once every 10 s while there are refreshes. The readings decide nothing (CON-14).

Verify: A
Status: active
Source: m8-refresh-priority, STRM-022, STRM-026

#### Scenario: A refresh after a wait
- **WHEN** a resident tile is refreshed and 250 ms of the clock pass before the step that dispatches it
- **THEN** its sample is a refresh that is done with a streamer queue wait of 250 ms, the samples of the missing tiles before it are not refreshes with a wait of 0, and the summary counts the one refresh
