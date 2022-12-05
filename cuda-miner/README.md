# CUDA Mining Kernel

This code is run on a GPU in parallel. The kernel computes the hash of a candidate block header, given the intermediate state of the hash (see [Mining](../docs/Mining.md#optimizations) for an explanation of this) and a nonce.

The main function, `finish_hash`, must hash the last two chunks of the block header. The CPU hashes the first chunk and passes enough information into the kernel for the kernel to hash the next two chunks. Each kernel inserts a different nonce into the block header and computes a hash for that nonce.

The function `finish_hash` takes several arguments:

- `nonces`: Each nonce is a 256-bit integer. The CPU generates several nonces randomly and lays them out sequentially in device memory. When the kernel starts, it retrieves its thread index and uses it to index into the nonce array.
- `prev`: This is the part of the block header that comes before the nonce, but after the first 512 bit chunk. The block header is split into 3 chunks when hashing. The CPU hashes the first chunk and passes the next 11 32-bit integers into the kernel. The kernel will then retrieve its nonce and initialize the message schedule with the first part of the nonce. After hashing the second chunk, the CPU will initialize the message schedule with the remaining part of the nonce, a trailing `1`, and the fixed size of the block header `0x460`. (The trailing `1` and chunk size are part of the SHA256 algorithm.) This constitutes the third and final chunk.
- `hash_vars`: These are the 8 hash variables after hashing the first chunk. The CPU hashes the first chunk and passes these into the kernel so that it can hash the next chunk.
- `hashes`: This is a pointer to some memory on the device that has enough space for the hashes computed by each kernel. If `N` kernels are started in one iteration, then `hashes` will point to an array of size `32 * N` bytes. The kernel indexes into the hash array, computes the hash given its nonce, and places the hash at the corresponding position in the hash array. This makes it easy for the CPU to figure out which nonce produced a given hash after the kernels return.
