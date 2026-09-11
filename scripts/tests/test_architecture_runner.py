"""Check the test harness without requiring Rust or cargo-pup."""

from pathlib import Path
import subprocess
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from check_architecture import Case, verify_pup_result


class DiagnosticClassificationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.violation = Case("bad", expected_rule="domain_inward_only")
        self.diagnostic = (
            "error: domain_inward_only: "
            "Use of module 'crate::infrastructure::Repository' is denied"
        )

    def result(self, code: int, output: str = "") -> subprocess.CompletedProcess[str]:
        return subprocess.CompletedProcess(["cargo-pup"], code, stdout=output)

    def test_matching_diagnostic_and_failure_are_accepted(self) -> None:
        verify_pup_result(self.violation, self.result(1, self.diagnostic))

    def test_silent_success_cannot_pass_a_negative_test(self) -> None:
        with self.assertRaises(AssertionError):
            verify_pup_result(self.violation, self.result(0))

    def test_warning_does_not_count_as_enforcement(self) -> None:
        with self.assertRaises(AssertionError):
            verify_pup_result(self.violation, self.result(0, self.diagnostic))

    def test_unrelated_compilation_error_is_rejected(self) -> None:
        with self.assertRaises(AssertionError):
            verify_pup_result(self.violation, self.result(101, "error[E0308]: wrong type"))

    def test_another_rule_does_not_satisfy_the_expected_rule(self) -> None:
        wrong = self.diagnostic.replace("domain_inward_only", "another_rule")
        with self.assertRaises(AssertionError):
            verify_pup_result(self.violation, self.result(1, wrong))

    def test_configuration_error_is_not_a_lint_success(self) -> None:
        with self.assertRaises(AssertionError):
            verify_pup_result(self.violation, self.result(1, "invalid config: domain_inward_only"))

    def test_signal_termination_is_not_an_expected_lint_failure(self) -> None:
        with self.assertRaises(AssertionError):
            verify_pup_result(self.violation, self.result(-9, self.diagnostic))

    def test_valid_code_must_pass(self) -> None:
        verify_pup_result(Case("good"), self.result(0))
        with self.assertRaises(AssertionError):
            verify_pup_result(Case("good"), self.result(1, self.diagnostic))

    def test_known_gap_is_explicitly_characterized_as_not_detected(self) -> None:
        gap = Case("fully_qualified", known_gap=True)
        verify_pup_result(gap, self.result(0))
        with self.assertRaises(AssertionError):
            verify_pup_result(gap, self.result(1, self.diagnostic))


if __name__ == "__main__":
    unittest.main()
