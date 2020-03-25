def append(a: [int], k: int) -> [int]:
    return a + [k]


def extend(a: [int], b: [int], b_start: int, b_end: int) -> [int]:
    extended: [int] = None
    i: int = 0

    extended = a
    i = b_start
    while i < b_end:
        extended = append(extended, b[i])
        i = i + 1
    return extended


def merge(left: [int], right: [int]) -> [int]:
    merged: [int] = None
    i: int = 0
    j: int = 0

    merged = []
    while i < len(left) and j < len(right):
        if left[i] < right[j]:
            merged = append(merged, left[i])
            i = i + 1
        else:
            merged = append(merged, right[j])
            j = j + 1

    if i < len(left):
        merged = extend(merged, left, i, len(left))
    if j < len(right):
        merged = extend(merged, right, j, len(right))

    return merged


def mergesort(a: [int]) -> [int]:
    mid: int = 0
    left: [int] = None
    right: [int] = None

    if len(a) < 2:
        return a

    mid = len(a) // 2
    left = extend([], a, 0, mid)
    right = extend([], a, mid, len(a))

    left = mergesort(left)
    right = mergesort(right)
    return merge(left, right)


initial: [int] = None
ordered: [int] = None

initial = [2, 7, 3, 11, 5]
ordered = mergesort(initial)

print(ordered)
