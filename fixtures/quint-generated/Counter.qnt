module Counter {
  var count: int
  action init = count' = 0
  action step = count' = if (count < 3) count + 1 else 0
  val safety = count >= 0 and count <= 3
  val broken = count < 2
}
