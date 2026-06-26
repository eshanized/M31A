# Real Repository Benchmark Results

**Methodology:** 30 runs per (repository, backend) pair, 95% confidence intervals

| Repository | Files | Backend | Build (ms) | 95% CI | Query (ms) | 95% CI | Ratio |
|-----------|-------|---------|------------|--------|------------|--------|-------|
| gin | 99 | EFIE | 48.6 ± 4.3 | [47.1, 50.2] | 0.4 ± 0.4 | [0.2, 0.5] | 12.5x build, 28.1x query |
| | | Original | 3.9 ± 0.6 | [3.7, 4.1] | 0.0 ± 0.0 | [0.0, 0.0] | | |
| docker | 10218 | EFIE | 934.4 ± 42.0 | [919.4, 949.5] | 4.0 ± 0.9 | [3.6, 4.3] | 12.3x build, 257.5x query |
| | | Original | 76.0 ± 3.8 | [74.6, 77.3] | 0.0 ± 0.0 | [0.0, 0.0] | | |
| go-stdlib | 11466 | EFIE | 8578.8 ± 547.0 | [8383.1, 8774.6] | 18.4 ± 6.6 | [16.1, 20.8] | 22.9x build, 736.0x query |
| | | Original | 375.0 ± 25.3 | [366.0, 384.1] | 0.0 ± 0.0 | [0.0, 0.0] | | |
| kubernetes | 17266 | EFIE | 5926.5 ± 421.0 | [5775.8, 6077.1] | 6.7 ± 6.6 | [4.3, 9.0] | 21.1x build, 244.8x query |
| | | Original | 280.4 ± 23.7 | [271.9, 288.9] | 0.0 ± 0.0 | [0.0, 0.0] | | |
