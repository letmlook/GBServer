import unittest

from smoke_result import csv_success, missing_source_rejected, smoke_exit_code


class SmokeResultTests(unittest.TestCase):
    def test_csv_requires_success_and_expected_header(self):
        self.assertTrue(csv_success(200, "\ufeffid,time,level,logger,thread,source,message\n"))
        self.assertFalse(csv_success(500, "id,time,level,logger,thread,source,message\n"))
        self.assertFalse(csv_success(200, "<html>not csv</html>"))

    def test_missing_source_must_be_a_zlm_404(self):
        self.assertTrue(missing_source_rejected(200, 500, "ZLM error: -1 - DESCRIBE:404 Stream Not Found"))
        self.assertFalse(missing_source_rejected(200, 500, "ZLM request failed: connect error"))
        self.assertFalse(missing_source_rejected(200, 0, "success"))

    def test_unexpected_failures_set_exit_code(self):
        self.assertEqual(smoke_exit_code([]), 0)
        self.assertEqual(smoke_exit_code([("broken endpoint",)]), 1)


if __name__ == "__main__":
    unittest.main()
