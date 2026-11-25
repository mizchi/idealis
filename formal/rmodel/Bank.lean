import Lean

open Lean

def InRange (amount : Int) : Prop := 0 ≤ amount ∧ amount ≤ 4

instance (amount : Int) : Decidable (InRange amount) :=
  inferInstanceAs (Decidable (0 ≤ amount ∧ amount ≤ 4))

theorem zeroInRange : InRange 0 := by decide

def checkBank (observation : Json) : Except String Bool := do
  let state ← observation.getObjVal? "state"
  let balances ← (← state.getObjVal? "balance").getObj?
  if balances.isEmpty then
    throw "expected at least one balance"
  let amounts ← balances.toList.mapM fun (_, value) => fromJson? (α := Int) value
  return amounts.all fun amount => decide (InRange amount)

def main : IO Unit := do
  let input ← (← IO.getStdin).readToEnd
  let checks := do
    let states ← (← Json.parse input).getArr?
    states.mapM checkBank
  match checks with
  | .ok verdicts => IO.println (toJson verdicts).compress
  | .error message => throw (IO.userError message)
