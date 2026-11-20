module SequenceFold {
  var total: int
  action init = total' = List(3, 1, 2).foldl(0, (acc, element) => acc * 10 + element)
  action step = total' = total
  val safety = total == 312
  val broken = total == 123
}
