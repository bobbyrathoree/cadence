import copy
import importlib.util
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "check_feature_graph.py"
SPEC = importlib.util.spec_from_file_location("check_feature_graph", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)

ROOT_ID = "opaque-root-id"
CORE_ID = "opaque-core-id"
DEP_ID = "opaque-dependency-id"


def metadata():
    return {
        "packages": [
            {"id": ROOT_ID, "name": "cadence-mcp"},
            {"id": CORE_ID, "name": "cadence-core"},
            {"id": DEP_ID, "name": "serde"},
        ]
    }


def graph():
    return {
        "version": 1,
        "units": [
            {"pkg_id": ROOT_ID, "features": []},
            {"pkg_id": CORE_ID, "features": []},
            {"pkg_id": DEP_ID, "features": ["derive"]},
        ],
        "roots": [0],
    }


class FeatureGraphFixtureTests(unittest.TestCase):
    def assert_rejected(self, candidate_graph, candidate_metadata=None):
        with self.assertRaises(CHECKER.AuditError):
            CHECKER.check_feature_graph(
                candidate_graph,
                candidate_metadata if candidate_metadata is not None else metadata(),
                "cadence-mcp",
            )

    def test_root_has_test_support_fails(self):
        candidate = graph()
        candidate["units"][0]["features"] = ["test-support"]
        self.assert_rejected(candidate)

    def test_root_has_test_faults_fails(self):
        candidate = graph()
        candidate["units"][0]["features"] = ["test-faults"]
        self.assert_rejected(candidate)

    def test_root_has_lifecycle_test_fails(self):
        candidate = graph()
        candidate["units"][0]["features"] = ["lifecycle-test"]
        self.assert_rejected(candidate)

    def test_core_unit_has_test_support_fails(self):
        candidate = graph()
        candidate["units"][1]["features"] = ["test-support"]
        self.assert_rejected(candidate)

    def test_empty_root_features_pass(self):
        CHECKER.check_feature_graph(graph(), metadata(), "cadence-mcp")

    def test_zero_roots_fails(self):
        candidate = graph()
        candidate["roots"] = []
        self.assert_rejected(candidate)

    def test_zero_matching_root_units_fails(self):
        candidate = graph()
        candidate["roots"] = [2]
        self.assert_rejected(candidate)

    def test_multiple_root_ids_fails(self):
        candidate = graph()
        candidate["roots"] = [0, 2]
        self.assert_rejected(candidate)

    def test_malformed_and_unknown_version_fail(self):
        self.assert_rejected([])
        candidate = graph()
        candidate["version"] = 2
        self.assert_rejected(candidate)

    def test_pkg_id_join_miss_fails(self):
        candidate = graph()
        candidate["units"][2]["pkg_id"] = "unknown-opaque-id"
        self.assert_rejected(candidate)

    def test_zero_core_units_fails(self):
        candidate = graph()
        candidate["units"] = [
            unit for unit in candidate["units"] if unit["pkg_id"] != CORE_ID
        ]
        self.assert_rejected(candidate)

    def test_fixture_mutations_do_not_share_state(self):
        first = graph()
        second = copy.deepcopy(first)
        first["units"][0]["features"].append("test-support")
        CHECKER.check_feature_graph(second, metadata(), "cadence-mcp")


if __name__ == "__main__":
    unittest.main()
