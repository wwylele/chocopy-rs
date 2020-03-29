class s(object):
    t: object = None

class r(object):
    sobj: s = None

class q(object):
    robj: r = None

class p(q):
    def f(self: "p") -> object:
        return self.robj.sobj.t

def a() -> int:
    def b() -> int:
        def c() -> int:
            def d() -> int:
                def e() -> int:
                    return 1
                return e()
            return d()
        return c()
    return b()

def w():
 def x():
   def y():
      def z():
          4
      3
   2
 1

xs: [[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[int]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]] = None
xs = [[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[1]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]

if 1:
    if 2:
        if 3:
            if 4:
                True
        elif -3:
            False
else:
    False
