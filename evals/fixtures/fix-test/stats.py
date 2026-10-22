"""Small statistics helpers."""


def mean(values):
    """The arithmetic mean of a non-empty sequence."""
    return sum(values) / len(values)


def median(values):
    """The middle value; for an even count, the mean of the two middle values."""
    ordered = sorted(values)
    middle = len(ordered) // 2
    if len(ordered) % 2:
        return ordered[middle]
    return ordered[middle]
