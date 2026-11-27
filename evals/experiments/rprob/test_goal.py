import unittest

from goal import classify, score


class GoalTest(unittest.TestCase):
    def test_threshold_and_noise(self):
        self.assertEqual(classify([1.0] * 5, [0.8] * 5), "improved")
        self.assertEqual(classify([1.0] * 5, [0.95] * 5), "no-gain")
        self.assertEqual(classify([1.0] * 5, [1.2] * 5), "regressed")
        self.assertEqual(classify([1.0] * 5, [0.75, 0.8, 0.8, 0.8, 1.0]), "unmeasured")
        self.assertEqual(classify([1.0] * 5, [0.8] * 4), "unmeasured")

    def test_measurement_is_bound_to_pinned_inputs(self):
        import copy
        import json
        from goal import POLICY, commands, digest

        plan = {
            "schema": "idealis-rprob-goal-v1",
            "revision": {"candidate": "sha256:bench", "rule": "measure", "evaluation_revision": "v1:example"},
            "policy": POLICY,
            "benchmark_sha256": "bench",
            "model_sha256": "model",
            "platform": {"os": "Darwin", "architecture": "arm64"},
            "hyperfine_version": "2.0.0",
        }
        document = {
            "schema_version": 2,
            "primary_metric": "time_wall_clock",
            "metadata": {"platform": plan["platform"], "hyperfine_version": "2.0.0"},
            "results": [
                {
                    "name": name, "command": command,
                    "measurements": [
                        {"exit_code": 0, "time_wall_clock": {"value": time, "unit": "second"}}
                        for _ in range(5)
                    ],
                }
                for (name, command), time in zip(commands().items(), [1.0, 0.8])
            ],
        }
        output = b'scope Holds, declarations Holds\nVerdict { check: "Full", outcome: Reachable { depth: 12 } }\n'

        def assess(raw, *, received=None, actual_output=output):
            payload = json.dumps(raw).encode()
            receipt = {
                "plan": plan if received is None else received,
                "measurements_sha256": digest(payload),
                "checked_output_sha256": digest(output),
            }
            return score(plan, receipt, payload, actual_output)

        self.assertEqual(assess(document), {"revision": plan["revision"], "outcome": "improved"})
        for field in ("benchmark_sha256", "model_sha256", "revision"):
            previous = {**plan, field: "old"}
            self.assertEqual(assess(document, received=previous)["outcome"], "unmeasured")
        self.assertEqual(assess(document, actual_output=b"changed")["outcome"], "unmeasured")
        edited = copy.deepcopy(document)
        edited["results"][1]["command"] = edited["results"][1]["command"].replace(" prepared ", " cold ", 1)
        self.assertEqual(assess(edited)["reason"], "command-mismatch")
        edited = copy.deepcopy(document)
        edited["results"][1]["measurements"][0]["exit_code"] = 1
        self.assertEqual(assess(edited)["reason"], "failed-or-invalid-sample")
        edited = copy.deepcopy(document)
        edited["results"][1]["measurements"].pop()
        self.assertEqual(assess(edited)["outcome"], "unmeasured")
