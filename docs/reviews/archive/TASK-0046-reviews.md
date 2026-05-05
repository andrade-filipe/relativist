# TASK-0046 Reviews: Wire classification logic

## Stage 4-6: Combined Review — PASS

- `classify_wires` implements SPEC-04 Section 4.4 Step 4
- Detects border wires only from smaller-ID side (avoids duplicates)
- Generates border entries for BOTH partitions in single pass
- Border IDs start from `max_freeport_id(net) + 1` (R12)
- Returns `WireClassification` struct with borders, border_entries, ID range
- 9 new tests: empty, internal, single border, border ID after FreePort, multiple, interface, range, no preexisting, no duplicates
- 274 total tests. Clippy clean, fmt clean
