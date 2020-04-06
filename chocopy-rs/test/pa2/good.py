# Please also refer to other files in student_contributed for more tests

class a(object):
    a: a = None # members can have the same name as class

A: a = None
B: [b] = None

class b(object):
    def b(self:b, other:a): # methods can have same name as class
        A.a = other # assign to member doesn't need global decl
        B[1] = None # nor does assign to array element
        self.b(other.a)

len(42.__init__().__init__()).__init__() # some crazy use of __init__
