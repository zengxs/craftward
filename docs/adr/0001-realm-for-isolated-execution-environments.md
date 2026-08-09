---
status: accepted
---

# Realm for Isolated Execution Environments

Craftward calls a managed isolated execution environment a Realm: it has its own
identity and lifecycle, may be realized by a virtual machine or container backend, and
may have Workspace data attached without that data defining its identity. We use Realm
instead of Environment because Environment is heavily overloaded, and instead of VM,
Container, or Sandbox because those terms either expose an implementation choice or
cover only part of the intended concept. This decision deliberately leaves the
universal capability set, lifecycle state machine, checkpoint and derivation
semantics, and backend interface undefined until concrete implementations establish
genuinely shared behavior.
