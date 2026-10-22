import unittest

from stats import mean, median


class MeanTest(unittest.TestCase):
    def test_mean(self):
        self.assertEqual(mean([1, 2, 3, 4]), 2.5)


class MedianTest(unittest.TestCase):
    def test_odd_count(self):
        self.assertEqual(median([3, 1, 2]), 2)

    def test_even_count(self):
        self.assertEqual(median([4, 1, 3, 2]), 2.5)

    def test_single_value(self):
        self.assertEqual(median([7]), 7)


if __name__ == "__main__":
    unittest.main()
