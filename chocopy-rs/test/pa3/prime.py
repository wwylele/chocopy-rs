max:int = 0
table:[bool] = None
current:int = 2
multi:int = 0
max_str:str = ""
c:str = ""

print("Please input the maximum nunber:")
max_str = input()
for c in max_str:
    max = max * 10
    if c == "0":
        max = max + 0
    elif c == "1":
        max = max + 1
    elif c == "2":
        max = max + 2
    elif c == "3":
        max = max + 3
    elif c == "4":
        max = max + 4
    elif c == "5":
        max = max + 5
    elif c == "6":
        max = max + 6
    elif c == "7":
        max = max + 7
    elif c == "8":
        max = max + 8
    elif c == "9":
        max = max + 9
    else:
        print("Not a digit: " + c + ". Default to 0.")

print("Prime numbers below " + max_str + " are:")

table = [True]
while len(table) < max + 1:
    table = table + table

while current <= max:
    if table[current]:
        print(current)
        multi = current * 2
        while multi <= max:
            table[multi] = False
            multi = multi + current

    current = current + 1

#!
#100
#<->#
#Please input the maximum nunber:
#Prime numbers below 100 are:
#2
#3
#5
#7
#11
#13
#17
#19
#23
#29
#31
#37
#41
#43
#47
#53
#59
#61
#67
#71
#73
#79
#83
#89
#97
#<->#

#!
#10
#<->#
#Please input the maximum nunber:
#Prime numbers below 10 are:
#2
#3
#5
#7
#<->#
