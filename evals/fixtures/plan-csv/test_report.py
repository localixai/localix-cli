import unittest

from report import table


class TableTest(unittest.TestCase):
    def test_has_a_header_and_a_row_per_item(self):
        lines = table([("pins", 2, 0.5)]).splitlines()
        self.assertEqual(len(lines), 2)
        self.assertTrue(lines[0].startswith("item"))
        self.assertIn("1.00", lines[1])


if __name__ == "__main__":
    unittest.main()
