# ADR-0004: Correlation and Trend Analysis Approach

## Status
Accepted

## Context
The spec requires trend analysis with period bucketing and correlation analysis between metrics. We needed to decide on statistical methods appropriate for a lightweight CLI tool.

## Decision

### Trend Analysis
- Use Ordinary Least Squares (OLS) linear regression on period-bucketed averages
- Period buckets: daily (YYYY-MM-DD), weekly (ISO week YYYY-Wxx), monthly (YYYY-MM)
- BTreeMap with string keys provides natural chronological ordering
- Direction threshold: slope > 0.01 = increasing, < -0.01 = decreasing, else stable
- 30-day projection extrapolates from slope, adjusted for period length

### Correlation Analysis
- Use Pearson correlation coefficient on daily averages
- Matching by date: only days where both metrics have data are included
- Interpretation bands: |r| < 0.3 = weak, < 0.7 = moderate, >= 0.7 = strong
- Minimum 2 data points required; fewer returns coefficient = 0

## Consequences
- Simple and fast — no external math libraries needed
- Linear regression is appropriate for short-term health trends
- Pearson correlation captures linear relationships well for health metrics
- Does not detect non-linear patterns (could add Spearman later if needed)
