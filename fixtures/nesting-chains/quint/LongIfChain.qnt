module LongIfChain {
  var x: int
  action init = x' = 0
  action step = x' = if (x < 55) x + 1 else 0
  val label = if (x == 0) 100 else if (x == 1) 101 else if (x == 2) 102 else if (x == 3) 103 else if (x == 4) 104 else if (x == 5) 105 else if (x == 6) 106 else if (x == 7) 107 else if (x == 8) 108 else if (x == 9) 109 else if (x == 10) 110 else if (x == 11) 111 else if (x == 12) 112 else if (x == 13) 113 else if (x == 14) 114 else if (x == 15) 115 else if (x == 16) 116 else if (x == 17) 117 else if (x == 18) 118 else if (x == 19) 119 else if (x == 20) 120 else if (x == 21) 121 else if (x == 22) 122 else if (x == 23) 123 else if (x == 24) 124 else if (x == 25) 125 else if (x == 26) 126 else if (x == 27) 127 else if (x == 28) 128 else if (x == 29) 129 else if (x == 30) 130 else if (x == 31) 131 else if (x == 32) 132 else if (x == 33) 133 else if (x == 34) 134 else if (x == 35) 135 else if (x == 36) 136 else if (x == 37) 137 else if (x == 38) 138 else if (x == 39) 139 else if (x == 40) 140 else if (x == 41) 141 else if (x == 42) 142 else if (x == 43) 143 else if (x == 44) 144 else if (x == 45) 145 else if (x == 46) 146 else if (x == 47) 147 else if (x == 48) 148 else if (x == 49) 149 else if (x == 50) 150 else if (x == 51) 151 else if (x == 52) 152 else if (x == 53) 153 else 0
  val safety = label >= 0
  val broken = label != 153
}
