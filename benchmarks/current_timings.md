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
| 4x4 | random | 0.215 | 0.160 | 0.278 | 57 |
| 4x4 | first_against_rest | 0.113 | 0.106 | 0.150 | 17 |
| 10x10 | random | 27.645 | 23.186 | 31.077 | 1416 |
| 10x10 | first_against_rest | 6.482 | 6.096 | 7.994 | 241 |
