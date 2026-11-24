// Facts over many builtins, each expected to hold. Quint's simulator decides
// whether they hold; rquint's translation must agree (plan 0089).
module Coverage {
  type Shape = Circle(int) | Square(int) | Empty
  var n: int
  action init = n' = 0
  action step = any {
    all { n < 3, n' = n + 1 },
    all { n >= 3, n' = 0 },
  }
  val lists = and {
    List(10, 20, 30).nth(0) == 10,
    List(10, 20, 30).nth(2) == 30,
    List(1, 2).length() == 2,
    List(1, 2).append(3) == List(1, 2, 3),
    List(1).concat(List(2, 3)) == List(1, 2, 3),
    List(4, 5, 6).head() == 4,
    List(4, 5, 6).tail() == List(5, 6),
    List(1, 2, 3).foldl(0, (acc, x) => acc * 10 + x) == 123,
    // Added after the fold-initial fault went undetected (0089).
    List(1, 2).foldl(100, (acc, x) => acc + x) == 103,
    Set(1, 2).fold(10, (acc, x) => acc + x) == 13,
  }
  val sets = and {
    Set(1, 2).union(Set(2, 3)) == Set(1, 2, 3),
    Set(1, 2).intersect(Set(2, 3)) == Set(2),
    Set(1, 2, 3).exclude(Set(2)) == Set(1, 3),
    Set(1).subseteq(Set(1, 2)),
    Set(1, 2, 3).size() == 3,
    Set(1, 2).map(x => x * 2) == Set(2, 4),
    Set(1, 2, 3, 4).filter(x => x % 2 == 0) == Set(2, 4),
    Set(1, 2, 3).exists(x => x > 2),
    Set(1, 2, 3).forall(x => x > 0),
    1.to(3) == Set(1, 2, 3),
    Set(1, 2).powerset().size() == 4,
    Set(Set(1), Set(2)).flatten() == Set(1, 2),
    Set(1, 2).contains(2),
    3.in(Set(3)),
  }
  val records = and {
    { a: 1, b: 2 }.a == 1,
    { a: 1, b: 2 }.with("a", 5) == { a: 5, b: 2 },
    (7, 8)._2 == 8,
  }
  val maps = and {
    Set(1, 2).mapBy(x => x + 1).get(2) == 3,
    Set(1, 2).mapBy(x => x + 1).keys() == Set(1, 2),
    Set(1, 2).mapBy(x => x).set(1, 9).get(1) == 9,
    Set(1).mapBy(x => x).put(5, 6).get(5) == 6,
  }
  val arithmetic = and {
    7 / 2 == 3,
    7 % 2 == 1,
    -(3) == 0 - 3,
    2 ^ 3 == 8,
    (1 > 0) implies (2 > 1),
    (1 > 0) iff (2 > 1),
    (if (n >= 0) 1 else 2) == 1,
  }
  def area(s: Shape): int = match s {
    | Circle(r) => 3 * r * r
    | Square(side) => side * side
    | _ => 0
  }
  val variants = and {
    area(Circle(2)) == 12,
    area(Square(3)) == 9,
    area(Empty) == 0,
  }
  val nondets = n >= 0 and n <= 3
  val all_facts = lists and sets and records and maps and arithmetic and variants and nondets
  val broken_index = List(10, 20, 30).nth(1) == 10
}
