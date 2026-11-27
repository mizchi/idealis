# IR compatibility fixtures

`pkl-bank-v1.json` is the original Pkl adapter output from commit
`898b101d3` at `fixtures/rcheck/adapters/pkl/bank.json`. Its SHA-256 is
`4022e7e5068ae3d245726d3ec8d200af38f6ece0ee8209ee7936cc22b3288e8a`.
Keep this input fixed when the adapter or IR writer is updated. The
compatibility test compares its results with the source model and
replays its reachability witness.
