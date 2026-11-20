module SetFold {
  var total: int
  action init = total' = Set(3, 1, 2).fold(0, (acc, element) => acc + element)
  action step = total' = total
  val safety = total == 6
  val broken = total == 5
}
