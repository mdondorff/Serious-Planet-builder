# 0079 The constitution's size cap follows the owner's amendments
Status: accepted (owner, 2026-10-23)
Date: 2026-10-23
Source: owner, 2026-10-23 (CON-08 amended to name chunks); ADR 0075, ADR 0078; BUILD-023

## Context
- ADR 0075 caps the constitution at its size when that ADR was accepted; a cap changes only by a new ADR.
- Each owner amendment grows the constitution by a sentence; a new ADR per amendment only to move the cap adds no decision.

## Decision
1. The constitution's cap is its size after the latest owner amendment; the amendment's PR sets BUILD-023's number and the lint constant to that size.
2. Every other cap of ADR 0075 changes only by a new ADR, as before.

## Consequences
- Growth of the constitution stays visible (one number per amendment) without an ADR per amendment.

## Licences of adopted code or techniques
None.
