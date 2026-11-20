module LongConjunction {
  var x: int
  action init = x' = 0
  action step = x' = if (x < 5) x + 1 else 0
  val safety = x != 10 and x != 11 and x != 12 and x != 13 and x != 14 and x != 15 and x != 16 and x != 17 and x != 18 and x != 19 and x != 20 and x != 21 and x != 22 and x != 23 and x != 24 and x != 25 and x != 26 and x != 27 and x != 28 and x != 29 and x != 30 and x != 31 and x != 32 and x != 33 and x != 34 and x != 35 and x != 36 and x != 37 and x != 38 and x != 39 and x != 40 and x != 41 and x != 42 and x != 43 and x != 44 and x != 45 and x != 46 and x != 47 and x != 48 and x != 49 and x != 50 and x != 51 and x != 52 and x != 53
  val broken = x != 10 and x != 11 and x != 12 and x != 13 and x != 14 and x != 15 and x != 16 and x != 17 and x != 18 and x != 19 and x != 20 and x != 21 and x != 22 and x != 23 and x != 24 and x != 25 and x != 26 and x != 27 and x != 28 and x != 29 and x != 30 and x != 31 and x != 32 and x != 33 and x != 34 and x != 35 and x != 36 and x != 37 and x != 38 and x != 39 and x != 40 and x != 41 and x != 42 and x != 43 and x != 44 and x != 45 and x != 46 and x != 47 and x != 48 and x != 49 and x != 50 and x != 51 and x != 52 and x != 4
}
