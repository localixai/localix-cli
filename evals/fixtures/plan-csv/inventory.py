"""The items in stock: name, quantity, unit price."""

ITEMS = [
    ("bolts, M4", 120, 0.15),
    ("nuts, M4", 200, 0.05),
    ("washers", 75, 0.02),
]


def value(qty, price):
    return round(qty * price, 2)
