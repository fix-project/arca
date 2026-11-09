# Suspended dependencies

`Task` holds a caller's request, opaque evaluation state, and resolved dependency results. `poll` invokes the caller's evaluation function and reports completion or missing requests. `resume` validates dependency slots and installs their results. The caller owns execution, proof validation, readiness queues, placement, and transport.

Dependency keys are chosen by the caller; use canonical identities when results cross stores or machines.
