# Design of the compiler

## Parser

Both lexer and parser are hand-written.

The lexer is written as a "generator". Generator is not available in stable rust yet, so it is emulated by an async pipe instead.

The parser is a recursive descent parser. Each `parse_xxx(...)` function roughly corresponds to the non-terminal `xxx` in the grammer. Left recursion is implemented using loop instead of actual recursive functions. Operator precedence is implemented by specifying unambiguous grammer with expression "levels". Each `parse_expr`_n_`(...)` handles only operators in the _n_-th level. The longest look-ahead is k = 2 that happens in distinguish variable declarations and statements.

## Semantic analysis

chocopy-rs supports intermediate AST (typed or untyped) in JSON format that conforms to the course project specification. The internal format of the AST, however, is different from the one provided in the Java starter code. The type hierarchy from the java code is devirtualized into trees of `struct`s and `enum`s for idiomatic Rust code. Similarly, the dispatch pattern in the semantic analyser is devirtualized into pattern matching on `enum`s.

## Code generation

The code generation part was worked on before the consulting the [implementation guide](https://chocopy.org/chocopy_implementation_guide.pdf) from the course. Together with the fact that chocopy-rs targets a different architecture, the implementation detail is different from the one in the guide in many ways.

### Naming conventions

Symbol names for functions and classes follow a similar rule to the guide, except that user-defined functions are not prefixed with `$`. The `$` prefix is generally reserved for built-in / hidden functions and data blocks.
 - The main procedure is named `$chocopy_main`.
 - The global variable section is named `$global`.
 - Global functions use their function names directly as symbol names.
 - Constructors use type names as symbol names.
 - Destructors use `<TypeName>.$dtor` as symbol names.
 - Methods use `<TypeName>.<FuncName>` as symbol names.
 - Prototypes use `<TypeName>.$proto` as symbol names.
 - Nested functions use `<ParentSymbolName>.<FuncName>` as symbol names.
 - Functions from the standard library all have symbol names starts with `$`, except for the program entry point `main`.

All the names above are visible to the linker, and also appear as debugging symbols. There are also symbols for debugging only:
 - All variables and attributes are named as-is.
 - Hidden attributes are prefixed with `$`. See sections below for detail.

### Register conventions

`RSP` and `RBP` are used in their conventional ways: stack pointer and frame base pointer. Other registers are for general use.

### Objects

Since chocopy-rs targets a 64-bit architecture, objects are represented by 64-bit address, with address 0 reserved for `None` as usual.

#### Unwrapped values

This compiler also make use of unwrapped values for `int` and `bool`, which are 4-byte and 1-byte, respectively. They are always stored in 8-byte slots on the stack. However, in the global section and in object attributes, they are stored in a losely "packed" way: they are only aligned to their own size. Because of this, object layout does not have uniform slots any more.

#### Object layout

An object is a record containing a 16-byte header followed by object attributes. The object header contains a 8-byte pointer to the object prototype (`$proto`), and a 8-byte reference counter (`$ref`, for garbage collection).

Array-like objects (`str` and `[T]`) has a 16-byte `$len` attribute after the object header, followed by the array data. Note that `[int]` and `[bool]` has packed layout where each element is only 4 or 1 byte. `str` is also packed with 1-byte ASCII characters. Strings in `str` are **not** null-terminated, and `\0` is allowed as a valid ASCII character in strings.

#### Prototype objects

For each type with name `C` (including all primitive types), the global symbol `C.$proto` points to a _prototype_ object of that class. All objects of type `C` also has its `$proto` attribute pointed to this prototype object. Prototype objects in chocopy-rs share two purposes with prototype objects in the reference implementation: the type tag and the dispatch table. The third purpose of prototype objects in the reference implementation, object initial values, is **not** included in the prototype here. Please refer the next section _Constructors_ for this.

A prototype object starts with a 4-byte signed integer `$size` that describes the memory layout of objects of type `C`. When it is a positive number, it is the object size without the object header. When it is a negative number, it indicates `C` is an array-like type and the value without the sign is the size of one element.

Following `$size` is a 4-byte signed integer `$tag` for type tag. The type tag value follows the same convention in the implementation guide, except that all user-defined classes use value 0 for the tag.

Following `$tag` is the list of function pointers to methods. The first function pointer `$dtor` points to the destructor, the second function pointer points to the `__init__` method, and so on for other user-defined methods.

#### Constructors

For each non-list type with name `C`, the global symbol `C` is the constructor for the type. The constructor allocates memory for the object, initialize its attributes, and call its `__init__` method. Instead of copying initial value from a "prototype" object as described in the implementation guide, constructors generated from chocopy-rs simply assign value for each attribute one by one.

### Functions and methods
#### Calling convention

chocopy-rs compiles program using the following calling convention:
 - push all arguments to the stack. The rightmost argument is placed at the highest address.
 - pass static link in `R10` if it is a nested function.
 - ensure the stack pointer is aligned at 8 mod 16 at the beginning of a function.
 - pass return value in `RAX`.
 - caller restores stack.
 - `RAX`, `RCX`, `RDX`, `RSI`, `RDI`, `R8`, `R9`, `R10`, `R11` are volatile across function call.

All functions are called using this calling convention, except for the followings:

 - the main procedure `$chocopy_main`
 - functions in the standard library (all `$`-prefixed functions and `main`)
 - destructors `<TypeName>.$dtor`.

 These functions are called using platform's default C ABI (System V ABI for linux, and Microsoft Windows ABI for Windows).

#### Stack frames

Stack frame layout is similar to the one described in the implementation guide, except for the position of the static link: instead of being pushed below all parameters, it is passed in `R10` upon function call, and pushed immediately above the dynamic link, and below all local variables. So the stack frame layout in top-to-bottom order is:
1. actual parameters to the function it is calling (if applicable),
2. padding bytes to align the callee stack frame to 16 (if applicable),
3. temporary storage for intermediate results of expression evaluation or other operations,
4. the values of the local variables of the function,
5. the static link of the current function (if applicable),
6. the saved frame pointer of its callerm
7. the saved return address. Its own actual parameters are in the frame immediately below it.

The local variables are stored in the same order as described in the implementation guide: vairables declared first has the highest address towards the bottom of the stack. However, the parameters are stored in a different order: the leftmost parameter has the lowest address towards the top of the stack.

### Execution environment

The final executable produced by the compiler can be run directly under the target system. The executable is made from linking 3 major components:
 - the compiled ChocoPy object (`program.o`)
 - the standard library (`chocopy_rs_std`)
 - the system C runtime (`libc`)

Their dependency on each other is shown in the following example call chain:
```
                      libc
--(`main`)->          chocopy_rs_std
--(`$chocopy_main`)-> program.o
--(`print`)->         program.o
--(`$print`)->        chocopy_rs_std
--(`fwrite`)->        libc
```

### Standard library

All ChocoPy programs are linked against a small standard library `chocopy_rs_std`. The library contains the implementation for built-in functions, object (de)allocation, and error reporting. Contrary to the implementation guide, all functions from the standard library are `$`-prefixed, while user-defined ChocoPy functions don't. Note that for each user-invocable built-in function such as `print`, the standard library provides `$print`, and the compiler also generates a wrapper function `print` in the compiled object `program.o`. The purpose of the wrapper is to convert the calling convention from ChocoPy convention to system C convention.

The standard library also provides the program entry point `main`, which directly calls into the main procedure `$chocopy_main` from the compiled object. Doing so is to avoid conflict when the ChocoPy program contains a user-defined function named `main`, which in the current implementation will be a local symbol only visible to the compiled object itself.

### Garbage collection

chocopy-rs implements simple garbage collection by reference counting. Just like all reference counting, this will fail to collect garbage that contains reference cycle. In such case, the compiled program will report memory leak on exit.

#### Destructors

For each type with name `C`, the global symbol `C.$dtor` (also pointed by `C.$proto.$dtor`) is the destructor for the type. The destructor is called by the standard library when the object is being deallocated. The destructor decreases the reference counter for each of its non-unwrapped, non-None attributes, and recursively deallocate more objects if needed.
