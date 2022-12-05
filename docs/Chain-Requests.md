# Chain Requests

TsengCoin supports the ability to make requests, analogous to web requests, to other addresses on the blockchain. This will be used to allow two addresses to set up a chat session without revealing their identities to anyone else. In order for two nodes to begin to communicate with chain requests, they must first perform a Diffie-Hellman key exchange. This only works if both nodes are online. The address who wishes to initiate a connection will generate a public and a private key and create a P2PKH transaction paying some amount of TsengCoin to the address he/she wishes to connect to. In the transaction's metadata, the sender will provide the public key and the sender will prefix the entire string with `DH` to indicate that this is a Diffie-Hellman request.

If the other address wishes to reciprocate the connection request, they will generate their own secret and public key and create a P2PKH transaction paying the first node some amount of TsengCoin. They will put the public key in the metadata field, using the same scheme as the first node.

When the first node receives this reciprocating request, the Diffie-Hellman exchange is complete and both nodes now have a shared secret. The shared secret is used to create an AES-256 key, and the AES-256 key will be used with a nonce counter to encrypt/decrypt chain requests made between the two nodes for as long as both nodes are online. If one or both nodes goes offline, they will need to perform the Diffie-Hellman exchange again and obtain a new symmetric key if they wish to communicate.

## Security

Conveniently, performing a Diffie-Hellman exchange on the blockchain easily circumvents man-in-the-middle (MITM) attacks. Consider two parties performing a DH exchange without a blockchain or any other public-key infrastructure. It would be possible for a bad actor to sit between the two parties and generate his own secret and public key, which he would use to obtain two shared secrets - one for communication with the first party, and one for communication with the second. This bad actor could intercept messages sent by one party, decrypt and read them, then encrypt them again with the other key and pass them to the second party. Without some way to verify the identity of the recipient, Diffie-Hellman alone is susceptible to MITM attacks. (Note that on the blockchain you can see the history of requests and try to determine if there's a MITM that way - but we don't have to deal with that for the reasons mentioned here.)

In TsengCoin, a [P2PKH transaction](./Transactions.md#pay-to-public-key-hash) is signed by the party that created it. If a P2PKH transaction has an invalid signature, it will be dropped by the network. By the time we check the metadata field to see if the transaction is a chain request, we have already validated it and ensured that the sender address has indeed originated the transaction. The metadata is included in the message data that is signed by the sender, so we know that the sender created whatever request is in the metadata. A man in the middle would not be able to reproduce valid signatures for another party's public key, therefore any message sent by the man in the middle would be rejected. This is a crucial property because by the nature of a blockchain, every chain request is public and every node is a potential "man in the middle."

## Cost of a Request

Chain requests cost some amount of TsengCoin because they are TsengCoin transactions, and a transaction can't have empty inputs or empty outputs unless it is a coinbase transaction (the core client never treats coinbase transactions as chain requests). The transaction fee must also be at least 1 TsengCoin, so with every chain request some amount of TsengCoin is lost to the miner as fees. This was intentional - we chose to make the transaction fee nonzero to help mitigate chain request spam, or just transaction spam in general.

## Chain Request Structure

A chain request transaction must be a P2PKH transaction, and it can have no more than two outputs: one output to the recipient of the request, and a possible second output returning some change back to the sender. The metadata field must start with `DH` or `ENC`. For a Diffie-Hellman exchange request, the metadata looks something like this:

```
DH 241fc6dfd2cb6a79f2297521ba1514a0e62f37eeb5552e5b0379b4348efbac7c
```

The hex integer is the sender's public key generated for the DH exchange (not the public key corresponding to the sender's address), which the recipient will use to create a shared secret.

If the recipient wishes to reciprocate the connection request, they will send some TsengCoin back to the sender - in our core client, they return all the TsengCoin that was originally sent to them and pay a fixed fee of 1 TsengCoin. They are not required to do this, but a client may decide to abort a DH exchange if the other party isn't courteous (TODO: "payback percentage" setting and exclusivity) and keeps the TsengCoin. Note also that if a party is offline, they won't return any of the TsengCoin (TODO: check for previous DH exchange requests upon joining and send back TsengCoin).

In the reciprocating request, the other party puts their public key in the transaction metadata:

```
DH c0599d307945fc695e1ae47eddb55edd6187f03e02b6e30e552ce93e40fe4778
```

The other party now has enough information to create an AES key and associate it to the given address.

When the sender receives this transaction, it can create the same AES key as the other party and send an encrypted request. For an encrypted chain request, the metadata must start with `ENC` and contain the ciphertext encoded in base58check:

```
ENC XQgKyVEa3yeB7cjnuzftuZMtpD8re1qgLBQWJG8PgjNskqyEjvuV5FAVwM
```

Like before, the transaction can have at most 2 outputs.

The other party will receive this transaction, determine that it is intended for them, and decrypt it with the AES key associated with the sender. If there is no AES key associated with the sender, if decryption fails, or if deserialization fails, the other party proceeds normally. The transaction is still considered valid - remember that chain requests are embedded in already valid transactions. It would not be feasible for chain requests to have a part in determining the validity of a transaction because every node would need to be able to decrypt and deserialize the ciphertext.
