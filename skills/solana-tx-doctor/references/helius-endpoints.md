# Helius endpoints

Get a free key at https://dashboard.helius.dev. Keep it in `HELIUS_API_KEY`. Never print it, and never print a URL that contains it.

## getTransaction

`POST https://mainnet.helius-rpc.com/?api-key=$HELIUS_API_KEY`

```json
{"jsonrpc":"2.0","id":1,"method":"getTransaction",
 "params":["<SIG>",{"encoding":"json","maxSupportedTransactionVersion":1,"commitment":"finalized"}]}
```

`result.meta.err` and `result.meta.logMessages` hold everything the procedure reads. A `null` result means the node has no such transaction.

## getTransactionsForAddress

A Helius RPC method that returns full transactions for an address with a status filter, in one call.

```json
{"jsonrpc":"2.0","id":1,"method":"getTransactionsForAddress",
 "params":["<ADDR>",{"transactionDetails":"full","sortOrder":"desc","limit":10,"encoding":"json",
   "maxSupportedTransactionVersion":1,"filters":{"status":"failed"}}]}
```

The Helius docs do not say clearly whether the free plan includes this method. If it returns an error, fall back to `getSignaturesForAddress` with `{"limit":100}`, keep the entries whose `err` is not null, and call `getTransaction` for each one.

## Parsed Events

`POST https://mainnet.helius-rpc.com/v1/parsed-events/transactions?api-key=$HELIUS_API_KEY`

```json
{"transactions":["<SIG>"],"includeRawTransaction":false}
```

Each item has `parsed.decodedError` with `programId`, `code` and `name`. Use it as a second opinion on your own decode, not as the only source.

## Without a key

The public RPC at `https://api.mainnet-beta.solana.com` serves `getTransaction`, `getSignaturesForAddress` and the scan above. It rate limits with HTTP 429, so retry once after a short wait and space out scans.
