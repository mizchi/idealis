module NestedFold {
  var total: int
  action init = total' = List(1, 2).foldl(0, (acc, outer) =>
    acc + Set(outer, outer + 1).fold(0, (inner, element) => inner + element))
  action step = total' = total
  val safety = total == 8
  val broken = total == 6
}
