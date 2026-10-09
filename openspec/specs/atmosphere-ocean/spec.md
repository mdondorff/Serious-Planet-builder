# atmosphere-ocean Specification

## Purpose
TBD - created by archiving change m0-foundations. Update Purpose after archive.

## Requirements

### Requirement: ATMO-001 Atmosphere LUTs match the CPU reference
The atmosphere look-up tables (transmittance, multiple scattering, sky view, aerial perspective) SHALL match a CPU reference within a stated tolerance.

Verify: B
Status: planned
Source: report §15 (M5), Key Finding 7

#### Scenario: Differential
- **WHEN** the GPU LUTs and the CPU reference are computed for the same parameters
- **THEN** the maximum relative difference is within the tolerance

### Requirement: ATMO-002 Aerial perspective on all geometry
Aerial perspective SHALL be applied to all opaque geometry, and atmosphere parameters SHALL be physical so that other planets are a parameter change.

Verify: C
Status: planned
Source: report §8

#### Scenario: Distant terrain
- **WHEN** the ground view is rendered with and without aerial perspective
- **THEN** distant pixels differ in the expected direction (higher luminance, bluer)

### Requirement: ATMO-003 HDR and exposure
The pipeline SHALL be linear HDR with physical sun units and automatic exposure covering at least ten orders of magnitude of luminance.

Verify: D
Status: planned
Review: contact sheet of ground, limb and sunset views approved by the owner
Source: report §8

#### Scenario: Day to night
- **WHEN** the sun sets in a scripted view
- **THEN** exposure adapts without clipping or banding visible on the contact sheet

### Requirement: ATMO-004 Ocean and shoreline
The ocean SHALL render as a sphere-level surface with a shoreline that does not flicker as terrain level of detail changes.

Verify: C
Status: planned
Source: report §14, §17 item 13

#### Scenario: Coastal flight
- **WHEN** a scripted coastal flight is rendered in the shoreline debug view
- **THEN** consecutive frames differ only by the expected camera motion, with no isolated flicker pixels
