# Please also refer to other files in student_contributed for more tests

# incompatible type
x:int = True

y:[int] = None
z:[object] = None

# 1. list + <Empty> (empty list is not a list type)
# 2. assiging object to list
y = [1,2,3]+[]

# 1. int + str
# 2. assigning int to list
y = x + "string"

# int + str
x = x + "string"

z = [None]

# Multi assign with [<None>] (even though they are the same target)
z = z = z = [None]

# str as index
y["str"] = 1

# totally messed up arguments
x = len(1,2,3,"hello")

# "is" on int.
if 1 is 2:
    print("ok")
