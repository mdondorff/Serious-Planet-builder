## MODIFIED Requirements

### Requirement: TEST-006 Performance protocol
The performance harness SHALL discard warm-up samples, report median and 99th percentile (nearest rank) per run, reject non-finite samples, log GPU clocks, power, temperature, throttle reasons and the active power scheme, record the owner's machine-ready confirmation, and report run-to-run noise as the relative spread of run medians, and SHALL mark a session invalid for time budgets (with the reasons in the log and on the console) when the power scheme is not a performance scheme or the GPU was in a low performance state when a run started.

Verify: A
Status: active
Source: CON-21, report §8

#### Scenario: Statistics
- **WHEN** samples 1 to 100 follow two warm-up outliers
- **THEN** the median is 50.5, the p99 is 99 and the outliers are ignored

#### Scenario: Balanced power profile
- **WHEN** a session ran under the Balanced power scheme or started with the GPU idle
- **THEN** the log contains `valid_for_budgets` false and a warning naming the scheme or the performance state

#### Scenario: Log content
- **WHEN** a run report is serialised
- **THEN** it contains the confirmation flag, median, clocks before and after, and the protocol parameters

