// Counts up by three without a bound (plan 0096). Within eight steps the
// count reaches 21, beyond 5-bit integers (-16..15): a wrapping SAT encoding
// would see -11 and report a false counterexample to nonnegative.
module Overflow {
  var count: int
  action init = count' = 0
  action step = count' = count + 3
  val nonnegative = count >= 0
  val small = count < 9
}
