# TsengScript Language Spec

The TsengScript language is based off of Bitcoin's Script language. The presence of scripts in transactions allows users to define custom logic that must be satisfied for their payment to be received. In effect, scripts are simple smart contracts. Unlike Ethereum's smart contracts, TsengScript is not Turing complete, and TsengScripts do not connect to the internet (although they can easily be extended to do so). If the language was Turing complete, then it would be possible to write a script that never halts and prevent any node from ever verifying the blockchain past that point. We wouldn't be able to determine if this script halts because of the Halting Problem, so we chose to deal with this by dumbing down the language. Other platforms like Ethereum require the sender to pay "gas fees" for every instruction executed so that the sender's program would always halt, whether naturally or due to excessive gas fees.

The language is very simple. Tokens are delimited by spaces, and then "executed" from left to right. The execution context is an empty stack (TODO: put tx data on stack before execution). Tokens can be one of a few types: `Bool`, `UByteSeq`, or `Operator`. A bool is either `TRUE` or `FALSE`, and a `UByteSeq` is an unsigned integer of any size. Integer literals in this language are always in hexadecimal and are interpreted as `UByteSeq` literals. Execution proceeds as follows:

- Read the next token from the left
- If the token is not an operator, push it on the stack
- If the token is an operator, pop the args off the stack and perform the operation. Push the result on the stack.

Operators take arguments which must be present on the stack when the operator is called. This means that using an operator may look something like this: `(arg2) (arg1) OPERATOR`. In this case, `arg2` is pushed onto the stack, then `arg1`, then `OPERATOR` is an operator that takes two arguments so `arg1` is popped off the stack as the first argument, then `arg2` is popped off as the second argument, then the result of `(arg2) (arg1) OPERATOR` is evaluated and pushed on the stack. There are several operators, listed here with their arguments:

- `(op2: UByteSeq) (op1: UByteSeq) ADD` -> `UByteSeq`
  - Adds `op1` and `op2` without overflow and pushes the sum on the stack.
- `(op2: UByteSeq) (op1: UByteSeq) SUB` -> `UByteSeq`
  - Performs `op1 - op2` (with overflow, because the arguments are unsigned) and pushes the result on the stack.
- `(op2: Bool | UByteSeq) (op1: Bool | UByteSeq) EQUAL` -> `Bool`
  - Compares `op1` and `op2` and pushes the result on the stack.
- `(op2: Bool | UByteSeq) (op1: Bool | UByteSeq) REQUIRE_EQUAL` -> `Bool`
  - Compares `op1` and `op2`. If they are equal, pushes `TRUE` on the stack. If they are not equal, throws an error.
- `(op: T) DUP` -> `T`
  - Duplicates `op` and pushes it on the stack. `op` can have any type.
- `(op: UByteSeq) HASH160` -> `UByteSeq`
  - Hashes the given byte sequence using `RIPEMD160(SHA256(op))` and pushes the result on the stack.
- `(data: UByteSeq) (sig: UByteSeq) (public_key: UByteSeq) CHECKSIG` -> `Bool`
  - Checks that the public key matches the private key used to generate `sig` for `data`. This is used in pay-to-public-key-hash (P2PKH) transactions in which a locking script specifies that an unlocking script must produce a signature satisfying the recipient's public key.

More operators coming soon; we still need operators to check ECDSA signatures.

Here is an example TsengScript program:

```
5 2 ADD 9 SUB 2 EQUAL
```

<hr>

_You can run this program directly from the command line with_

```
cargo run run-script [--show-stack] 5 2 ADD 9 SUB 2 EQUAL
```

_If `--show-stack` is provided, the stack at the end of the program's execution will be printed._

<hr>

This program starts with two UByteSeq hex literals that are pushed onto the stack, one after the other, so that the stack looks like this:

```
| 5 | 2 | ->
```

The arrow indicates the direction in which the stack grows.

`ADD` pops the two operands off the stack and pushes the result on, so that the stack looks like this:

```
| 7 | ->
```

The hex literal 9 is pushed onto the stack:

```
| 7 | 9 | ->
```

`SUB` pops its two operands off the stack. The first argument is the topmost token on the stack and the second argument is the one below it. `SUB` here performs `9 - 7` and pushes the result on the stack:

```
| 2 | ->
```

The hex literal `2` is pushed on the stack:

```
| 2 | 2 | ->
```

Finally, `EQUAL` pops its two operands off the stack and compares them. If the operands have different types, `EQUAL` throws an error. In this case both types are `UByteSeq`, so we can compare them. `EQUAL` pushes the result of the comparison on the stack:

```
| TRUE | ->
```

The program has finished executing. The result of this program is `TRUE` because that was the token at the top of the stack when the program finished.

Each input or output in a transaction has an associated script. Outputs have "locking scripts" which detail requirements that need to be met for the recipient to claim the Tsengcoin in the transaction. Let's say that Bob publishes a transaction in which Alice receives 10 Tsengcoin so long as she can satisfy the condition in his locking script. To use the Tsengcoin, Alice must publish a transaction with at least one input corresponding to the transaction from Bob. She must include in this input an "unlocking script" which will satisfy the requirements set by Bob's locking script. To verify a transaction, the two scripts are run sequentially. First the unlocking script is run - this will leave some data on the stack. This data is copied to a new stack, and the locking script is run with that stack. If the result of the script (the value at the top of the stack when it finishes executing) is `TRUE`, then Alice receives the Tsengcoin.
