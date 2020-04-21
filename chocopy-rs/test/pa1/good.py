z: int = 3
class C(object):
    m: [[int]] = None
    def f(self: "C") -> bool:
        return True
    def j(o: C, a: [[[[[[[int]]]]]]]):
        pass

def g(a: int, b: int) -> int:
    nonlocal z
    def h():
        global z
        if True if True else False:
            if False:
                pass
      # unaligned comment
        else:
            if 1 > 2:
                while "\n0"[z] == "\"":
                    pass
            elif 3 > 2:
                for z in C().m[0]:
                    return
    return a+-b

y: [int] = None
x: C = None
x = C()
y = [1, 2, 3]
x.m = [[0], []] ##### I am comment
# Some really lone expression
z = y[y[0]] = (3 if y * (2 + 3) > x[0][0] // 4 - 1 and z < y or (y < z + 1 or z < y * 2) and not "aa\\\t"[1][0 if True else 1][0] == "a" else x.m[0][0]) if (x is y) != True if x.f() else [[], [C(), C()]][1][0].m[0][0] if False else g(1,2) + 3 + 4 %5 + (6-7) else 7
print(x.m[x.n[0][0]][[0][0]])
