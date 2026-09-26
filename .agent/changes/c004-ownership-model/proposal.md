# C004 proposal

C003 validated a conservative process-isolated multi-runtime substrate together with owner-gated handles and ownership epochs. C004 promotes the proven ownership semantics out of the experiment harness and into `cobblestone-core` as reusable production mechanism.

The core mechanism owns identity, current `RuntimeId`, transfer epoch, mutation/reclamation gates, and stale-handle behavior. It does not decide gameplay routing policy. A higher semantic layer may route a wrong-owner command to the current owner when that operation permits routing; otherwise the core error is surfaced safely.

Immutable native values such as `NativeBuffer`, packet buffers, snapshots, and encoded blobs remain shareable by value/reference-counted clone. Sharing those values does not move mutable authority and does not require an ownership transfer.

C004 remains an internal mechanism change. Normal PHP/plugin APIs continue to avoid runtime IDs, epochs, handles, mutexes, and thread/region ceremony.
