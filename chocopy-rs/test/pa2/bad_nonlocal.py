k: int = 2
def f():
    h: int = 1
    def k():
        def h():
            def i():
                nonlocal h
                global k
                h = 2
                pass
            pass
        pass
    pass
