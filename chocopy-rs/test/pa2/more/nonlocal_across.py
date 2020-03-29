def x():
    a:str = ""
    def y():
        def z():
            nonlocal a
            a = "a"
        pass
    pass
