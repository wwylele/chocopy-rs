# Please also refer to other files in student_contributed for more tests

x:int = 1
x:int = 2 # global name collision

class u(y): # define u before y
    pass

class y(object):
    z: int = 0
    def z(self: y): # member name collision
        pass
    def bar(): # missing self
        pass
    def foo(self: y, y: int): # shadowing class name
        pass
    def baz(self: y, x: x): # x is not a type
        pass

class v(y):
    def foo(self: v, n: int) -> int: # override with wrong signature
        return 0
    def fooo(self: v, n: int) -> int:
        if n == 0:
            return 0
        else:
            print("missing return")

y: str = "" # collision with class name
