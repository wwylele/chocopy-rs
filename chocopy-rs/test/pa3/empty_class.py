class Empty(object):
    def foo(self:"Empty"):
        print("hello")
    #k: str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"

class NonEmpty(object):
    a: str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    b: Empty = None
    c: "NonEmpty" = None

    def check(self:"NonEmpty", base: int):
        if self.c is None:
            print(base)
        else:
            self.c.check(base + 1 if self.b is None else base * 2)

x: Empty = None
y: NonEmpty = None
z: NonEmpty = None
i: int = 0
while i < 10000:
    x = Empty()
    z = y
    y = NonEmpty()
    if i % 3 == 0:
        y.b = x
    if i % 17 != 0:
        y.c = z
    if i % 1007 == 0:
        y.check(1)
    i = i + 1
x.foo()

#!
#<->#
#1
#7
#28
#46
#157
#5
#20
#38
#125
#3
#hello
#<->#
