"""Prints the stock report: one row per item, as an aligned table."""

import sys

from inventory import ITEMS, value


def table(items):
    lines = [f"{'item':<12}{'qty':>6}{'price':>9}{'value':>10}"]
    for name, qty, price in items:
        lines.append(f"{name:<12}{qty:>6}{price:>9.2f}{value(qty, price):>10.2f}")
    return "\n".join(lines)


def main(argv):
    if argv:
        sys.exit(f"unknown option: {argv[0]}")
    print(table(ITEMS))


if __name__ == "__main__":
    main(sys.argv[1:])
