import Lean

open Lean

def CorrectCapture (requested captured charges : Nat) : Prop :=
  requested > 0 ∧ captured = requested ∧ charges = 1

instance (requested captured charges : Nat) : Decidable (CorrectCapture requested captured charges) :=
  inferInstanceAs (Decidable (requested > 0 ∧ captured = requested ∧ charges = 1))

def units (amount : Json) : Except String Nat := do
  fromJson? (← amount.getObjVal? "units")

def checkCheckout (observation : Json) : Except String Bool := do
  let state ← observation.getObjVal? "state"
  let payments ← (← state.getObjVal? "payment").getObj?
  let requests ← state.getObjVal? "requested"
  let counts ← state.getObjVal? "chargeCount"
  if payments.isEmpty then
    throw "expected at least one order"
  let verdicts ← payments.toList.mapM fun (order, payment) => do
    let tag ← (← payment.getObjVal? "tag").getStr?
    let charges ← fromJson? (α := Nat) (← counts.getObjVal? order)
    match tag with
    | "Captured" =>
      let requested ← units (← requests.getObjVal? order)
      let captured ← units (← payment.getObjVal? "value")
      return decide (CorrectCapture requested captured charges)
    | "Unplaced" | "Pending" | "Rejected" => return charges == 0
    | _ => throw s!"unknown payment tag {tag}"
  return verdicts.all id

def main : IO Unit := do
  let input ← (← IO.getStdin).readToEnd
  let checks := do
    let states ← (← Json.parse input).getArr?
    states.mapM checkCheckout
  match checks with
  | .ok verdicts => IO.println (toJson verdicts).compress
  | .error message => throw (IO.userError message)
