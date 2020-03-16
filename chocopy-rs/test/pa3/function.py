aa:int = 0
cc:str = ""
dd:object = None
def f(a:int, b:bool, c:str, d:object, e:[int], m:[str], n:[object]):
    global aa
    global cc
    global dd
    o:int = 98
    p:str = "oh"
    def g(g1: object, g2: object, g3: object) -> str:
        ggg: str = "hmm"
        def h(h1: object) -> int:
            nonlocal o
            nonlocal ggg
            nonlocal g2
            global aa
            print("h()")
            print(h1)
            print(c)
            print(cc)
            print(g2)
            for aa in e:
                print(aa)
                o = o + aa
            ggg = ggg + ggg
            g2 = ggg + c
            print(ggg)
            e[0] = 80
            return a
        print("g()")
        print(h(o))
        print(g1)
        g2 = 289
        ggg = "well"
        print(h(p))
        print(g2)
        print(g3)
        print(d)
        return c + ggg
    def i(u:int, v:str):
        print("i()")
        print(u)
        print(g(a, u, c))
        print(v)
        print(g(b, v, v))
        print(c)
        print(p)
    print("f()")

    print(a)
    print(b)
    print(c)
    i(333, "kkk")
    print(d)
    print(o)
    for aa in e:
        print(aa)
    for c in m:
        print(c)
    for dd in n:
        print(dd)
    o = 98
    p = "nice"
    i(444, "lll")
    print(p)
    print(o)

f(42, True, "Hello", 51, [22,33,44],["aaa","bbb"],[1,False])
f(39, False, "World", "!", [55,66],["j","i"],[True, "What"])

#!
#<->#
#f()
#42
#True
#Hello
#i()
#333
#g()
#h()
#98
#Hello
#
#333
#22
#33
#44
#hmmhmm
#42
#42
#h()
#oh
#Hello
#
#289
#80
#33
#44
#wellwell
#42
#wellwellHello
#Hello
#51
#Hellowellwell
#kkk
#g()
#h()
#354
#Hello
#
#kkk
#80
#33
#44
#hmmhmm
#42
#True
#h()
#oh
#Hello
#
#289
#80
#33
#44
#wellwell
#42
#wellwellHello
#kkk
#51
#Hellowellwell
#Hello
#oh
#51
#668
#80
#33
#44
#aaa
#bbb
#1
#False
#i()
#444
#g()
#h()
#98
#bbb
#
#444
#80
#33
#44
#hmmhmm
#42
#42
#h()
#nice
#bbb
#
#289
#80
#33
#44
#wellwell
#42
#wellwellbbb
#bbb
#51
#bbbwellwell
#lll
#g()
#h()
#412
#bbb
#
#lll
#80
#33
#44
#hmmhmm
#42
#True
#h()
#nice
#bbb
#
#289
#80
#33
#44
#wellwell
#42
#wellwellbbb
#lll
#51
#bbbwellwell
#bbb
#nice
#nice
#726
#f()
#39
#False
#World
#i()
#333
#g()
#h()
#98
#World
#
#333
#55
#66
#hmmhmm
#39
#39
#h()
#oh
#World
#
#289
#80
#66
#wellwell
#39
#wellwellWorld
#World
#!
#Worldwellwell
#kkk
#g()
#h()
#365
#World
#
#kkk
#80
#66
#hmmhmm
#39
#False
#h()
#oh
#World
#
#289
#80
#66
#wellwell
#39
#wellwellWorld
#kkk
#!
#Worldwellwell
#World
#oh
#!
#657
#80
#66
#j
#i
#True
#What
#i()
#444
#g()
#h()
#98
#i
#
#444
#80
#66
#hmmhmm
#39
#39
#h()
#nice
#i
#
#289
#80
#66
#wellwell
#39
#wellwelli
#i
#!
#iwellwell
#lll
#g()
#h()
#390
#i
#
#lll
#80
#66
#hmmhmm
#39
#False
#h()
#nice
#i
#
#289
#80
#66
#wellwell
#39
#wellwelli
#lll
#!
#iwellwell
#i
#nice
#nice
#682
#<->#
