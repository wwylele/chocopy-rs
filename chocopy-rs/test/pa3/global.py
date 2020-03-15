a:int = 3
b:int = 4
c:object = 5
d:bool = True
e:object = False
f:str = "hello"
g:[int] = None
h:object = "world"
i:[bool] = None
j:[object] = None
print(a)
print(b)
print(c)
print(d)
print(e)
print(f)
print(h)
print("---")
f = h = "hey"
print(f)
print(h)
a = h = 42
print(a)
print(h)
g = e = [a] + [b]
print(g[1])
g = c = None
print("---")
g = [0,1,2,3,4,5]
g[3] = 33
i = [False, False, False]
i = i + i
a = 0
while a < len(g):
    i[a] = g[a] % 3 != 0
    print(g[a])
    a = a + 1
a = 0
while a < len(i):
    print(i[a])
    a = a + 1
j = [0, "hehe", True]
j[2] = "what"
a = 0
while a < len(j):
    print(j[a])
    a = a + 1
a = 0
j[1] = False
while a < len(j):
    print(j[a])
    a = a + 1
#!
#<->#
#3
#4
#5
#True
#False
#hello
#world
#---
#hey
#hey
#42
#42
#4
#---
#0
#1
#2
#33
#4
#5
#False
#True
#True
#False
#True
#True
#0
#hehe
#what
#0
#False
#what
#<->#
