# Reganalys

- It should be possible to build a database of registers that are queriable
- Reference source for such a database will be SVD. But is should be possible to build a database manually.
- It should be possible to query the "address" and get "something" back.
- This "something" (Result<RegisterDescription>?) should be "interactive".
    - Upon success of query one should get an object that can be fed a value in order to "decode it" (get Result<DecodedValue>). It should be possible to run "diffs" against `DecodedValue`s, preferably. This should yield `DecodedValueDiff`?
    - One usecase is to build a storage for "so-far-encountered" registers and their values in order to show diffs
