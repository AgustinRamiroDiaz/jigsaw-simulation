# Solver Benchmark Timings

Snapshot captured on 2026-06-07.

Command:

```bash
cargo bench --bench solver_strategies
```

Environment:

```text
rustc 1.96.0 (ac68faa20 2026-05-25)
cargo 1.96.0 (30a34c682 2026-05-25)
Linux az 6.17.0-35-generic #35~24.04.1-Ubuntu SMP PREEMPT_DYNAMIC Tue May 26 19:30:42 UTC 2 x86_64 x86_64 x86_64 GNU/Linux
```

Results:

| Case | Strategy | Avg ms | Min ms | Max ms | Attempts |
| --- | --- | ---: | ---: | ---: | ---: |
| 4x4 | random | 0.257 | 0.171 | 0.668 | 57 |
| 4x4 | first_against_rest | 0.119 | 0.112 | 0.154 | 17 |
| 4x4 | side_indexed | 0.244 | 0.185 | 0.527 | 15 |
| 10x10 | random | 28.072 | 23.068 | 32.061 | 1416 |
| 10x10 | first_against_rest | 6.814 | 6.260 | 10.146 | 241 |
| 10x10 | side_indexed | 4.746 | 4.544 | 5.155 | 99 |
