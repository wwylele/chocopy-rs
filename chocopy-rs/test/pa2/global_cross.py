a: int = 0
def f():
    a: str = ""
    def g():
        global a
        def h():
            b: int = 0
            b = a
        pass
    pass
