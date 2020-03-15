a:str = ""
b:object = None
c:int = 3
d:object = None
e:[int] = None
for a in "abcde":
    print(a)
print(a)
for b in "xyz":
    print(b)
for c in [1,2,3]+[11,22,33]:
    print(c)
for d in [42, True, False, "Hello"]:
    print(d)
for e in [[9,8,7],[6,5,4],[3,2,1]]:
    print("-")
    for c in e:
        print(c)
#!
#<->#
#a
#b
#c
#d
#e
#e
#x
#y
#z
#1
#2
#3
#11
#22
#33
#42
#True
#False
#Hello
#-
#9
#8
#7
#-
#6
#5
#4
#-
#3
#2
#1
#<->#
