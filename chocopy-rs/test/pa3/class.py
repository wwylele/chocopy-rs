class A(object):
    def foo(self:"A", u: int):
        print("==foo==")
        print(self.a)
        print(u)

    def bar(self:"A"):
        print("==bar==")
        print(self.aa)

    a: str = "Hello"
    aa: bool = True
    aaa: int = 69
    ax: object = None

class B(A):
    def __init__(self:"B"):
        print("B.__init__")
    def bar(self:"B"):
        print("==bar2==")
        print(self.b2)

    b: int = 42
    b2: int = 48
    bb: str = "World"

x: B = None
y: A = None
x = B()
y = A()
print(x.b)
print(x.bb)
print(x.aaa)
print(x.b2)
x.ax = 5
y = x
x.a = x.a + x.a
x = None
print(y.a)
print(y.aa)
print(y.ax)
y.ax = "hey"
print(y.ax)
y.foo(5)
y.bar()


#!
#<->#
#B.__init__
#42
#World
#69
#48
#HelloHello
#True
#5
#hey
#==foo==
#HelloHello
#5
#==bar2==
#48
#<->#
