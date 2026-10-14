## Decision

The check is a lint rule, not a requirement of its own: it belongs to TEST-001 (spec traceability, CON-16), whose job is that the specs are well-formed. "TBD" is matched case-insensitively anywhere in the Purpose, so a half-edited placeholder is also caught. A missing section is an error too, since OpenSpec's archive always writes one. The constitution is untouched.
