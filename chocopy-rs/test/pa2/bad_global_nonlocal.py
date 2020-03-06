a:int = 0
def x():
    a:str = ""
    def y():
        global a
        def z():
            def w():
                nonlocal a # error: not a nonlocal
                pass
            print(a) # int
        print(a) # int
    pass
