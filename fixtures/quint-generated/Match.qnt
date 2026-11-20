module Match {
  type Phase = Idle | Busy | Done
  var phase: Phase
  action init = phase' = Idle
  action step = phase' = match phase {
    | Idle => Busy
    | Busy => Done
    | Done => Idle
  }
  val safety = match phase {
    | Idle => true
    | Busy => true
    | Done => true
  }
  val broken = phase != Done
}
