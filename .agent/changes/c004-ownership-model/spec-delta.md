# C004 spec delta

C004 introduces no new stable requirement IDs. It implements and sharpens the existing foundation requirements:

- **CB-004**: every live mutable authoritative native value has exactly one owning PHP runtime;
- **CB-005**: production ownership uses the existing type-safe generational identity and stale-handle rejection;
- **CB-006**: mutation and reclamation require the current owner; wrong-owner operations fail safely at the core boundary while semantic routing remains a higher-layer choice;
- **CB-008**: delayed commands are protected by an ownership epoch so stale work cannot mutate after transfer;
- **CB-010**: ownership metadata remains internal mechanism and is not ordinary plugin API ceremony.

No requirement is added for same-process Zend sharing. The successful C003 decision applies to the tested process-isolated runtime topology only.
