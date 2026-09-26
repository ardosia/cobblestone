# C005 spec delta

No new stable requirement IDs are needed for the first transport slice.

C005 implements existing requirements:

- CB-001: the initial compatibility target requires RakNet protocol 8.
- CB-008: cross-thread/runtime queues remain bounded and define backpressure.
- CB-011: RakNet transport stays distinct from protocol-84 wire packets and gameplay/domain APIs.

The accepted fixed target remains unchanged. Supporting additional RakNet versions or moving Minecraft packet semantics into the transport would require a later accepted spec change.
