# Performance (AC-20)

Measured with `cargo run --release --example bench -- <empty folder>` (see `src-tauri/examples/bench.rs`).

**Reference machine:** Mac16,12 (Apple silicon), 16 GB RAM, macOS 26.2.
**Reference dataset:** 10,000 pages × 8 blocks of mixed Thai/English text in a nested tree; one database with 100,000 records (title, number, status); database file 196 MB.

| NFR | Target | Measurement | Result |
| --- | --- | --- | ---: |
| NFR-01 | usable within 3 s | open workspace + load the sidebar tree (10,007 pages) | 129–368 ms |
| NFR-02 | search p95 ≤ 1 s | full-text search, p95 of 100 two-word queries (median 20 ms) | 44 ms |
| NFR-03 | typing never stalls | autosave of one page incl. search index and links, p95 (runs debounced, off the keystroke path) | 0.4 ms |
| — | — | open a page (blocks + breadcrumb), p95 | 1.1 ms |
| NFR-04 | 100,000 records with paging | first page of 200 records | 323 ms |
| NFR-04 | | filter (number > 500) + sort, first page of 200 | 367 ms |
| NFR-04 | | edit one record value, p95 | 105 ms |
| NFR-05 | 1,000-object canvas pans/zooms smoothly | not measured headless — Excalidraw renders on canvas; check interactively | — |

## Fixed while measuring

Loading a single record used to read every value of its database, so creating or editing records got slower as a database grew (≈ 68 ms per record at 100k). It now reads only that record's values.

## Known limits

- Database views filter and sort in memory (one database at a time). At 100k records a query takes ~0.3 s; much larger single databases would need SQL-side filtering.
- The first sidebar load varies with the OS file cache (129 ms warm, ~370 ms after other heavy I/O).
