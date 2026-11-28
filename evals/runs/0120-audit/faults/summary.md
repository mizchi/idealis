| fault | change | named control | detected | failing lib tests | sub-inputs failing |
| --- | --- | --- | --- | ---: | --- |
| F1 | hop does not increase `forwarded` (r1 loop) | `v2_recursive_forwarding_keeps_the_former_limit`, `v2_recursion_entered_after_forwarding_keeps_the_former_limit`, `v2_forwarded_depth_reaches_enabled_and_snapshots`, `v2_enabled_subscripts_keep_the_recursive_limit_for_forwarded_parameters` | yes | 4 | snapshot, step, subscript |
| F2a | one `consume(1)` per chain | `v2_forwarding_hops_charge_one_unit_of_work_each` | yes | 11 | snapshot, step, subscript |
| F2b | follow the chain first, charge `consume(n)` once at the end | `v2_forwarding_charges_each_hop_before_the_next_reference` | yes | 5 | - |
| F3a | terminal context built without `prime` | `v2_primed_forwarding_keeps_the_primed_values` | yes | 2 | - |
| F3b | a `Prime` root actual is passed through as a forward | `v2_primed_forwarding_keeps_the_primed_values` | yes | 1 | - |
| F4 | terminal uses the user side `locals` and `old` | `v2_forwarded_bound_and_old_keep_their_lexical_values` | yes | 11 | - |
| F5a | ENABLED subscript context drops `forwarded` | `v2_forwarded_depth_reaches_enabled_and_snapshots` | yes | 1 | subscript |
| F5b | ENABLED step context drops `forwarded` | `v2_forwarded_depth_reaches_enabled_and_snapshots` | yes | 1 | step |
| F5c | `Snapshot` drops `forwarded` | `v2_forwarded_depth_reaches_enabled_and_snapshots` | NO (0 of 1) at evaluation; control `v2_applied_lazy_function_keeps_the_forwarded_depth_of_its_snapshot` added by correction-0003 and corrected by correction-0004: fails under F5c, F5d and F1, passes on before, after and both oracle builds | 0 at evaluation (1 with the added control) | - |
| F5d | `Snapshot` stores `forwarded` one less (`context.forwarded.saturating_sub(1)`), added after the evaluation | `v2_applied_lazy_function_keeps_the_forwarded_depth_of_its_snapshot` | yes (the correction-0003 values 70 and 63 alone did not detect it) | 1 | passes 61, 60, 57 |
| F6 | no judgement at intermediate hops | `v2_forwarding_refuses_before_the_next_reference` | yes | 3 | - |
| F7 | 256 judgement placed before the v1 early return | `v1_enabled_subscripts_at_256_skip_the_depth_check` | yes | 1 | - |

baseline (no fault): test result: ok. 164 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s
faults not detected by their named control at evaluation: ['F5c'] (detected after correction-0003 and correction-0004; F5d added)
