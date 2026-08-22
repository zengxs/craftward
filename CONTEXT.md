# Craftward

Craftward manages isolated development environments for agent-assisted software
work. This glossary names concepts that remain independent of a particular virtual
machine or container implementation.

## Language

**Realm**:
A managed isolated execution environment with its own identity and lifecycle. A
Realm may be realized by a virtual machine or container backend.
_Avoid_: Environment, VM, container, sandbox

**Realm Bundle**:
The durable on-disk package that represents a Realm between runs. Attached
Workspace data does not necessarily belong to the bundle.
_Avoid_: VM bundle, VZ bundle

**Workspace**:
Development data that can be attached to and moved between Realms without defining
their identity.
_Avoid_: Realm, Realm Bundle

**Project Location**:
A filesystem root through which a software project is accessible within one host or
Realm. It identifies an access location rather than the project's durable data or
conversation state.
_Avoid_: Workspace, Working Copy, Checkout

**Saved State**:
Optional execution state captured when a Realm is suspended so that its prior
execution can resume. It is neither durable Workspace data nor a branchable Realm
checkpoint.
_Avoid_: Checkpoint, Snapshot
